//! Only this module knows scanner v4 field names. Unused fields (including previews)
//! are ignored; an observed upgrade must always be entered explicitly.
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountImportErrorCode {
    InvalidJson,
    UnsupportedSource,
    UnsupportedVersion,
    InvalidStructure,
    InvalidCharacter,
    InvalidLightCone,
    InvalidRelic,
    InvalidEquipmentRelation,
    DuplicateId,
    NoUsableRelics,
}

impl AccountImportErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidJson => "invalid_json",
            Self::UnsupportedSource => "unsupported_source",
            Self::UnsupportedVersion => "unsupported_version",
            Self::InvalidStructure => "invalid_structure",
            Self::InvalidCharacter => "invalid_character",
            Self::InvalidLightCone => "invalid_light_cone",
            Self::InvalidRelic => "invalid_relic",
            Self::InvalidEquipmentRelation => "invalid_equipment_relation",
            Self::DuplicateId => "duplicate_id",
            Self::NoUsableRelics => "no_usable_relics",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountImportError {
    pub code: AccountImportErrorCode,
    pub path: Option<String>,
    pub message: String,
}

impl AccountImportError {
    fn new(
        code: AccountImportErrorCode,
        path: impl Into<Option<String>>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for AccountImportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(path) = &self.path {
            write!(formatter, "{}（{path}）", self.message)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl std::error::Error for AccountImportError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountImportSummary {
    pub source: String,
    pub version: u32,
    pub build: String,
    pub characters: usize,
    pub relics_in_file: usize,
    pub relics_imported: usize,
    pub relics_skipped: usize,
    pub light_cones: usize,
    pub equipped_relics: usize,
    pub imported_equipped_relics: usize,
    pub equipped_light_cones: usize,
    pub equipment_relations_recognized: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedAccount {
    pub account: AccountState,
    pub summary: AccountImportSummary,
}

#[derive(Deserialize)]
struct ScannerAccount {
    source: String,
    #[serde(default)]
    build: String,
    version: u32,
    characters: Vec<ScannerCharacter>,
    #[serde(default)]
    light_cones: Vec<ScannerLightCone>,
    relics: Vec<ScannerRelic>,
}

#[derive(Deserialize)]
struct ScannerCharacter {
    id: String,
    name: String,
    level: u8,
    eidolon: u8,
    ability_version: Option<u8>,
}

#[derive(Deserialize)]
struct ScannerLightCone {
    #[serde(rename = "_uid")]
    uid: String,
    id: String,
    level: u8,
    superimposition: u8,
    location: String,
    lock: bool,
}

#[derive(Deserialize)]
struct ScannerRelic {
    #[serde(rename = "_uid")]
    id: String,
    set_id: String,
    slot: String,
    rarity: u8,
    level: u8,
    mainstat: String,
    substats: Vec<ScannerSubstat>,
    location: String,
    lock: bool,
    discard: bool,
}

#[derive(Deserialize)]
struct ScannerSubstat {
    key: String,
    value: f64,
}

fn substat(key: &str) -> Result<Stat> {
    Ok(match key {
        "HP" => Stat::Hp,
        "ATK" => Stat::Atk,
        "DEF" => Stat::Def,
        "HP_" => Stat::HpPercent,
        "ATK_" => Stat::AtkPercent,
        "DEF_" => Stat::DefPercent,
        "SPD" => Stat::Speed,
        "CRIT Rate_" => Stat::CritRate,
        "CRIT DMG_" => Stat::CritDamage,
        "Effect Hit Rate_" => Stat::EffectHit,
        "Effect RES_" => Stat::EffectRes,
        "Break Effect_" => Stat::BreakEffect,
        _ => return Err(Error(format!("未知 scanner 副属性: {key}"))),
    })
}

fn main_stat(key: &str, slot: Slot) -> Result<Stat> {
    if slot == Slot::Head {
        return Ok(Stat::Hp);
    }
    if slot == Slot::Hands {
        return Ok(Stat::Atk);
    }
    Ok(match key {
        "HP" => Stat::HpPercent,
        "ATK" => Stat::AtkPercent,
        "DEF" => Stat::DefPercent,
        "SPD" => Stat::Speed,
        "CRIT Rate" => Stat::CritRate,
        "CRIT DMG" => Stat::CritDamage,
        "Effect Hit Rate" => Stat::EffectHit,
        "Break Effect" => Stat::BreakEffect,
        "Energy Regeneration Rate" => Stat::EnergyRegen,
        "Outgoing Healing Boost" => Stat::Healing,
        "Physical DMG Boost" => Stat::PhysicalDamage,
        "Fire DMG Boost" => Stat::FireDamage,
        "Ice DMG Boost" => Stat::IceDamage,
        "Lightning DMG Boost" => Stat::LightningDamage,
        "Wind DMG Boost" => Stat::WindDamage,
        "Quantum DMG Boost" => Stat::QuantumDamage,
        "Imaginary DMG Boost" => Stat::ImaginaryDamage,
        _ => return Err(Error(format!("未知 scanner 主属性: {key}"))),
    })
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

fn relic_slot(value: &str) -> Result<Slot> {
    Ok(match value.replace(' ', "").as_str() {
        "Head" => Slot::Head,
        "Hands" => Slot::Hands,
        "Body" => Slot::Body,
        "Feet" => Slot::Feet,
        "PlanarSphere" => Slot::Sphere,
        "LinkRope" => Slot::Rope,
        _ => return Err(Error(format!("未知部位: {value}"))),
    })
}

/// Loads the Demo subset of HSR-Scanner v4 (five-star relics, +3 checkpoints).
/// Budget is supplied by our caller; scanner materials are not interpreted as steps.
pub fn load_scanner_v4(json: &str, upgrade_steps: u32) -> Result<AccountState> {
    let raw: ScannerAccount =
        serde_json::from_str(json).map_err(|e| Error(format!("JSON 导入失败: {e}")))?;
    if raw.source != "HSR-Scanner" || raw.version != 4 {
        return Err(Error("需要 HSR-Scanner v4 数据".into()));
    }
    let mut account = AccountState {
        characters: BTreeMap::new(),
        light_cones: BTreeMap::new(),
        relics: BTreeMap::new(),
        upgrade_steps,
        history: vec![],
        decisions: BTreeMap::new(),
    };
    for c in raw.characters {
        if c.id.is_empty() || c.name.is_empty() || c.level == 0 || c.level > 80 || c.eidolon > 6 {
            return Err(Error("角色字段无效".into()));
        }
        if c.ability_version != Some(0) {
            return Err(Error(
                "v0.1 需要显式 ability_version: 0；buffed 版本留待后续导入层支持".into(),
            ));
        }
        let character = Character {
            id: c.id.clone(),
            name: c.name,
            level: c.level,
            eidolon: c.eidolon,
            light_cone: None,
        };
        if account.characters.insert(c.id.clone(), character).is_some() {
            return Err(Error(format!("重复角色 ID: {}", c.id)));
        }
    }
    for lc in raw.light_cones {
        if lc.uid.is_empty()
            || lc.id.is_empty()
            || !(1..=80).contains(&lc.level)
            || !(1..=5).contains(&lc.superimposition)
        {
            return Err(Error("光锥字段或装备关系无效".into()));
        }
        let equipped_by = (!lc.location.is_empty()).then_some(lc.location.clone());
        let inventory = InventoryLightCone {
            uid: lc.uid.clone(),
            id: lc.id,
            level: lc.level,
            superimposition: lc.superimposition,
            equipped_by: equipped_by.clone(),
            locked: lc.lock,
        };
        if let Some(character_id) = equipped_by {
            let character = account
                .characters
                .get_mut(&character_id)
                .ok_or_else(|| Error(format!("光锥装备角色不存在: {character_id}")))?;
            if character.light_cone.is_some() {
                return Err(Error("同一角色重复装备光锥".into()));
            }
            character.light_cone = Some(LightCone {
                id: inventory.id.clone(),
                level: inventory.level,
                superimposition: inventory.superimposition,
            });
        }
        if account
            .light_cones
            .insert(lc.uid.clone(), inventory)
            .is_some()
        {
            return Err(Error(format!("重复光锥 UID: {}", lc.uid)));
        }
    }
    for r in raw.relics {
        let slot = relic_slot(&r.slot)?;
        if r.id.is_empty()
            || r.set_id.is_empty()
            || r.rarity != 5
            || r.level > 15
            || r.level % 3 != 0
        {
            return Err(Error(format!(
                "遗器 {}: v0.1 需要五星、0..15 的 +3 检查点",
                r.id
            )));
        }
        let main_stat = main_stat(&r.mainstat, slot)?;
        if !valid_main(slot, main_stat) {
            return Err(Error("主属性与部位不匹配".into()));
        }
        let mut substats = BTreeMap::new();
        for s in r.substats {
            let stat = substat(&s.key)?;
            if !s.value.is_finite()
                || s.value <= 0.0
                || stat == main_stat
                || substats.insert(stat, s.value).is_some()
            {
                return Err(Error(format!("遗器 {} 的副属性无效或重复", r.id)));
            }
        }
        if !(3..=4).contains(&substats.len()) || (r.level >= 3 && substats.len() != 4) {
            return Err(Error(format!("遗器 {} 的副属性数量与等级不符", r.id)));
        }
        let equipped_by =
            if r.location.is_empty() {
                None
            } else {
                if !account.characters.contains_key(&r.location) {
                    return Err(Error("遗器装备角色不存在".into()));
                }
                if account.relics.values().any(|other| {
                    other.equipped_by.as_ref() == Some(&r.location) && other.slot == slot
                }) {
                    return Err(Error("同一角色同一部位重复装备".into()));
                }
                Some(r.location)
            };
        let relic = Relic {
            id: r.id.clone(),
            slot,
            set_id: r.set_id,
            rarity: r.rarity,
            level: r.level,
            main_stat,
            substats,
            equipped_by,
            locked: r.lock,
            discarded: r.discard,
        };
        if account.relics.insert(r.id.clone(), relic).is_some() {
            return Err(Error(format!("重复遗器 ID: {}", r.id)));
        }
    }
    Ok(account)
}

pub fn load_reliquary_v4(
    json: &str,
    upgrade_steps: u32,
) -> std::result::Result<ImportedAccount, AccountImportError> {
    let raw: ScannerAccount = serde_json::from_str(json).map_err(|error| {
        AccountImportError::new(
            AccountImportErrorCode::InvalidJson,
            None,
            format!("Reliquary JSON 无法解析：{error}"),
        )
    })?;
    if raw.source != "reliquary_archiver" {
        return Err(AccountImportError::new(
            AccountImportErrorCode::UnsupportedSource,
            Some("source".into()),
            "需要 Reliquary Archiver 导出的 JSON",
        ));
    }
    if raw.version != 4 {
        return Err(AccountImportError::new(
            AccountImportErrorCode::UnsupportedVersion,
            Some("version".into()),
            "当前只支持 Reliquary schema version 4",
        ));
    }
    if raw.characters.is_empty() || raw.relics.is_empty() {
        return Err(AccountImportError::new(
            AccountImportErrorCode::InvalidStructure,
            None,
            "账号必须包含角色和遗器",
        ));
    }

    let source_relics = raw.relics.len();
    let source_light_cones = raw.light_cones.len();
    let mut account = AccountState {
        characters: BTreeMap::new(),
        light_cones: BTreeMap::new(),
        relics: BTreeMap::new(),
        upgrade_steps,
        history: vec![],
        decisions: BTreeMap::new(),
    };

    for (index, character) in raw.characters.into_iter().enumerate() {
        let path = format!("characters[{index}]");
        if character.id.is_empty()
            || character.name.is_empty()
            || !(1..=80).contains(&character.level)
            || character.eidolon > 6
        {
            return Err(AccountImportError::new(
                AccountImportErrorCode::InvalidCharacter,
                Some(path),
                "角色字段无效",
            ));
        }
        let id = character.id.clone();
        if account
            .characters
            .insert(
                id.clone(),
                Character {
                    id,
                    name: character.name,
                    level: character.level,
                    eidolon: character.eidolon,
                    light_cone: None,
                },
            )
            .is_some()
        {
            return Err(AccountImportError::new(
                AccountImportErrorCode::DuplicateId,
                Some(path),
                "角色 ID 重复",
            ));
        }
    }

    let mut equipped_light_cones = 0;
    for (index, light_cone) in raw.light_cones.into_iter().enumerate() {
        let path = format!("light_cones[{index}]");
        if light_cone.uid.is_empty()
            || light_cone.id.is_empty()
            || !(1..=80).contains(&light_cone.level)
            || !(1..=5).contains(&light_cone.superimposition)
        {
            return Err(AccountImportError::new(
                AccountImportErrorCode::InvalidLightCone,
                Some(path),
                "光锥字段无效",
            ));
        }
        let equipped_by = (!light_cone.location.is_empty()).then_some(light_cone.location);
        let item = InventoryLightCone {
            uid: light_cone.uid.clone(),
            id: light_cone.id,
            level: light_cone.level,
            superimposition: light_cone.superimposition,
            equipped_by: equipped_by.clone(),
            locked: light_cone.lock,
        };
        if let Some(character_id) = equipped_by {
            let character = account.characters.get_mut(&character_id).ok_or_else(|| {
                AccountImportError::new(
                    AccountImportErrorCode::InvalidEquipmentRelation,
                    Some(format!("{path}.location")),
                    "光锥装备关系引用了不存在的角色",
                )
            })?;
            if character.light_cone.is_some() {
                return Err(AccountImportError::new(
                    AccountImportErrorCode::InvalidEquipmentRelation,
                    Some(format!("{path}.location")),
                    "同一角色不能装备多件光锥",
                ));
            }
            character.light_cone = Some(LightCone {
                id: item.id.clone(),
                level: item.level,
                superimposition: item.superimposition,
            });
            equipped_light_cones += 1;
        }
        if account.light_cones.insert(light_cone.uid, item).is_some() {
            return Err(AccountImportError::new(
                AccountImportErrorCode::DuplicateId,
                Some(path),
                "光锥 _uid 重复",
            ));
        }
    }

    let mut seen_relic_ids = BTreeSet::new();
    let mut equipment_slots = BTreeSet::new();
    let mut skipped_relics = 0;
    let mut equipped_relics = 0;
    let mut imported_equipped_relics = 0;
    for (index, raw_relic) in raw.relics.into_iter().enumerate() {
        let path = format!("relics[{index}]");
        if raw_relic.id.is_empty() || !seen_relic_ids.insert(raw_relic.id.clone()) {
            return Err(AccountImportError::new(
                AccountImportErrorCode::DuplicateId,
                Some(format!("{path}._uid")),
                "遗器 _uid 为空或重复",
            ));
        }
        let slot = relic_slot(&raw_relic.slot).map_err(|_| {
            AccountImportError::new(
                AccountImportErrorCode::InvalidRelic,
                Some(format!("{path}.slot")),
                "遗器部位无效",
            )
        })?;
        let maximum_level = raw_relic.rarity.saturating_mul(3);
        if raw_relic.set_id.is_empty()
            || !(2..=5).contains(&raw_relic.rarity)
            || raw_relic.level > maximum_level
            || raw_relic.substats.is_empty()
            || raw_relic.substats.len() > 4
        {
            return Err(AccountImportError::new(
                AccountImportErrorCode::InvalidRelic,
                Some(path.clone()),
                "遗器基础字段无效",
            ));
        }
        let main_stat = main_stat(&raw_relic.mainstat, slot).map_err(|_| {
            AccountImportError::new(
                AccountImportErrorCode::InvalidRelic,
                Some(format!("{path}.mainstat")),
                "遗器主属性无效",
            )
        })?;
        if !valid_main(slot, main_stat) {
            return Err(AccountImportError::new(
                AccountImportErrorCode::InvalidRelic,
                Some(format!("{path}.mainstat")),
                "遗器主属性与部位不匹配",
            ));
        }
        let mut substats = BTreeMap::new();
        for (substat_index, raw_substat) in raw_relic.substats.into_iter().enumerate() {
            let stat = substat(&raw_substat.key).map_err(|_| {
                AccountImportError::new(
                    AccountImportErrorCode::InvalidRelic,
                    Some(format!("{path}.substats[{substat_index}]")),
                    "遗器副属性类型无效",
                )
            })?;
            if !raw_substat.value.is_finite()
                || raw_substat.value <= 0.0
                || stat == main_stat
                || substats.insert(stat, raw_substat.value).is_some()
            {
                return Err(AccountImportError::new(
                    AccountImportErrorCode::InvalidRelic,
                    Some(format!("{path}.substats[{substat_index}]")),
                    "遗器副属性数值无效、重复或与主属性冲突",
                ));
            }
        }
        let equipped_by = (!raw_relic.location.is_empty()).then_some(raw_relic.location);
        if let Some(character_id) = &equipped_by {
            equipped_relics += 1;
            if !account.characters.contains_key(character_id) {
                return Err(AccountImportError::new(
                    AccountImportErrorCode::InvalidEquipmentRelation,
                    Some(format!("{path}.location")),
                    "遗器装备关系引用了不存在的角色",
                ));
            }
            if !equipment_slots.insert((character_id.clone(), slot)) {
                return Err(AccountImportError::new(
                    AccountImportErrorCode::InvalidEquipmentRelation,
                    Some(format!("{path}.location")),
                    "同一角色同一部位重复装备遗器",
                ));
            }
        }

        let usable = raw_relic.rarity == 5
            && raw_relic.level.is_multiple_of(3)
            && (3..=4).contains(&substats.len())
            && (raw_relic.level == 0 || substats.len() == 4);
        if !usable {
            skipped_relics += 1;
            continue;
        }
        if equipped_by.is_some() {
            imported_equipped_relics += 1;
        }
        let relic = Relic {
            id: raw_relic.id.clone(),
            slot,
            set_id: raw_relic.set_id,
            rarity: raw_relic.rarity,
            level: raw_relic.level,
            main_stat,
            substats,
            equipped_by,
            locked: raw_relic.lock,
            discarded: raw_relic.discard,
        };
        account.relics.insert(raw_relic.id, relic);
    }
    if account.relics.is_empty() {
        return Err(AccountImportError::new(
            AccountImportErrorCode::NoUsableRelics,
            Some("relics".into()),
            "没有可进入当前五星 +3 强化模型的遗器",
        ));
    }

    let summary = AccountImportSummary {
        source: "reliquary_archiver".into(),
        version: 4,
        build: raw.build,
        characters: account.characters.len(),
        relics_in_file: source_relics,
        relics_imported: account.relics.len(),
        relics_skipped: skipped_relics,
        light_cones: source_light_cones,
        equipped_relics,
        imported_equipped_relics,
        equipped_light_cones,
        equipment_relations_recognized: true,
    };
    Ok(ImportedAccount { account, summary })
}
