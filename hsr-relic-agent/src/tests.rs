use crate::*;

fn engine(steps: u32) -> DecisionEngine<MockEvaluator> {
    let mut engine =
        DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, steps).unwrap(), MockEvaluator);
    engine.set_goal("1205").unwrap();
    engine
}

fn upgrade(id: &str, level: u8, stat: Stat, increase: f64) -> UpgradeResult {
    UpgradeResult {
        relic_id: id.into(),
        expected_level: level,
        stat,
        increase,
    }
}

#[test]
fn imports_fixture_into_own_model_without_preview() {
    let account = load_scanner_v4(DEMO_ACCOUNT, 8).unwrap();
    assert_eq!(account.characters.len(), 2);
    assert_eq!(account.relics.len(), 12);
    assert_eq!(
        account.characters["1205"].light_cone.as_ref().unwrap().id,
        "23009"
    );
    assert_eq!(account.relics["9100001"].substats.len(), 3);
    assert!(
        !account.relics["9100001"]
            .substats
            .contains_key(&Stat::CritDamage)
    );
    assert_eq!(account.relics["9100001"].substats[&Stat::HpPercent], 4.32);
    assert_eq!(
        account.relics["9100005"].equipped_by.as_deref(),
        Some("1205")
    );
    assert!(account.relics["9100005"].locked);
    assert!(account.history.is_empty());
}

#[test]
fn switching_goal_reverses_candidate_order() {
    let mut e = engine(8);
    let blade = e.rank_candidates().unwrap();
    assert_eq!(blade[0].relic_id, "9100002");
    assert!(
        blade.iter().position(|r| r.relic_id == "9100002")
            < blade.iter().position(|r| r.relic_id == "9200002")
    );
    e.set_goal("1102").unwrap();
    let seele = e.rank_candidates().unwrap();
    assert_eq!(seele[0].relic_id, "9200002");
    assert!(
        seele.iter().position(|r| r.relic_id == "9200002")
            < seele.iter().position(|r| r.relic_id == "9100002")
    );
}

#[test]
fn different_observations_on_same_starting_relic_produce_continue_or_hold() {
    let mut positive = engine(8);
    positive.select_relic("9100001").unwrap();
    let out = positive
        .apply_upgrade(upgrade("9100001", 0, Stat::CritDamage, 6.48))
        .unwrap();
    assert_eq!(out.decision, UpgradeDecision::Continue);
    assert_eq!(positive.selected().unwrap().id, "9100001");
    let mut negative = engine(8);
    negative.select_relic("9100001").unwrap();
    let out = negative
        .apply_upgrade(upgrade("9100001", 0, Stat::DefPercent, 5.4))
        .unwrap();
    assert_eq!(out.decision, UpgradeDecision::Hold);
    assert_ne!(negative.selected().unwrap().id, "9100001");
}

#[test]
fn same_relic_accumulates_history_then_stops_and_recommends_another() {
    let mut e = engine(8);
    let untouched = e.account().relics["9100002"].clone();
    e.select_relic("9100001").unwrap();
    e.apply_upgrade(upgrade("9100001", 0, Stat::DefPercent, 5.4))
        .unwrap();
    assert!(
        e.rank_candidates()
            .unwrap()
            .iter()
            .all(|r| r.relic_id != "9100001")
    );
    e.select_relic("9100001").unwrap();
    let out = e
        .apply_upgrade(upgrade("9100001", 3, Stat::DefPercent, 5.4))
        .unwrap();
    assert_eq!(out.decision, UpgradeDecision::Stop);
    assert_eq!(out.next.unwrap().relic_id, "9100002");
    assert_eq!(e.account().relics.len(), 12);
    let relic = &e.account().relics["9100001"];
    assert_eq!(relic.level, 6);
    assert_eq!(relic.substats[&Stat::DefPercent], 10.8);
    assert_eq!(e.account().relics["9100002"], untouched);
    assert_eq!(e.account().upgrade_steps, 6);
    let history = &e.account().history;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].before.level, 0);
    assert_eq!(history[0].after, history[1].before);
    assert_eq!(history[1].after, *relic);
    assert!(e.select_relic("9100001").is_err());
    e.set_goal("1102").unwrap();
    assert_eq!(e.account().relics["9100001"].level, 6);
    assert_eq!(e.account().history.len(), 2);
    assert_eq!(e.account().upgrade_steps, 6);
    assert!(e.select_relic("9100001").is_ok());
    e.set_goal("1205").unwrap();
    assert!(e.select_relic("9100001").is_err());
}

#[test]
fn low_potential_stops_even_on_first_observation() {
    let mut e = engine(8);
    e.select_relic("9100003").unwrap();
    let out = e
        .apply_upgrade(upgrade("9100003", 6, Stat::DefPercent, 5.4))
        .unwrap();
    assert_eq!(out.decision, UpgradeDecision::Stop);
    assert!(out.next.is_some());
}

#[test]
fn useful_upgrade_holds_when_another_candidate_is_clearly_better() {
    let mut e = engine(8);
    e.select_relic("9200001").unwrap();
    let out = e
        .apply_upgrade(upgrade("9200001", 0, Stat::CritRate, 3.24))
        .unwrap();
    assert!(e.account().history.last().unwrap().useful);
    assert_eq!(out.decision, UpgradeDecision::Hold);
    assert!(out.reason.contains("25%"));
    assert_eq!(out.next.unwrap().relic_id, "9100002");
}

#[test]
fn useful_observation_resets_consecutive_misses() {
    let mut e = engine(8);
    e.select_relic("9100001").unwrap();
    e.apply_upgrade(upgrade("9100001", 0, Stat::DefPercent, 5.4))
        .unwrap();
    e.select_relic("9100001").unwrap();
    e.apply_upgrade(upgrade("9100001", 3, Stat::CritRate, 3.24))
        .unwrap();
    e.select_relic("9100001").unwrap();
    let out = e
        .apply_upgrade(upgrade("9100001", 6, Stat::DefPercent, 5.4))
        .unwrap();
    assert_eq!(out.decision, UpgradeDecision::Hold);
    assert!(!e.account().history[0].useful);
    assert!(e.account().history[1].useful);
    assert!(!e.account().history[2].useful);
}

#[test]
fn budget_exhaustion_holds_without_another_recommendation() {
    let mut e = engine(1);
    e.select_relic("9100002").unwrap();
    let out = e
        .apply_upgrade(upgrade("9100002", 3, Stat::CritRate, 3.24))
        .unwrap();
    assert_eq!(out.decision, UpgradeDecision::Hold);
    assert!(out.next.is_none());
    assert_eq!(e.account().upgrade_steps, 0);
    assert!(e.select_relic("9200002").is_err());
    assert!(engine(0).recommend_next().unwrap().is_none());
}

#[test]
fn max_level_is_retained_not_recommended_again() {
    let mut account = load_scanner_v4(DEMO_ACCOUNT, 8).unwrap();
    account.relics.get_mut("9100006").unwrap().locked = false;
    let mut e = DecisionEngine::new(account, MockEvaluator);
    e.set_goal("1205").unwrap();
    e.select_relic("9100006").unwrap();
    let out = e
        .apply_upgrade(upgrade("9100006", 12, Stat::CritRate, 3.24))
        .unwrap();
    assert_eq!(out.decision, UpgradeDecision::Hold);
    assert_eq!(e.account().relics["9100006"].level, 15);
    assert!(e.select_relic("9100006").is_err());
    assert!(
        e.rank_candidates()
            .unwrap()
            .iter()
            .all(|r| r.relic_id != "9100006")
    );
}

#[test]
fn locked_discarded_and_other_characters_equipment_are_excluded() {
    let mut account = load_scanner_v4(DEMO_ACCOUNT, 8).unwrap();
    account.relics.get_mut("9100002").unwrap().discarded = true;
    account.relics.get_mut("9200002").unwrap().equipped_by = Some("1102".into());
    let mut e = DecisionEngine::new(account, MockEvaluator);
    e.set_goal("1205").unwrap();
    let ranking = e.rank_candidates().unwrap();
    for id in [
        "9100002", "9200002", "9100005", "9100006", "9200005", "9200006",
    ] {
        assert!(ranking.iter().all(|r| r.relic_id != id));
        assert!(e.select_relic(id).is_err());
    }
}

#[test]
fn invalid_or_stale_results_do_not_mutate_any_state() {
    let mut e = engine(8);
    e.select_relic("9100001").unwrap();
    let before = e.account().clone();
    for result in [
        upgrade("9100002", 3, Stat::CritRate, 3.24),
        upgrade("9100001", 3, Stat::CritDamage, 6.48),
        upgrade("9100001", 0, Stat::Hp, 42.0),
        upgrade("9100001", 0, Stat::CritRate, 3.24),
        upgrade("9100001", 0, Stat::EnergyRegen, 1.0),
        upgrade("9100001", 0, Stat::CritDamage, -1.0),
        upgrade("9100001", 0, Stat::CritDamage, 0.0),
        upgrade("9100001", 0, Stat::CritDamage, f64::NAN),
        upgrade("9100001", 0, Stat::CritDamage, f64::INFINITY),
    ] {
        assert!(e.apply_upgrade(result).is_err());
        assert_eq!(*e.account(), before);
        assert_eq!(e.selected().unwrap().id, "9100001");
    }
    let result = upgrade("9100001", 0, Stat::CritDamage, 6.48);
    e.apply_upgrade(result.clone()).unwrap();
    let after = e.account().clone();
    assert!(e.apply_upgrade(result).is_err());
    assert!(
        e.apply_upgrade(upgrade("9100001", 3, Stat::DefPercent, 5.4))
            .is_err()
    );
    assert_eq!(*e.account(), after);
}

#[test]
fn malformed_scanner_data_is_rejected() {
    let raw: serde_json::Value = serde_json::from_str(DEMO_ACCOUNT).unwrap();
    for (pointer, bad_value) in [
        ("/version", serde_json::json!(3)),
        ("/source", serde_json::json!("unknown")),
        ("/characters/0/ability_version", serde_json::json!(1)),
        ("/relics/0/level", serde_json::json!(16)),
        ("/relics/0/level", serde_json::json!(1)),
        ("/relics/0/rarity", serde_json::json!(4)),
        ("/relics/0/substats/0/key", serde_json::json!("not_a_stat")),
        ("/relics/0/substats/0/value", serde_json::json!(-1)),
        ("/relics/0/substats/0/key", serde_json::json!("SPD")),
        ("/relics/0/location", serde_json::json!("missing")),
        ("/relics/1/_uid", serde_json::json!("9100001")),
    ] {
        let mut bad = raw.clone();
        *bad.pointer_mut(pointer).unwrap() = bad_value;
        assert!(load_scanner_v4(&bad.to_string(), 8).is_err(), "{pointer}");
    }
}

#[test]
fn stronger_equipped_baseline_removes_non_improving_candidate() {
    let mut account = load_scanner_v4(DEMO_ACCOUNT, 8).unwrap();
    let equipped = account.relics.get_mut("9100001").unwrap();
    equipped.equipped_by = Some("1205".into());
    equipped.locked = true;
    equipped.substats.insert(Stat::CritDamage, 32.4);
    equipped.level = 15;
    let mut e = DecisionEngine::new(account, MockEvaluator);
    e.set_goal("1205").unwrap();
    assert!(
        e.rank_candidates()
            .unwrap()
            .iter()
            .all(|r| r.relic_id != "9200001")
    );
}

struct FailingEvaluator;

impl Evaluator for FailingEvaluator {
    fn evaluate(
        &self,
        account: &AccountState,
        goal: &CultivationGoal,
        relic: &Relic,
    ) -> Result<Evaluation> {
        if relic.id == "9100001" && relic.level > 0 {
            return Err(Error("模拟评估器失败".into()));
        }
        MockEvaluator.evaluate(account, goal, relic)
    }
}

#[test]
fn evaluator_can_be_replaced_and_failure_rolls_back_upgrade() {
    let mut e = DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), FailingEvaluator);
    e.set_goal("1205").unwrap();
    e.select_relic("9100001").unwrap();
    let before = e.account().clone();
    assert!(
        e.apply_upgrade(upgrade("9100001", 0, Stat::CritDamage, 6.48))
            .is_err()
    );
    assert_eq!(*e.account(), before);
    assert_eq!(e.selected().unwrap().id, "9100001");
}

#[test]
fn invalid_goal_does_not_reset_selection_or_account() {
    let mut e = engine(8);
    e.recommend_next().unwrap();
    let before = e.account().clone();
    let selected = e.selected().unwrap().id.clone();
    assert!(e.set_goal("unknown").is_err());
    assert_eq!(*e.account(), before);
    assert_eq!(e.selected().unwrap().id, selected);
    assert_eq!(e.goal().unwrap().character_id, "1205");
}
