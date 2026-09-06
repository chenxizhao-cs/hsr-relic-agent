use crate::{AccountState, CultivationGoal, Error, Relic, Result, Slot, Stat};

#[derive(Debug, Clone, Copy)]
pub struct Evaluation {
    pub current_score: f64,
    pub projected_score: f64,
}

/// Implementations may fail (e.g. a future external tool). They never mutate account state.
pub trait Evaluator {
    fn evaluate(
        &self,
        account: &AccountState,
        goal: &CultivationGoal,
        relic: &Relic,
    ) -> Result<Evaluation>;

    fn name(&self) -> &'static str {
        "Mock"
    }

    /// Threshold in the evaluator's score units, not a game-derived optimum.
    fn minimum_potential(&self) -> f64 {
        4.0
    }

    fn details(
        &self,
        _account: &AccountState,
        _goal: &CultivationGoal,
        _relic: &Relic,
    ) -> Result<Option<crate::EvaluationDetails>> {
        Ok(None)
    }
}

impl<T: Evaluator + ?Sized> Evaluator for Box<T> {
    fn evaluate(&self, a: &AccountState, g: &CultivationGoal, r: &Relic) -> Result<Evaluation> {
        (**self).evaluate(a, g, r)
    }
    fn name(&self) -> &'static str {
        (**self).name()
    }
    fn minimum_potential(&self) -> f64 {
        (**self).minimum_potential()
    }
    fn details(
        &self,
        a: &AccountState,
        g: &CultivationGoal,
        r: &Relic,
    ) -> Result<Option<crate::EvaluationDetails>> {
        (**self).details(a, g, r)
    }
}

/// Deliberately small heuristic, not Fribbels scoring, a probability model, or DPS.
pub struct MockEvaluator;

impl MockEvaluator {
    fn weight(character: &str, stat: Stat) -> f64 {
        match stat {
            Stat::CritRate | Stat::CritDamage | Stat::Speed => 1.0,
            Stat::HpPercent if character == "1205" => 1.0,
            Stat::AtkPercent if character == "1102" => 1.0,
            _ => 0.0,
        }
    }

    fn unit(stat: Stat) -> f64 {
        match stat {
            Stat::CritRate => 3.0,
            Stat::CritDamage => 6.0,
            Stat::Speed => 2.5,
            _ => 4.0,
        }
    }

    fn matching_main(character: &str, relic: &Relic) -> bool {
        let scaling = if character == "1205" {
            Stat::HpPercent
        } else {
            Stat::AtkPercent
        };
        let element = if character == "1205" {
            Stat::WindDamage
        } else {
            Stat::QuantumDamage
        };
        match relic.slot {
            Slot::Head | Slot::Hands => true,
            Slot::Body => matches!(relic.main_stat, Stat::CritRate | Stat::CritDamage),
            Slot::Feet => relic.main_stat == scaling || relic.main_stat == Stat::Speed,
            Slot::Sphere => relic.main_stat == scaling || relic.main_stat == element,
            Slot::Rope => relic.main_stat == scaling,
        }
    }
}

impl Evaluator for MockEvaluator {
    fn evaluate(
        &self,
        account: &AccountState,
        goal: &CultivationGoal,
        relic: &Relic,
    ) -> Result<Evaluation> {
        if !account.characters.contains_key(&goal.character_id)
            || !matches!(goal.character_id.as_str(), "1205" | "1102")
        {
            return Err(Error(
                "MockEvaluator 仅支持账号中的 Blade(1205) / Seele(1102)".into(),
            ));
        }
        if relic.level > 15
            || !relic.level.is_multiple_of(3)
            || relic.rarity != 5
            || !(3..=4).contains(&relic.substats.len())
            || (relic.level >= 3 && relic.substats.len() != 4)
            || relic.substats.iter().any(|(&stat, &value)| {
                !stat.is_substat() || stat == relic.main_stat || !value.is_finite() || value <= 0.0
            })
        {
            return Err(Error("MockEvaluator 需要有效的五星遗器和 +3 检查点".into()));
        }
        let id = goal.character_id.as_str();
        let score: f64 = relic
            .substats
            .iter()
            .map(|(&stat, &value)| value / Self::unit(stat) * Self::weight(id, stat))
            .sum();
        let preferred = if id == "1205" {
            ["113", "306"]
        } else {
            ["108", "309"]
        };
        let set_bonus = if preferred.contains(&relic.set_id.as_str()) {
            2.0
        } else {
            0.0
        };
        let main_factor = if Self::matching_main(id, relic) {
            1.0
        } else {
            0.25
        };
        let known_weights: f64 = relic
            .substats
            .keys()
            .map(|&stat| Self::weight(id, stat))
            .sum();
        let remaining = (15 - relic.level) / 3;
        // A missing fourth substat gets a fixed 0.5 heuristic, not a future observation.
        let mean_weight = (known_weights + (4 - relic.substats.len()) as f64 * 0.5) / 4.0;
        let current_score = (score + set_bonus) * main_factor;
        Ok(Evaluation {
            current_score,
            projected_score: current_score + f64::from(remaining) * mean_weight * main_factor,
        })
    }
}
