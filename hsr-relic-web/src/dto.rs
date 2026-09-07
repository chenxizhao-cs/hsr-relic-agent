use super::*;

pub(super) fn decision(d: UpgradeDecision) -> &'static str {
    match d {
        UpgradeDecision::Continue => "Continue",
        UpgradeDecision::Hold => "Hold",
        UpgradeDecision::Stop => "Stop",
    }
}
fn relic(r: &Relic) -> Value {
    json!({"id":r.id,"slot":r.slot,"set_id":r.set_id,"rarity":r.rarity,"level":r.level,
        "main_stat":r.main_stat,"substats":r.substats,"equipped_by":r.equipped_by,"locked":r.locked,"discarded":r.discarded})
}
pub(super) fn snapshot(data: &SessionData) -> ApiResult<Value> {
    let e = &data.engine;
    let a = e.account();
    let characters: Vec<_> = a
        .characters
        .values()
        .map(|c| json!({"id":c.id,"name":c.name,"level":c.level,"eidolon":c.eidolon,
            "has_light_cone":c.light_cone.is_some(),"evaluation_ready":data.evaluator.supports_character(c)}))
        .collect();
    let mut inventory = vec![];
    for r in a.relics.values() {
        // Ask the existing core for selection restrictions; no Web copy of eligibility rules.
        let blocked = e.check_relic_selectable(&r.id).err().map(|error| {
            let (code, message) = operation_error(&error);
            json!({"code":code,"message":message})
        });
        let status = e
            .goal()
            .and_then(|g| a.decisions.get(&(g.character_id.clone(), r.id.clone())))
            .copied()
            .map(decision);
        let mut row = relic(r);
        row["blocked"] = json!(blocked);
        row["decision"] = json!(status);
        row["set_match"] = json!(e.set_match(r));
        inventory.push(row);
    }
    let recommendations = if e.goal().is_some() {
        e.rank_candidates()?
    } else {
        vec![]
    };
    let recommendations: Vec<_> = recommendations.into_iter().map(|r| json!({"relic_id":r.relic_id,
        "set_match":r.set_match,
        "current_score":r.current_score,"projected_score":r.projected_score,"baseline_score":r.baseline_score,
        "priority":r.priority,"reason":r.reason,"details":r.details})).collect();
    let selected_evaluation = match (e.goal(), e.selected()) {
        (Some(g), Some(r)) => {
            let score = data.evaluator.evaluate(a, g, r)?;
            Some(
                json!({"current_score":score.current_score,"projected_score":score.projected_score,"details":data.evaluator.details(a,g,r)?}),
            )
        }
        _ => None,
    };
    let history: Vec<_> = a.history.iter().map(|r| json!({"character_id":r.goal.character_id,"relic_id":r.after.id,
        "before_level":r.before.level,"after_level":r.after.level,"stat":r.result.stat,"increase":r.result.increase,
        "decision":decision(r.decision),"reason":r.reason})).collect();
    let usage = data.usage.summary();
    Ok(
        json!({"revision":data.revision,"evaluator":data.evaluator.name(),"characters":characters,
        "target_id":e.goal().map(|g| &g.character_id),"selected_id":e.selected().map(|r| &r.id),
        "inventory":inventory,"recommendations":recommendations,"selected_evaluation":selected_evaluation,
        "remaining_budget":a.upgrade_steps,"history":history,"last_result":data.last_result,
        "account_summary":data.import_summary,
        "model_config":data.model_config.view(),"usage":{"summary":usage,"calls":data.usage.records()},
        "last_agent":data.last_agent}),
    )
}

pub(super) fn operation_error(error: &RelicOperationError) -> (&'static str, String) {
    use RelicOperationError::*;
    let (code, message) = match error {
        TargetNotSelected => ("target_not_selected", "请先选择培养角色。".into()),
        RelicNotFound { relic_id } => ("relic_not_found", format!("找不到遗器 {relic_id}。")),
        NoRelicSelected => ("no_relic_selected", "请先选择一件遗器。".into()),
        DifferentRelicSelected { .. } => (
            "selection_changed",
            "当前选择已改变，请核对遗器后再录入。".into(),
        ),
        BudgetExhausted => (
            "budget_exhausted",
            "本次强化预算已用完，可以重置 Demo 再试。".into(),
        ),
        Discarded { .. } => ("discarded", "这件遗器已标记弃置。".into()),
        Locked { .. } => ("locked", "这件遗器已锁定，受到装备保护。".into()),
        MaxLevel { .. } => ("max_level", "这件遗器已经满级。".into()),
        EquippedByOtherCharacter { .. } => (
            "equipped_elsewhere",
            "这件遗器已装备在其他角色身上。".into(),
        ),
        SetNotRecommendedForTarget { .. } => (
            "set_not_recommended",
            "这件遗器的套装未列入当前角色的游戏静态推荐。".into(),
        ),
        StoppedForTarget { .. } => (
            "stopped_for_target",
            "对当前角色已判定 Stop，请选择其他候选。".into(),
        ),
        HoldRequiresExplicitResume { .. } => (
            "hold_requires_resume",
            "这件遗器处于 Hold，请点击恢复观察再强化。".into(),
        ),
        StaleUpgradeResult { .. } => (
            "stale_upgrade",
            "遗器等级已改变，这次过期或重复结果未被录入。".into(),
        ),
        InvalidIncrease => ("invalid_increase", "请输入大于 0 的有限增量。".into()),
        InvalidSubstat { .. } => ("invalid_substat", "该属性不能作为副属性强化。".into()),
        MainStatConflict { .. } => (
            "main_stat_conflict",
            "主属性不能同时作为副属性录入。".into(),
        ),
        MustAddFourthSubstat => (
            "must_add_fourth",
            "当前只有三条副属性，请录入新增的第四条副属性。".into(),
        ),
        MustUpgradeExistingSubstat => (
            "must_upgrade_existing",
            "已有四条副属性，只能增加其中一条。".into(),
        ),
        InvalidSubstatCount { .. } => (
            "invalid_substat_count",
            "副属性数量不符合当前强化规则。".into(),
        ),
        StatOverflow { .. } => ("stat_overflow", "增量过大，结果无法表示。".into()),
        EvaluationFailed(error) => ("evaluation_failed", format!("重新评价失败：{error}")),
    };
    (code, message)
}
