use crate::{Error, Relic, Result, Slot, Stat};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const RECOMMENDATION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationMatch {
    Recommended,
    NotRecommended,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecommendationSource {
    pub kind: String,
    pub repository_url: String,
    pub commit: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecommendedMainStats {
    pub body: Vec<Stat>,
    pub feet: Vec<Stat>,
    pub sphere: Vec<Stat>,
    pub rope: Vec<Stat>,
}

impl RecommendedMainStats {
    pub fn for_slot(&self, slot: Slot) -> Option<&[Stat]> {
        match slot {
            Slot::Head | Slot::Hands => None,
            Slot::Body => Some(&self.body),
            Slot::Feet => Some(&self.feet),
            Slot::Sphere => Some(&self.sphere),
            Slot::Rope => Some(&self.rope),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterRelicProfile {
    pub character_id: String,
    pub relic_set_ids: Vec<String>,
    pub ornament_set_ids: Vec<String>,
    pub main_stats: RecommendedMainStats,
    pub substats: Vec<Stat>,
}

#[derive(Debug, Deserialize)]
struct RecommendationDocument {
    schema_version: u32,
    source: RecommendationSource,
    profiles: Vec<CharacterRelicProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterRelicDatabase {
    schema_version: u32,
    source: RecommendationSource,
    profiles: BTreeMap<String, CharacterRelicProfile>,
}

impl CharacterRelicDatabase {
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn source(&self) -> &RecommendationSource {
        &self.source
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    pub fn profile(&self, character_id: &str) -> Option<&CharacterRelicProfile> {
        self.profiles.get(character_id)
    }

    pub fn set_match(&self, character_id: &str, relic: &Relic) -> RecommendationMatch {
        let Some(profile) = self.profile(character_id) else {
            return RecommendationMatch::Unknown;
        };
        let sets = match relic.slot {
            Slot::Head | Slot::Hands | Slot::Body | Slot::Feet => &profile.relic_set_ids,
            Slot::Sphere | Slot::Rope => &profile.ornament_set_ids,
        };
        if sets.contains(&relic.set_id) {
            RecommendationMatch::Recommended
        } else {
            RecommendationMatch::NotRecommended
        }
    }

    pub fn main_stat_match(&self, character_id: &str, relic: &Relic) -> RecommendationMatch {
        let Some(profile) = self.profile(character_id) else {
            return RecommendationMatch::Unknown;
        };
        let Some(stats) = profile.main_stats.for_slot(relic.slot) else {
            return RecommendationMatch::Recommended;
        };
        if stats.contains(&relic.main_stat) {
            RecommendationMatch::Recommended
        } else {
            RecommendationMatch::NotRecommended
        }
    }
}

pub fn load_character_relic_database(json: &str) -> Result<CharacterRelicDatabase> {
    let document: RecommendationDocument = serde_json::from_str(json)
        .map_err(|error| Error(format!("静态推荐数据库 JSON 无效：{error}")))?;
    if document.schema_version != RECOMMENDATION_SCHEMA_VERSION {
        return Err(Error(format!(
            "不支持静态推荐数据库 schema_version {}",
            document.schema_version
        )));
    }
    if document.source.kind.is_empty()
        || document.source.repository_url.is_empty()
        || document.source.commit.is_empty()
        || document.source.path.is_empty()
        || document.source.sha256.len() != 64
    {
        return Err(Error("静态推荐数据库缺少完整来源信息".into()));
    }
    if document.profiles.is_empty() {
        return Err(Error("静态推荐数据库没有角色记录".into()));
    }

    let mut profiles = BTreeMap::new();
    for profile in document.profiles {
        validate_profile(&profile)?;
        let character_id = profile.character_id.clone();
        if profiles.insert(character_id.clone(), profile).is_some() {
            return Err(Error(format!("静态推荐数据库包含重复角色 {character_id}")));
        }
    }
    Ok(CharacterRelicDatabase {
        schema_version: document.schema_version,
        source: document.source,
        profiles,
    })
}

fn validate_profile(profile: &CharacterRelicProfile) -> Result<()> {
    if profile.character_id.is_empty()
        || profile.relic_set_ids.is_empty()
        || profile.ornament_set_ids.is_empty()
    {
        return Err(Error(format!(
            "角色 {} 的套装推荐不完整",
            profile.character_id
        )));
    }
    unique_nonempty(&profile.relic_set_ids, &profile.character_id, "外圈套装")?;
    unique_nonempty(&profile.ornament_set_ids, &profile.character_id, "位面套装")?;
    for (slot, stats) in [
        (Slot::Body, &profile.main_stats.body),
        (Slot::Feet, &profile.main_stats.feet),
        (Slot::Sphere, &profile.main_stats.sphere),
        (Slot::Rope, &profile.main_stats.rope),
    ] {
        if stats.is_empty()
            || stats.iter().copied().any(|stat| !valid_main(slot, stat))
            || stats.iter().copied().collect::<BTreeSet<_>>().len() != stats.len()
        {
            return Err(Error(format!(
                "角色 {} 的 {:?} 主属性推荐无效",
                profile.character_id, slot
            )));
        }
    }
    if profile.substats.is_empty()
        || profile.substats.iter().any(|stat| !stat.is_substat())
        || profile
            .substats
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != profile.substats.len()
    {
        return Err(Error(format!(
            "角色 {} 的副属性推荐无效",
            profile.character_id
        )));
    }
    Ok(())
}

fn unique_nonempty(values: &[String], character_id: &str, label: &str) -> Result<()> {
    if values.iter().any(|value| value.is_empty())
        || values.iter().collect::<BTreeSet<_>>().len() != values.len()
    {
        return Err(Error(format!("角色 {character_id} 的{label} ID 无效")));
    }
    Ok(())
}

fn valid_main(slot: Slot, stat: Stat) -> bool {
    match slot {
        Slot::Head => stat == Stat::Hp,
        Slot::Hands => stat == Stat::Atk,
        Slot::Body => matches!(
            stat,
            Stat::HpPercent
                | Stat::AtkPercent
                | Stat::DefPercent
                | Stat::CritRate
                | Stat::CritDamage
                | Stat::EffectHit
                | Stat::Healing
        ),
        Slot::Feet => matches!(
            stat,
            Stat::HpPercent | Stat::AtkPercent | Stat::DefPercent | Stat::Speed
        ),
        Slot::Sphere => matches!(
            stat,
            Stat::HpPercent
                | Stat::AtkPercent
                | Stat::DefPercent
                | Stat::PhysicalDamage
                | Stat::FireDamage
                | Stat::IceDamage
                | Stat::LightningDamage
                | Stat::WindDamage
                | Stat::QuantumDamage
                | Stat::ImaginaryDamage
        ),
        Slot::Rope => matches!(
            stat,
            Stat::HpPercent
                | Stat::AtkPercent
                | Stat::DefPercent
                | Stat::BreakEffect
                | Stat::EnergyRegen
        ),
    }
}
