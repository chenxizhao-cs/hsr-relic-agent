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
        "strategy":r.strategy,
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
        "cultivation_intent":e.goal().map(|g| json!({"character_id":g.character_id,
            "preferences":g.preferences,"strategy":g.strategy(),
            "strategy_explanation":g.strategy().explanation()})),
        "inventory":inventory,"recommendations":recommendations,"selected_evaluation":selected_evaluation,
        "remaining_budget":a.upgrade_steps,"history":history,"last_result":data.last_result,
        "account_summary":data.import_summary,
        "model_config":data.model_config.view(),"usage":{"summary":usage,"calls":data.usage.records()},
        "last_agent":data.last_agent}),
    )
}

pub(super) fn operation_error(error: &RelicOperationError) -> (&'static str, String) {
    (error.code(), error.message())
}
