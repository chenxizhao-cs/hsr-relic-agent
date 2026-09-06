//! Opt in after building the adapter: cargo test --features fribbels-integration.
#![cfg(feature = "fribbels-integration")]
use hsr_relic_agent::*;
use std::{
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

fn account() -> AccountState {
    load_scanner_v4(DEMO_ACCOUNT, 8).unwrap()
}
fn goal(id: &str) -> CultivationGoal {
    CultivationGoal {
        character_id: id.into(),
    }
}
fn evaluator() -> FribbelsEvaluator {
    FribbelsEvaluator::new(FribbelsConfig::default())
}
fn engine() -> DecisionEngine<FribbelsEvaluator> {
    let mut e = DecisionEngine::new(account(), evaluator());
    e.set_goal("1205").unwrap();
    e
}
fn close(a: f64, b: f64, tolerance: f64) {
    assert!((a - b).abs() < tolerance, "{a} != {b}");
}

#[test]
fn real_scores_match_prior_parser_spike_and_build_is_structured() {
    let a = account();
    let e = evaluator();
    let b = e.metrics(&a, &goal("1205"), &a.relics["9100002"]).unwrap();
    let s = e.metrics(&a, &goal("1102"), &a.relics["9200002"]).unwrap();
    // Golden scores independently established by the scanner parser spike.
    close(b.relic.raw_current_score, 55.5, 0.01);
    close(b.relic.average, 95.38, 0.02);
    close(s.relic.raw_current_score, 54.2, 0.01);
    assert_eq!(b.upstream_commit, FRIBBELS_COMMIT);
    assert_eq!(
        b.candidate_build.damage_model,
        DamageModel::LegacyAtkBasicV1
    );
    assert!(b.candidate_build.panel.hp > 5000.0);
    assert!(b.candidate_build.basic_damage > 0.0);
    assert_eq!(
        b.reference.assumed_slots,
        [Slot::Head, Slot::Hands, Slot::Body, Slot::Feet]
    );
    assert_eq!(&b.reference.relic_ids[4..], ["9100005", "9100006"]);
    assert_eq!(a, account(), "evaluation must not equip or mutate anything");
}

#[test]
fn raw_score_and_main_penalized_potential_are_not_conflated() {
    let mut a = account();
    let r = a.relics.get_mut("9100004").unwrap();
    r.main_stat = Stat::DefPercent;
    let d = evaluator()
        .metrics(&a, &goal("1205"), &a.relics["9100004"])
        .unwrap();
    assert!(d.relic.raw_current_score > d.relic.current);
    assert!(d.relic.average >= d.relic.current);
}

#[test]
fn goal_changes_ranking_and_build_ratio_really_enters_priority() {
    let mut e = engine();
    let blade = e.rank_candidates().unwrap();
    e.set_goal("1102").unwrap();
    let seele = e.rank_candidates().unwrap();
    assert_ne!(blade[0].relic_id, seele[0].relic_id);
    assert_eq!(seele[0].relic_id, "9200001");
    let r = &seele[0];
    let d = r.details.as_ref().unwrap();
    let score_only = (r.projected_score - r.baseline_score).max(0.0) / 5.0;
    close(r.priority, score_only * d.damage_ratio(), 1e-9);
    assert!((r.priority - score_only).abs() > 0.1);
}

#[test]
fn same_relic_updates_produce_continue_hold_stop_and_switch() {
    let mut e = engine();
    e.select_relic("9100002").unwrap();
    let outcome = e
        .apply_upgrade(UpgradeResult {
            relic_id: "9100002".into(),
            expected_level: 3,
            stat: Stat::CritRate,
            increase: 3.24,
        })
        .unwrap();
    assert_eq!(outcome.decision, UpgradeDecision::Continue);
    e.select_relic("9100001").unwrap();
    let first = e
        .apply_upgrade(UpgradeResult {
            relic_id: "9100001".into(),
            expected_level: 0,
            stat: Stat::DefPercent,
            increase: 5.4,
        })
        .unwrap();
    assert_eq!(first.decision, UpgradeDecision::Hold);
    assert!(matches!(
        e.select_relic("9100001").unwrap(),
        RelicSelection::ResumedFromHold { .. }
    ));
    let second = e
        .apply_upgrade(UpgradeResult {
            relic_id: "9100001".into(),
            expected_level: 3,
            stat: Stat::DefPercent,
            increase: 5.4,
        })
        .unwrap();
    assert_eq!(second.decision, UpgradeDecision::Stop);
    assert_eq!(second.details.as_ref().unwrap().relic.id, "9100001");
    assert_ne!(second.next.unwrap().relic_id, "9100001");
    assert_eq!(e.account().relics["9100001"].level, 6);
    close(
        e.account().relics["9100001"].substats[&Stat::DefPercent],
        10.8,
        1e-9,
    );
    assert_eq!(e.account().history.len(), 3);
    assert_eq!(e.account().upgrade_steps, 5);
    assert!(!e.account().relics["9100001"].discarded);
    assert!(matches!(
        e.select_relic("9100001"),
        Err(RelicOperationError::StoppedForTarget { .. })
    ));
}

#[test]
fn real_low_potential_stops_and_budget_still_holds() {
    let mut e = engine();
    e.select_relic("9100003").unwrap();
    assert_eq!(
        e.apply_upgrade(UpgradeResult {
            relic_id: "9100003".into(),
            expected_level: 6,
            stat: Stat::DefPercent,
            increase: 5.4
        })
        .unwrap()
        .decision,
        UpgradeDecision::Stop
    );
    let mut a = account();
    a.upgrade_steps = 1;
    let mut e = DecisionEngine::new(a, evaluator());
    e.set_goal("1205").unwrap();
    e.select_relic("9100002").unwrap();
    let out = e
        .apply_upgrade(UpgradeResult {
            relic_id: "9100002".into(),
            expected_level: 3,
            stat: Stat::CritRate,
            increase: 3.24,
        })
        .unwrap();
    assert_eq!(out.decision, UpgradeDecision::Hold);
    assert!(out.next.is_none());
}

#[test]
fn real_full_level_relic_is_retained_and_excluded_from_upgrade_candidates() {
    let mut a = account();
    a.relics.get_mut("9100002").unwrap().level = 12;
    let mut e = DecisionEngine::new(a, evaluator());
    e.set_goal("1205").unwrap();
    e.select_relic("9100002").unwrap();
    let out = e
        .apply_upgrade(UpgradeResult {
            relic_id: "9100002".into(),
            expected_level: 12,
            stat: Stat::CritRate,
            increase: 3.24,
        })
        .unwrap();
    assert_eq!(out.decision, UpgradeDecision::Hold);
    let d = out.details.unwrap();
    close(d.relic.current, d.relic.average, 0.11);
    assert_eq!(e.account().relics["9100002"].level, 15);
    assert!(!e.account().relics["9100002"].discarded);
    assert!(
        e.rank_candidates()
            .unwrap()
            .iter()
            .all(|r| r.relic_id != "9100002")
    );
}

#[test]
fn cache_uses_state_not_id_and_does_not_hide_main_stat_growth() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let e = FribbelsEvaluator::new(FribbelsConfig {
        progress: Some(Arc::new(move |event| {
            if matches!(event, EvaluationProgress::Started) {
                counted.fetch_add(1, Ordering::Relaxed);
            }
        })),
        ..FribbelsConfig::default()
    });
    let mut a = account();
    let g = goal("1205");
    let before = e.metrics(&a, &g, &a.relics["9100001"]).unwrap();
    e.evaluate(&a, &g, &a.relics["9100002"]).unwrap();
    a.upgrade_steps -= 1;
    e.evaluate(&a, &g, &a.relics["9100002"]).unwrap();
    assert_eq!(
        calls.load(Ordering::Relaxed),
        1,
        "one batch serves all inventory evaluations"
    );
    let r = a.relics.get_mut("9100001").unwrap();
    r.level = 3;
    r.substats.insert(Stat::CritDamage, 6.48);
    let after = e.metrics(&a, &g, &a.relics["9100001"]).unwrap();
    assert!(after.relic.current > before.relic.current);
    assert!(after.candidate_build.panel.hp > before.candidate_build.panel.hp + 100.0);
    assert!(after.candidate_build.basic_damage > before.candidate_build.basic_damage);
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    let original = account();
    assert_eq!(
        e.metrics(&original, &g, &original.relics["9100001"])
            .unwrap(),
        before
    );
}

#[test]
fn enemy_conditions_change_damage_not_relic_score_and_units_are_percentage_points() {
    let a = account();
    let g = goal("1205");
    let base = evaluator().metrics(&a, &g, &a.relics["9100002"]).unwrap();
    let resistant = FribbelsEvaluator::new(FribbelsConfig {
        conditions: CombatConditions {
            elemental_weakness: false,
            ..CombatConditions::default()
        },
        ..FribbelsConfig::default()
    })
    .metrics(&a, &g, &a.relics["9100002"])
    .unwrap();
    assert_eq!(base.relic, resistant.relic);
    close(
        resistant.candidate_build.basic_damage / base.candidate_build.basic_damage,
        0.8,
        1e-6,
    );
    assert!(base.candidate_build.panel.crit_rate_pct > 50.0);
    let mut lc_account = a.clone();
    lc_account
        .characters
        .get_mut("1205")
        .unwrap()
        .light_cone
        .as_mut()
        .unwrap()
        .superimposition = 5;
    let stronger = evaluator()
        .metrics(&lc_account, &g, &lc_account.relics["9100002"])
        .unwrap();
    assert!(stronger.candidate_build.basic_damage > base.candidate_build.basic_damage);
}

#[test]
fn unsupported_input_fails_instead_of_falling_back_and_failure_is_transactional() {
    let mut a = account();
    a.characters.get_mut("1205").unwrap().level = 70;
    let mut e = DecisionEngine::new(a.clone(), evaluator());
    assert!(e.set_goal("1205").unwrap_err().0.contains("level 80"));
    assert_eq!(e.account(), &a);
    let config = FribbelsConfig::default();
    let cancelled = config.cancelled.clone();
    let mut e = DecisionEngine::new(account(), FribbelsEvaluator::new(config));
    e.set_goal("1205").unwrap();
    e.select_relic("9100002").unwrap();
    let snapshot = e.account().clone();
    cancelled.store(true, Ordering::Relaxed);
    assert!(matches!(
        e.apply_upgrade(UpgradeResult {
            relic_id: "9100002".into(),
            expected_level: 3,
            stat: Stat::CritRate,
            increase: 3.24
        }),
        Err(RelicOperationError::EvaluationFailed(_))
    ));
    assert_eq!(e.account(), &snapshot);
    assert_eq!(e.selected().unwrap().id, "9100002");
}

#[test]
fn real_default_cli_demo_completes_without_mock_fallback() {
    let output = Command::new(env!("CARGO_BIN_EXE_hsr-relic-agent"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "判断 Continue",
        "判断 Hold",
        "判断 Stop",
        "Fribbels 当前",
        "简化普攻伤害",
        "演示结束",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
    assert!(!text.contains("Mock 对照"));
}

fn fault_config(mode: u8) -> FribbelsConfig {
    FribbelsConfig {
        adapter: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/fake-adapter.mjs"),
        conditions: CombatConditions {
            enemy_level: mode,
            ..CombatConditions::default()
        },
        ..FribbelsConfig::default()
    }
}

#[test]
fn invalid_protocol_version_status_and_metrics_are_rejected() {
    let a = account();
    for mode in [91, 92, 93, 96, 97, 98, 99] {
        let e = FribbelsEvaluator::new(fault_config(mode));
        assert!(
            e.evaluate(&a, &goal("1205"), &a.relics["9100002"]).is_err(),
            "accepted faulty mode {mode}"
        );
    }
}

#[test]
fn timeout_and_cancellation_terminate_waiting_process() {
    let a = account();
    let mut config = fault_config(94);
    config.timeout = std::time::Duration::from_millis(100);
    let started = std::time::Instant::now();
    let error = FribbelsEvaluator::new(config)
        .evaluate(&a, &goal("1205"), &a.relics["9100002"])
        .unwrap_err();
    assert!(error.0.contains("超时"));
    assert!(started.elapsed() < std::time::Duration::from_secs(3));

    let mut config = fault_config(94);
    let cancel = config.cancelled.clone();
    config.progress = Some(Arc::new(move |event| {
        if matches!(event, EvaluationProgress::Waiting { .. }) {
            cancel.store(true, Ordering::Relaxed);
        }
    }));
    let error = FribbelsEvaluator::new(config)
        .evaluate(&a, &goal("1205"), &a.relics["9100002"])
        .unwrap_err();
    assert!(error.0.contains("取消"));
}

#[test]
fn adapter_failure_after_staged_upgrade_preserves_all_state() {
    let mut e = DecisionEngine::new(account(), FribbelsEvaluator::new(fault_config(95)));
    e.set_goal("1205").unwrap();
    e.select_relic("9100002").unwrap();
    let snapshot = e.account().clone();
    let error = e
        .apply_upgrade(UpgradeResult {
            relic_id: "9100002".into(),
            expected_level: 3,
            stat: Stat::CritRate,
            increase: 3.24,
        })
        .unwrap_err();
    assert!(error.to_string().contains("staged evaluation failed"));
    assert_eq!(e.account(), &snapshot);
    assert_eq!(e.selected().unwrap().id, "9100002");
    assert_eq!(e.goal(), Some(&goal("1205")));
}
