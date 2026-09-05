use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stat {
    Hp,
    Atk,
    Def,
    HpPercent,
    AtkPercent,
    DefPercent,
    Speed,
    CritRate,
    CritDamage,
    EffectHit,
    EffectRes,
    BreakEffect,
    EnergyRegen,
    Healing,
    PhysicalDamage,
    FireDamage,
    IceDamage,
    LightningDamage,
    WindDamage,
    QuantumDamage,
    ImaginaryDamage,
}

impl Stat {
    pub fn is_substat(self) -> bool {
        matches!(
            self,
            Self::Hp
                | Self::Atk
                | Self::Def
                | Self::HpPercent
                | Self::AtkPercent
                | Self::DefPercent
                | Self::Speed
                | Self::CritRate
                | Self::CritDamage
                | Self::EffectHit
                | Self::EffectRes
                | Self::BreakEffect
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Head,
    Hands,
    Body,
    Feet,
    Sphere,
    Rope,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LightCone {
    pub id: String,
    pub level: u8,
    pub superimposition: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Character {
    pub id: String,
    pub name: String,
    pub level: u8,
    pub eidolon: u8,
    pub light_cone: Option<LightCone>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Relic {
    pub id: String,
    pub slot: Slot,
    pub set_id: String,
    pub rarity: u8,
    pub level: u8,
    pub main_stat: Stat,
    pub substats: BTreeMap<Stat, f64>,
    pub equipped_by: Option<String>,
    pub locked: bool,
    pub discarded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CultivationGoal {
    pub character_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeDecision {
    Continue,
    Hold,
    Stop,
}

/// One observed +3 event. `increase` is an absolute stat delta, in percentage
/// points for percentage stats. The caller supplies the old level to reject stale input.
#[derive(Debug, Clone, PartialEq)]
pub struct UpgradeResult {
    pub relic_id: String,
    pub expected_level: u8,
    pub stat: Stat,
    pub increase: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpgradeRecord {
    pub goal: CultivationGoal,
    pub before: Relic,
    pub after: Relic,
    pub result: UpgradeResult,
    pub useful: bool,
    pub decision: UpgradeDecision,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccountState {
    pub characters: BTreeMap<String, Character>,
    pub relics: BTreeMap<String, Relic>,
    /// Demo budget only: each accepted +3 event consumes one step, not real materials.
    pub upgrade_steps: u32,
    pub history: Vec<UpgradeRecord>,
    /// Decisions are scoped to a target, so changing goals can reconsider a relic.
    pub decisions: BTreeMap<(String, String), UpgradeDecision>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpgradeRecommendation {
    pub relic_id: String,
    pub current_score: f64,
    pub projected_score: f64,
    pub baseline_score: f64,
    pub priority: f64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpgradeOutcome {
    pub decision: UpgradeDecision,
    pub reason: String,
    pub next: Option<UpgradeRecommendation>,
}
