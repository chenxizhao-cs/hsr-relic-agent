use crate::*;

pub struct DecisionEngine<E> {
    account: AccountState,
    evaluator: E,
    goal: Option<CultivationGoal>,
    selected: Option<String>,
}

impl<E: Evaluator> DecisionEngine<E> {
    pub fn new(account: AccountState, evaluator: E) -> Self {
        Self {
            account,
            evaluator,
            goal: None,
            selected: None,
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
        if !self.account.characters.contains_key(character_id) {
            return Err(Error(format!("账号中没有角色 {character_id}")));
        }
        let goal = CultivationGoal {
            character_id: character_id.into(),
        };
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

    fn eligible(relic: &Relic, goal: &CultivationGoal) -> bool {
        relic.level < 15
            && !relic.locked
            && !relic.discarded
            && relic
                .equipped_by
                .as_ref()
                .is_none_or(|id| id == &goal.character_id)
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
            return Err(Error("Evaluator 返回无效评分".into()));
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
        let mut baseline: f64 = 0.0;
        for equipped in account
            .relics
            .values()
            .filter(|r| r.slot == relic.slot && r.equipped_by.as_ref() == Some(&goal.character_id))
        {
            baseline = baseline.max(self.evaluate(account, goal, equipped)?.current_score);
        }
        let remaining = f64::from((15 - relic.level) / 3);
        let priority = (evaluation.projected_score - baseline).max(0.0) / remaining;
        Ok(UpgradeRecommendation {
            relic_id: relic.id.clone(),
            current_score: evaluation.current_score,
            projected_score: evaluation.projected_score,
            baseline_score: baseline,
            priority,
            reason: format!(
                "Mock 当前 {:.2}，预计满级 {:.2}，已装备同部位基线 {:.2}；正收益 / 剩余 {:.0} 步 = {:.2}",
                evaluation.current_score, evaluation.projected_score, baseline, remaining, priority
            ),
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
            if !Self::eligible(relic, goal) {
                continue;
            }
            let status = account
                .decisions
                .get(&(goal.character_id.clone(), relic.id.clone()));
            if matches!(status, Some(UpgradeDecision::Hold | UpgradeDecision::Stop)) {
                continue;
            }
            let candidate = self.recommendation(account, goal, relic)?;
            if candidate.projected_score >= 4.0 && candidate.priority > 0.0 {
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

    /// Explicit selection resumes Hold; Stop is excluded for this target for this session.
    pub fn select_relic(&mut self, relic_id: &str) -> Result<()> {
        let goal = self.required_goal()?.clone();
        let relic = self
            .account
            .relics
            .get(relic_id)
            .ok_or_else(|| Error("遗器不存在".into()))?;
        if self.account.upgrade_steps == 0 || !Self::eligible(relic, &goal) {
            return Err(Error(
                "预算耗尽，或遗器已满级、锁定、标记丢弃、装备于其他角色".into(),
            ));
        }
        let key = (goal.character_id, relic_id.into());
        if self.account.decisions.get(&key) == Some(&UpgradeDecision::Stop) {
            return Err(Error("这件遗器已对当前目标 Stop；请推荐其他候选".into()));
        }
        self.account.decisions.remove(&key);
        self.selected = Some(relic_id.into());
        Ok(())
    }

    /// Applies a single observed result transactionally. Validation/evaluator errors
    /// leave the relic, budget, decisions, selection, and history unchanged.
    pub fn apply_upgrade(&mut self, result: UpgradeResult) -> Result<UpgradeOutcome> {
        let goal = self.required_goal()?.clone();
        if self.selected.as_deref() != Some(&result.relic_id) {
            return Err(Error("强化结果必须对应当前选中遗器".into()));
        }
        let before = self
            .account
            .relics
            .get(&result.relic_id)
            .ok_or_else(|| Error("遗器不存在".into()))?
            .clone();
        if self.account.upgrade_steps == 0 || !Self::eligible(&before, &goal) {
            return Err(Error("当前不能强化".into()));
        }
        if before.level != result.expected_level {
            return Err(Error("旧等级不匹配：拒绝重复或过期的强化结果".into()));
        }
        if !result.increase.is_finite()
            || result.increase <= 0.0
            || !result.stat.is_substat()
            || result.stat == before.main_stat
        {
            return Err(Error("副属性及增量无效".into()));
        }
        let exists = before.substats.contains_key(&result.stat);
        if (before.substats.len() == 3 && exists) || (before.substats.len() == 4 && !exists) {
            return Err(Error(
                "三词条时下一步必须新增第四词条；四词条时只能增加已有词条".into(),
            ));
        }
        let mut after = before.clone();
        let value = after.substats.entry(result.stat).or_default();
        *value += result.increase;
        if !value.is_finite() {
            return Err(Error("属性数值溢出".into()));
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
        } else if evaluation.projected_score < 4.0 {
            (
                UpgradeDecision::Stop,
                format!(
                    "预计满级分 {:.2} 低于阈值 4；对当前目标停止投入",
                    evaluation.projected_score
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
                "本次 Mock 评分未增加，暂停投入；可显式 choose 恢复观察".into(),
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
                        "本次 Mock 分增加 {:.2}，仍有预算与正向替换收益，继续观察",
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
        self.selected = next.as_ref().map(|r| r.relic_id.clone());
        self.account = staged;
        Ok(UpgradeOutcome {
            decision,
            reason,
            next,
        })
    }
}
