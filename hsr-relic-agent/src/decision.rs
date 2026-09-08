use crate::*;
use std::sync::Arc;

pub type RelicOperationResult<T> = std::result::Result<T, RelicOperationError>;

#[derive(Debug, Clone, PartialEq)]
pub enum RelicOperationError {
    TargetNotSelected,
    RelicNotFound {
        relic_id: String,
    },
    NoRelicSelected,
    DifferentRelicSelected {
        selected_relic_id: String,
        result_relic_id: String,
    },
    BudgetExhausted,
    Discarded {
        relic_id: String,
    },
    Locked {
        relic_id: String,
    },
    MaxLevel {
        relic_id: String,
        level: u8,
    },
    EquippedByOtherCharacter {
        relic_id: String,
        character_id: String,
    },
    SetNotRecommendedForTarget {
        relic_id: String,
        character_id: String,
        set_id: String,
    },
    StoppedForTarget {
        relic_id: String,
        character_id: String,
    },
    HoldRequiresExplicitResume {
        relic_id: String,
        character_id: String,
    },
    StaleUpgradeResult {
        relic_id: String,
        reported_level: u8,
        current_level: u8,
    },
    InvalidLevelTransition {
        from_level: u8,
        to_level: u8,
    },
    InvalidIncrease,
    InvalidSubstat {
        stat: Stat,
    },
    MainStatConflict {
        stat: Stat,
    },
    MustAddFourthSubstat,
    MustUpgradeExistingSubstat,
    InvalidSubstatCount {
        count: usize,
    },
    StatOverflow {
        stat: Stat,
    },
    EvaluationFailed(Error),
}

impl std::fmt::Display for RelicOperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for RelicOperationError {}

impl RelicOperationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::TargetNotSelected => "target_not_selected",
            Self::RelicNotFound { .. } => "relic_not_found",
            Self::NoRelicSelected => "no_relic_selected",
            Self::DifferentRelicSelected { .. } => "selection_changed",
            Self::BudgetExhausted => "budget_exhausted",
            Self::Discarded { .. } => "discarded",
            Self::Locked { .. } => "locked",
            Self::MaxLevel { .. } => "max_level",
            Self::EquippedByOtherCharacter { .. } => "equipped_elsewhere",
            Self::SetNotRecommendedForTarget { .. } => "set_not_recommended",
            Self::StoppedForTarget { .. } => "stopped_for_target",
            Self::HoldRequiresExplicitResume { .. } => "hold_requires_resume",
            Self::StaleUpgradeResult { .. } => "stale_upgrade",
            Self::InvalidLevelTransition { .. } => "invalid_level_transition",
            Self::InvalidIncrease => "invalid_increase",
            Self::InvalidSubstat { .. } => "invalid_substat",
            Self::MainStatConflict { .. } => "main_stat_conflict",
            Self::MustAddFourthSubstat => "must_add_fourth",
            Self::MustUpgradeExistingSubstat => "must_upgrade_existing",
            Self::InvalidSubstatCount { .. } => "invalid_substat_count",
            Self::StatOverflow { .. } => "stat_overflow",
            Self::EvaluationFailed(_) => "evaluation_failed",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::TargetNotSelected => "请先选择培养角色。".into(),
            Self::RelicNotFound { relic_id } => format!("找不到遗器 {relic_id}。"),
            Self::NoRelicSelected => "请先选择一件遗器。".into(),
            Self::DifferentRelicSelected { .. } => "当前选择已改变，请核对遗器后再录入。".into(),
            Self::BudgetExhausted => "本次强化预算已用完，可以重置 Demo 再试。".into(),
            Self::Discarded { .. } => "这件遗器已标记弃置。".into(),
            Self::Locked { .. } => "这件遗器已锁定，受到装备保护。".into(),
            Self::MaxLevel { .. } => "这件遗器已经满级。".into(),
            Self::EquippedByOtherCharacter { .. } => "这件遗器已装备在其他角色身上。".into(),
            Self::SetNotRecommendedForTarget { .. } => {
                "这件遗器的套装未列入当前角色的游戏静态推荐。".into()
            }
            Self::StoppedForTarget { .. } => "对当前角色已判定 Stop，请选择其他候选。".into(),
            Self::HoldRequiresExplicitResume { .. } => {
                "这件遗器处于 Hold，需要显式恢复后才能继续强化。".into()
            }
            Self::StaleUpgradeResult { .. } => {
                "遗器等级已改变，这次过期、重复或非法跨级结果未被录入。".into()
            }
            Self::InvalidLevelTransition {
                from_level,
                to_level,
            } => format!("一次观察只能强化 3 级，不能从 +{from_level} 直接记录为 +{to_level}。"),
            Self::InvalidIncrease => "请输入大于 0 的有限增量。".into(),
            Self::InvalidSubstat { .. } => "该属性不能作为副属性强化。".into(),
            Self::MainStatConflict { .. } => "主属性不能同时作为副属性录入。".into(),
            Self::MustAddFourthSubstat => "当前只有三条副属性，请录入新增的第四条副属性。".into(),
            Self::MustUpgradeExistingSubstat => "已有四条副属性，只能增加其中一条。".into(),
            Self::InvalidSubstatCount { .. } => "副属性数量不符合当前强化规则。".into(),
            Self::StatOverflow { .. } => "增量过大，结果无法表示。".into(),
            Self::EvaluationFailed(error) => format!("重新评价失败：{error}"),
        }
    }
}

impl From<Error> for RelicOperationError {
    fn from(error: Error) -> Self {
        Self::EvaluationFailed(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelicSelection {
    Selected { relic_id: String },
    ResumedFromHold { relic_id: String },
}

#[derive(Clone)]
pub struct DecisionEngine<E> {
    account: AccountState,
    evaluator: E,
    goal: Option<CultivationGoal>,
    selected: Option<String>,
    recommendation_database: Option<Arc<CharacterRelicDatabase>>,
}

impl<E: Evaluator> DecisionEngine<E> {
    pub fn new(account: AccountState, evaluator: E) -> Self {
        Self {
            account,
            evaluator,
            goal: None,
            selected: None,
            recommendation_database: None,
        }
    }

    pub fn restore(
        account: AccountState,
        evaluator: E,
        goal: Option<CultivationGoal>,
        selected_relic_id: Option<String>,
    ) -> Result<Self> {
        for (id, character) in &account.characters {
            if id != &character.id {
                return Err(Error("角色索引与角色 ID 不一致".into()));
            }
        }
        for (uid, light_cone) in &account.light_cones {
            if uid != &light_cone.uid
                || light_cone.id.is_empty()
                || !(1..=80).contains(&light_cone.level)
                || !(1..=5).contains(&light_cone.superimposition)
            {
                return Err(Error(format!("光锥 {uid} 的字段无效")));
            }
            if let Some(character_id) = &light_cone.equipped_by {
                let character = account
                    .characters
                    .get(character_id)
                    .ok_or_else(|| Error(format!("光锥 {uid} 装备于不存在的角色")))?;
                let equipped = LightCone {
                    id: light_cone.id.clone(),
                    level: light_cone.level,
                    superimposition: light_cone.superimposition,
                };
                if character.light_cone.as_ref() != Some(&equipped) {
                    return Err(Error(format!("光锥 {uid} 的角色装备关系不一致")));
                }
            }
        }
        for (id, relic) in &account.relics {
            if id != &relic.id {
                return Err(Error("遗器索引与遗器 ID 不一致".into()));
            }
            if relic.level > 15
                || relic.level % 3 != 0
                || !(3..=4).contains(&relic.substats.len())
                || relic.substats.iter().any(|(stat, value)| {
                    !stat.is_substat()
                        || *stat == relic.main_stat
                        || !value.is_finite()
                        || *value <= 0.0
                })
            {
                return Err(Error(format!("遗器 {id} 的等级或副属性数量无效")));
            }
            if let Some(character_id) = &relic.equipped_by
                && !account.characters.contains_key(character_id)
            {
                return Err(Error(format!("遗器 {id} 装备于不存在的角色")));
            }
        }
        for (character_id, relic_id) in account.decisions.keys() {
            if !account.characters.contains_key(character_id)
                || !account.relics.contains_key(relic_id)
            {
                return Err(Error("遗器决策引用了不存在的角色或遗器".into()));
            }
        }
        for record in &account.history {
            if !account.characters.contains_key(&record.goal.character_id)
                || !account.relics.contains_key(&record.after.id)
                || record.before.id != record.after.id
                || record.result.relic_id != record.after.id
            {
                return Err(Error("强化历史引用了不存在或不一致的状态".into()));
            }
        }
        if let Some(goal) = &goal
            && !account.characters.contains_key(&goal.character_id)
        {
            return Err(Error("培养目标不在账号中".into()));
        }
        if let Some(relic_id) = &selected_relic_id {
            if goal.is_none() {
                return Err(Error("没有培养目标时不能恢复选中遗器".into()));
            }
            if !account.relics.contains_key(relic_id) {
                return Err(Error("选中遗器不在账号中".into()));
            }
        }
        Ok(Self {
            account,
            evaluator,
            goal,
            selected: selected_relic_id,
            recommendation_database: None,
        })
    }

    pub fn with_recommendation_database(mut self, database: Arc<CharacterRelicDatabase>) -> Self {
        self.recommendation_database = Some(database);
        self
    }

    pub fn recommendation_database(&self) -> Option<&CharacterRelicDatabase> {
        self.recommendation_database.as_deref()
    }

    pub fn recommendation_database_handle(&self) -> Option<Arc<CharacterRelicDatabase>> {
        self.recommendation_database.clone()
    }

    pub fn set_match(&self, relic: &Relic) -> RecommendationMatch {
        match (&self.recommendation_database, &self.goal) {
            (Some(database), Some(goal)) => database.set_match(&goal.character_id, relic),
            _ => RecommendationMatch::Unknown,
        }
    }

    pub fn account(&self) -> &AccountState {
        &self.account
    }
    pub fn goal(&self) -> Option<&CultivationGoal> {
        self.goal.as_ref()
    }
    pub fn selected(&self) -> Option<&Relic> {
        self.selected
            .as_ref()
            .and_then(|id| self.account.relics.get(id))
    }

    pub fn set_goal(&mut self, character_id: &str) -> Result<()> {
        self.set_cultivation_goal(CultivationGoal::balanced(character_id))
    }

    pub fn set_cultivation_goal(&mut self, goal: CultivationGoal) -> Result<()> {
        if !self.account.characters.contains_key(&goal.character_id) {
            return Err(Error(format!("账号中没有角色 {}", goal.character_id)));
        }
        // Validate evaluator support before changing the active goal.
        for relic in self.account.relics.values() {
            self.evaluate(&self.account, &goal, relic)?;
        }
        self.goal = Some(goal);
        self.selected = None;
        Ok(())
    }

    fn required_goal(&self) -> Result<&CultivationGoal> {
        self.goal
            .as_ref()
            .ok_or_else(|| Error("请先选择目标角色".into()))
    }

    fn operation_goal(&self) -> RelicOperationResult<&CultivationGoal> {
        self.goal
            .as_ref()
            .ok_or(RelicOperationError::TargetNotSelected)
    }

    fn operation_block(
        &self,
        relic: &Relic,
        goal: &CultivationGoal,
    ) -> Option<RelicOperationError> {
        if self.account.upgrade_steps == 0 {
            return Some(RelicOperationError::BudgetExhausted);
        }
        if relic.discarded {
            return Some(RelicOperationError::Discarded {
                relic_id: relic.id.clone(),
            });
        }
        if relic.locked {
            return Some(RelicOperationError::Locked {
                relic_id: relic.id.clone(),
            });
        }
        if relic.level >= 15 {
            return Some(RelicOperationError::MaxLevel {
                relic_id: relic.id.clone(),
                level: relic.level,
            });
        }
        if let Some(character_id) = relic
            .equipped_by
            .as_ref()
            .filter(|id| *id != &goal.character_id)
        {
            return Some(RelicOperationError::EquippedByOtherCharacter {
                relic_id: relic.id.clone(),
                character_id: character_id.clone(),
            });
        }
        if self
            .recommendation_database
            .as_ref()
            .is_some_and(|database| {
                database.set_match(&goal.character_id, relic) == RecommendationMatch::NotRecommended
            })
        {
            return Some(RelicOperationError::SetNotRecommendedForTarget {
                relic_id: relic.id.clone(),
                character_id: goal.character_id.clone(),
                set_id: relic.set_id.clone(),
            });
        }
        None
    }

    fn eligible(relic: &Relic, goal: &CultivationGoal) -> bool {
        relic.level < 15
            && !relic.locked
            && !relic.discarded
            && relic
                .equipped_by
                .as_ref()
                .is_none_or(|id| id == &goal.character_id)
    }

    fn eligible_for_goal(&self, relic: &Relic, goal: &CultivationGoal) -> bool {
        Self::eligible(relic, goal)
            && self
                .recommendation_database
                .as_ref()
                .is_none_or(|database| {
                    database.set_match(&goal.character_id, relic)
                        != RecommendationMatch::NotRecommended
                })
    }

    fn evaluate(
        &self,
        account: &AccountState,
        goal: &CultivationGoal,
        relic: &Relic,
    ) -> Result<Evaluation> {
        let evaluation = self.evaluator.evaluate(account, goal, relic)?;
        if !evaluation.current_score.is_finite()
            || !evaluation.projected_score.is_finite()
            || evaluation.current_score < 0.0
            || evaluation.projected_score < evaluation.current_score
        {
            return Err(Error(format!(
                "Evaluator 返回无效评分：当前 {:.4}，预计 {:.4}",
                evaluation.current_score, evaluation.projected_score
            )));
        }
        Ok(evaluation)
    }

    fn recommendation(
        &self,
        account: &AccountState,
        goal: &CultivationGoal,
        relic: &Relic,
    ) -> Result<UpgradeRecommendation> {
        let evaluation = self.evaluate(account, goal, relic)?;
        let details = self.evaluator.details(account, goal, relic)?;
        let mut baseline: f64 = 0.0;
        for equipped in account
            .relics
            .values()
            .filter(|r| r.slot == relic.slot && r.equipped_by.as_ref() == Some(&goal.character_id))
        {
            baseline = baseline.max(self.evaluate(account, goal, equipped)?.current_score);
        }
        if let Some(metrics) = &details {
            baseline = metrics.baseline_slot_score;
        }
        let remaining = f64::from((15 - relic.level) / 3);
        let damage_ratio = details
            .as_ref()
            .map_or(1.0, EvaluationDetails::damage_ratio);
        if !baseline.is_finite()
            || baseline < 0.0
            || !damage_ratio.is_finite()
            || damage_ratio < 0.0
        {
            return Err(Error("Evaluator 返回无效 Build 比较指标".into()));
        }
        let average_gain = (evaluation.projected_score - baseline).max(0.0);
        let strategy = goal.strategy();
        let (priority, strategy_calculation) = match strategy {
            CultivationStrategy::Conservative => {
                let readiness = if evaluation.projected_score > 0.0 {
                    (evaluation.current_score / evaluation.projected_score).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let priority =
                    average_gain / remaining.powf(1.5) * (0.5 + readiness * 0.5) * damage_ratio;
                (
                    priority,
                    format!(
                        "平均正收益 {:.2} / 剩余 {:.0} 步^1.5 × 当前成熟度权重 {:.3} × Build 伤害比 {:.3}",
                        average_gain,
                        remaining,
                        0.5 + readiness * 0.5,
                        damage_ratio
                    ),
                )
            }
            CultivationStrategy::Balanced => (
                average_gain / remaining * damage_ratio,
                format!(
                    "平均正收益 {:.2} / 剩余 {:.0} 步 × Build 伤害比 {:.3}",
                    average_gain, remaining, damage_ratio
                ),
            ),
            CultivationStrategy::HighPotential => {
                let ceiling = details
                    .as_ref()
                    .map_or(evaluation.projected_score, |metrics| metrics.relic.best)
                    .max(evaluation.projected_score);
                let ceiling_gain = (ceiling - baseline).max(0.0);
                (
                    ceiling_gain / remaining.sqrt() * damage_ratio,
                    format!(
                        "best 上限正收益 {:.2} / √剩余 {:.0} 步 × Build 伤害比 {:.3}",
                        ceiling_gain, remaining, damage_ratio
                    ),
                )
            }
        };
        let set_match = self
            .recommendation_database
            .as_ref()
            .map_or(RecommendationMatch::Unknown, |database| {
                database.set_match(&goal.character_id, relic)
            });
        let set_reason = match set_match {
            RecommendationMatch::Recommended => "；套装在游戏静态推荐中",
            RecommendationMatch::Unknown => "；当前角色没有静态套装数据",
            RecommendationMatch::NotRecommended => "；套装未列入游戏静态推荐",
        };
        let reason = format!(
            "{} 当前 {:.2}，预计满级 {:.2}，同部位参考基线 {:.2}；{}；策略优先级 {:.2}（{}）",
            self.evaluator.name(),
            evaluation.current_score,
            evaluation.projected_score,
            baseline,
            strategy_calculation,
            priority,
            strategy.explanation()
        ) + set_reason;
        Ok(UpgradeRecommendation {
            relic_id: relic.id.clone(),
            strategy,
            set_match,
            current_score: evaluation.current_score,
            projected_score: evaluation.projected_score,
            baseline_score: baseline,
            priority,
            details,
            reason,
        })
    }

    fn rank_account(
        &self,
        account: &AccountState,
        goal: &CultivationGoal,
    ) -> Result<Vec<UpgradeRecommendation>> {
        if account.upgrade_steps == 0 {
            return Ok(vec![]);
        }
        let mut candidates = vec![];
        for relic in account.relics.values() {
            if !self.eligible_for_goal(relic, goal) {
                continue;
            }
            let status = account
                .decisions
                .get(&(goal.character_id.clone(), relic.id.clone()));
            if matches!(status, Some(UpgradeDecision::Hold | UpgradeDecision::Stop)) {
                continue;
            }
            let candidate = self.recommendation(account, goal, relic)?;
            if candidate.projected_score >= self.evaluator.minimum_potential()
                && candidate.priority > 0.0
            {
                candidates.push(candidate);
            }
        }
        candidates.sort_by(|a, b| {
            b.priority
                .total_cmp(&a.priority)
                .then_with(|| a.relic_id.cmp(&b.relic_id))
        });
        Ok(candidates)
    }

    pub fn rank_candidates(&self) -> Result<Vec<UpgradeRecommendation>> {
        self.rank_account(&self.account, self.required_goal()?)
    }

    pub fn recommend_next(&mut self) -> Result<Option<UpgradeRecommendation>> {
        let next = self.rank_candidates()?.into_iter().next();
        self.selected = next.as_ref().map(|r| r.relic_id.clone());
        Ok(next)
    }

    pub fn recommend_alternative(&mut self) -> Result<Option<UpgradeRecommendation>> {
        let current = self.selected.clone();
        let next = self
            .rank_candidates()?
            .into_iter()
            .find(|candidate| Some(&candidate.relic_id) != current.as_ref());
        self.selected = next.as_ref().map(|candidate| candidate.relic_id.clone());
        Ok(next)
    }

    /// Explicit selection resumes Hold; Stop is excluded for this target for this session.
    pub fn check_relic_selectable(&self, relic_id: &str) -> RelicOperationResult<()> {
        let goal = self.operation_goal()?;
        let relic = self.account.relics.get(relic_id).ok_or_else(|| {
            RelicOperationError::RelicNotFound {
                relic_id: relic_id.into(),
            }
        })?;
        if self
            .account
            .decisions
            .get(&(goal.character_id.clone(), relic_id.into()))
            == Some(&UpgradeDecision::Stop)
        {
            return Err(RelicOperationError::StoppedForTarget {
                relic_id: relic_id.into(),
                character_id: goal.character_id.clone(),
            });
        }
        if let Some(reason) = self.operation_block(relic, goal) {
            return Err(reason);
        }
        Ok(())
    }

    pub fn select_relic(&mut self, relic_id: &str) -> RelicOperationResult<RelicSelection> {
        self.check_relic_selectable(relic_id)?;
        let goal = self.operation_goal()?.clone();
        let key = (goal.character_id.clone(), relic_id.into());
        let resumed = self.account.decisions.remove(&key) == Some(UpgradeDecision::Hold);
        self.selected = Some(relic_id.into());
        if resumed {
            Ok(RelicSelection::ResumedFromHold {
                relic_id: relic_id.into(),
            })
        } else {
            Ok(RelicSelection::Selected {
                relic_id: relic_id.into(),
            })
        }
    }

    pub fn apply_upgrade_observation(
        &mut self,
        result: UpgradeResult,
        resulting_level: u8,
    ) -> RelicOperationResult<UpgradeOutcome> {
        if result.expected_level.checked_add(3) != Some(resulting_level) {
            return Err(RelicOperationError::InvalidLevelTransition {
                from_level: result.expected_level,
                to_level: resulting_level,
            });
        }
        self.apply_upgrade(result)
    }

    /// Applies a single observed result transactionally. Validation/evaluator errors
    /// leave the relic, budget, decisions, selection, and history unchanged.
    pub fn apply_upgrade(&mut self, result: UpgradeResult) -> RelicOperationResult<UpgradeOutcome> {
        let goal = self.operation_goal()?.clone();
        let before = self
            .account
            .relics
            .get(&result.relic_id)
            .ok_or_else(|| RelicOperationError::RelicNotFound {
                relic_id: result.relic_id.clone(),
            })?
            .clone();
        let key = (goal.character_id.clone(), before.id.clone());
        match self.account.decisions.get(&key) {
            Some(UpgradeDecision::Stop) => {
                return Err(RelicOperationError::StoppedForTarget {
                    relic_id: before.id,
                    character_id: goal.character_id,
                });
            }
            Some(UpgradeDecision::Hold) => {
                return Err(RelicOperationError::HoldRequiresExplicitResume {
                    relic_id: before.id,
                    character_id: goal.character_id,
                });
            }
            _ => {}
        }
        match self.selected.as_deref() {
            None => return Err(RelicOperationError::NoRelicSelected),
            Some(selected) if selected != result.relic_id => {
                return Err(RelicOperationError::DifferentRelicSelected {
                    selected_relic_id: selected.into(),
                    result_relic_id: result.relic_id,
                });
            }
            Some(_) => {}
        }
        if let Some(reason) = self.operation_block(&before, &goal) {
            return Err(reason);
        }
        if before.level != result.expected_level {
            return Err(RelicOperationError::StaleUpgradeResult {
                relic_id: before.id,
                reported_level: result.expected_level,
                current_level: before.level,
            });
        }
        if !result.increase.is_finite() || result.increase <= 0.0 {
            return Err(RelicOperationError::InvalidIncrease);
        }
        if !result.stat.is_substat() {
            return Err(RelicOperationError::InvalidSubstat { stat: result.stat });
        }
        if result.stat == before.main_stat {
            return Err(RelicOperationError::MainStatConflict { stat: result.stat });
        }
        let exists = before.substats.contains_key(&result.stat);
        match (before.substats.len(), exists) {
            (3, true) => return Err(RelicOperationError::MustAddFourthSubstat),
            (4, false) => return Err(RelicOperationError::MustUpgradeExistingSubstat),
            (3 | 4, _) => {}
            (count, _) => return Err(RelicOperationError::InvalidSubstatCount { count }),
        }
        let mut after = before.clone();
        let value = after.substats.entry(result.stat).or_default();
        *value += result.increase;
        if !value.is_finite() {
            return Err(RelicOperationError::StatOverflow { stat: result.stat });
        }
        after.level += 3;

        let old_score = self.evaluate(&self.account, &goal, &before)?.current_score;
        let mut staged = self.account.clone();
        staged.relics.insert(after.id.clone(), after.clone());
        staged.upgrade_steps -= 1;
        let evaluation = self.evaluate(&staged, &goal, &after)?;
        let useful = evaluation.current_score > old_score + 1e-9;
        let misses = if useful {
            0
        } else {
            1 + staged
                .history
                .iter()
                .rev()
                .filter(|r| r.goal == goal && r.after.id == after.id)
                .take_while(|r| !r.useful)
                .count()
        };
        let (decision, reason) = if after.level == 15 {
            (
                UpgradeDecision::Hold,
                "已满级，保留遗器，停止追加投入".into(),
            )
        } else if evaluation.projected_score < self.evaluator.minimum_potential() {
            (
                UpgradeDecision::Stop,
                format!(
                    "预计满级分 {:.2} 低于 {} 演示阈值 {}；对当前目标停止投入",
                    evaluation.projected_score,
                    self.evaluator.name(),
                    self.evaluator.minimum_potential()
                ),
            )
        } else if misses >= 2 {
            (
                UpgradeDecision::Stop,
                format!("对当前目标连续无效强化 {misses} 次，达到阈值 2；停止投入"),
            )
        } else if staged.upgrade_steps == 0 {
            (
                UpgradeDecision::Hold,
                "演示强化步数预算耗尽，暂存当前遗器".into(),
            )
        } else if !useful {
            (
                UpgradeDecision::Hold,
                format!(
                    "本次 {} 评分未增加，暂停投入；可显式 choose 恢复观察",
                    self.evaluator.name()
                ),
            )
        } else {
            let current = self.recommendation(&staged, &goal, &after)?;
            let alternative = self
                .rank_account(&staged, &goal)?
                .into_iter()
                .find(|r| r.relic_id != after.id);
            if current.priority <= 0.0 {
                (
                    UpgradeDecision::Hold,
                    "预计不能超越已装备同部位基线，先保留".into(),
                )
            } else if alternative
                .as_ref()
                .is_some_and(|other| other.priority > current.priority * 1.25)
            {
                (
                    UpgradeDecision::Hold,
                    format!(
                        "其他候选 {} 的单位步数收益高出当前 25%，暂缓当前投入",
                        alternative.unwrap().relic_id
                    ),
                )
            } else {
                (
                    UpgradeDecision::Continue,
                    format!(
                        "本次 {} 分增加 {:.2}，仍有预算与正向替换收益，继续观察",
                        self.evaluator.name(),
                        evaluation.current_score - old_score
                    ),
                )
            }
        };
        staged
            .decisions
            .insert((goal.character_id.clone(), after.id.clone()), decision);
        staged.history.push(UpgradeRecord {
            goal: goal.clone(),
            before,
            after: after.clone(),
            result,
            useful,
            decision,
            reason: reason.clone(),
        });
        let next = if decision == UpgradeDecision::Continue {
            Some(self.recommendation(&staged, &goal, &after)?)
        } else {
            self.rank_account(&staged, &goal)?.into_iter().next()
        };
        let details = self.evaluator.details(&staged, &goal, &after)?;
        self.selected = next.as_ref().map(|r| r.relic_id.clone());
        self.account = staged;
        Ok(UpgradeOutcome {
            decision,
            reason,
            details,
            next,
        })
    }
}
