//! Only this module knows scanner v4 field names. Unused fields (including previews)
//! are ignored; an observed upgrade must always be entered explicitly.
use crate::*;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct ScannerAccount {
    source: String,
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
    id: String,
    level: u8,
    superimposition: u8,
    location: String,
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
        if lc.location.is_empty() {
            continue;
        }
        let character = account
            .characters
            .get_mut(&lc.location)
            .ok_or_else(|| Error(format!("光锥装备角色不存在: {}", lc.location)))?;
        if lc.id.is_empty()
            || !(1..=80).contains(&lc.level)
            || !(1..=5).contains(&lc.superimposition)
            || character.light_cone.is_some()
        {
            return Err(Error("光锥字段或装备关系无效".into()));
        }
        character.light_cone = Some(LightCone {
            id: lc.id,
            level: lc.level,
            superimposition: lc.superimposition,
        });
    }
    for r in raw.relics {
        let slot = match r.slot.replace(' ', "").as_str() {
            "Head" => Slot::Head,
            "Hands" => Slot::Hands,
            "Body" => Slot::Body,
            "Feet" => Slot::Feet,
            "PlanarSphere" => Slot::Sphere,
            "LinkRope" => Slot::Rope,
            _ => return Err(Error(format!("未知部位: {}", r.slot))),
        };
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
