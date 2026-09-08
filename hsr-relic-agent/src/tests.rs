use crate::*;
use std::sync::Arc;

const DEMO_RECOMMENDATIONS: &str = r#"{
  "schema_version": 1,
  "source": {
    "kind": "honkai_star_rail_client_config",
    "repository_url": "https://example.invalid/source",
    "commit": "0123456789abcdef0123456789abcdef01234567",
    "path": "AvatarRelicRecommend.json",
    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
  },
  "profiles": [
    {
      "character_id": "1102",
      "relic_set_ids": ["108", "122", "102"],
      "ornament_set_ids": ["311", "301", "306"],
      "main_stats": {
        "body": ["crit_rate", "crit_damage"],
        "feet": ["speed", "atk_percent"],
        "sphere": ["quantum_damage", "atk_percent"],
        "rope": ["atk_percent"]
      },
      "substats": ["crit_rate", "crit_damage", "atk_percent", "speed"]
    },
    {
      "character_id": "1205",
      "relic_set_ids": ["113", "110", "102"],
      "ornament_set_ids": ["319", "306", "309"],
      "main_stats": {
        "body": ["crit_rate", "crit_damage"],
        "feet": ["speed", "hp_percent"],
        "sphere": ["wind_damage", "hp_percent"],
        "rope": ["hp_percent"]
      },
      "substats": ["crit_rate", "crit_damage", "hp_percent", "speed"]
    }
  ]
}"#;

fn engine(steps: u32) -> DecisionEngine<MockEvaluator> {
    let mut engine =
        DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, steps).unwrap(), MockEvaluator);
    engine.set_goal("1205").unwrap();
    engine
}

fn database() -> Arc<CharacterRelicDatabase> {
    Arc::new(load_character_relic_database(DEMO_RECOMMENDATIONS).unwrap())
}

#[test]
fn loads_versioned_static_recommendation_database() {
    let database = database();
    assert_eq!(database.schema_version(), 1);
    assert_eq!(database.len(), 2);
    assert_eq!(database.source().kind, "honkai_star_rail_client_config");
    let seele = database.profile("1102").unwrap();
    assert_eq!(seele.relic_set_ids, ["108", "122", "102"]);
    assert_eq!(seele.main_stats.sphere[0], Stat::QuantumDamage);
    assert!(seele.substats.contains(&Stat::CritRate));
}

#[test]
fn static_database_filters_sets_for_the_selected_character() {
    let mut engine = DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), MockEvaluator)
        .with_recommendation_database(database());

    engine.set_goal("1102").unwrap();
    assert_eq!(
        engine.set_match(&engine.account().relics["9100002"]),
        RecommendationMatch::NotRecommended
    );
    assert_eq!(
        engine.select_relic("9100002").unwrap_err(),
        RelicOperationError::SetNotRecommendedForTarget {
            relic_id: "9100002".into(),
            character_id: "1102".into(),
            set_id: "113".into(),
        }
    );
    assert!(engine.rank_candidates().unwrap().iter().all(|candidate| {
        matches!(candidate.set_match, RecommendationMatch::Recommended)
            && matches!(
                engine.account().relics[&candidate.relic_id].set_id.as_str(),
                "108" | "309"
            )
    }));

    engine.set_goal("1205").unwrap();
    assert!(engine.rank_candidates().unwrap().iter().all(|candidate| {
        matches!(candidate.set_match, RecommendationMatch::Recommended)
            && matches!(
                engine.account().relics[&candidate.relic_id].set_id.as_str(),
                "113" | "306"
            )
    }));
}

#[test]
fn unknown_character_profiles_do_not_exclude_relics() {
    let database = database();
    let relic = &load_scanner_v4(DEMO_ACCOUNT, 8).unwrap().relics["9100002"];
    assert_eq!(
        database.set_match("missing", relic),
        RecommendationMatch::Unknown
    );
}

#[test]
fn imports_full_reliquary_v4_demo_with_inventory_summary() {
    let imported = load_reliquary_v4(RELIQUARY_DEMO_ACCOUNT, 8).unwrap();
    assert_eq!(imported.summary.source, "reliquary_archiver");
    assert_eq!(imported.summary.version, 4);
    assert_eq!(imported.summary.characters, 64);
    assert_eq!(imported.summary.relics_in_file, 3001);
    assert_eq!(imported.summary.relics_imported, 2971);
    assert_eq!(imported.summary.relics_skipped, 30);
    assert_eq!(imported.summary.light_cones, 391);
    assert_eq!(imported.summary.equipped_relics, 262);
    assert_eq!(imported.summary.imported_equipped_relics, 261);
    assert_eq!(imported.summary.equipped_light_cones, 43);
    assert!(imported.summary.equipment_relations_recognized);
    assert_eq!(imported.account.characters.len(), 64);
    assert_eq!(imported.account.relics.len(), 2971);
    assert_eq!(imported.account.light_cones.len(), 391);
    assert!(
        imported
            .account
            .characters
            .get("1205")
            .unwrap()
            .light_cone
            .is_some()
    );
}

#[test]
fn reliquary_import_errors_are_structured_and_do_not_expose_values() {
    let invalid = r#"{"source":"wrong","build":"test","version":4,"characters":[],"light_cones":[],"relics":[]}"#;
    let error = load_reliquary_v4(invalid, 8).unwrap_err();
    assert_eq!(error.code, AccountImportErrorCode::UnsupportedSource);
    assert_eq!(error.path.as_deref(), Some("source"));
    assert!(!error.message.contains("wrong"));

    let invalid = r#"{"source":"reliquary_archiver","build":"test","version":3,"characters":[],"light_cones":[],"relics":[]}"#;
    let error = load_reliquary_v4(invalid, 8).unwrap_err();
    assert_eq!(error.code, AccountImportErrorCode::UnsupportedVersion);
    assert_eq!(error.path.as_deref(), Some("version"));
}

#[test]
fn reliquary_import_preserves_locks_discards_and_equipment_relations() {
    let json = r#"{
        "source":"reliquary_archiver","build":"test","version":4,
        "characters":[{"id":"character","name":"测试角色","level":80,"eidolon":0,"ability_version":0}],
        "light_cones":[{"_uid":"cone-uid","id":"cone","level":80,"superimposition":2,"location":"character","lock":true}],
        "relics":[{"_uid":"relic-uid","set_id":"set","slot":"Head","rarity":5,"level":0,"mainstat":"HP","substats":[{"key":"ATK","value":16.9},{"key":"DEF","value":16.9},{"key":"CRIT Rate_","value":2.5}],"location":"character","lock":true,"discard":true}]
    }"#;
    let imported = load_reliquary_v4(json, 8).unwrap();
    let character = &imported.account.characters["character"];
    assert_eq!(character.light_cone.as_ref().unwrap().id, "cone");
    let light_cone = &imported.account.light_cones["cone-uid"];
    assert!(light_cone.locked);
    assert_eq!(light_cone.equipped_by.as_deref(), Some("character"));
    let relic = &imported.account.relics["relic-uid"];
    assert!(relic.locked);
    assert!(relic.discarded);
    assert_eq!(relic.equipped_by.as_deref(), Some("character"));
    assert_eq!(imported.summary.equipped_relics, 1);
    assert_eq!(imported.summary.equipped_light_cones, 1);
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
fn selection_failures_return_specific_reasons() {
    let account = load_scanner_v4(DEMO_ACCOUNT, 8).unwrap();
    let mut no_goal = DecisionEngine::new(account.clone(), MockEvaluator);
    assert_eq!(
        no_goal.select_relic("9100001").unwrap_err(),
        RelicOperationError::TargetNotSelected
    );

    let mut account = account;
    account.relics.get_mut("9100002").unwrap().discarded = true;
    account.relics.get_mut("9100006").unwrap().locked = false;
    account.relics.get_mut("9100006").unwrap().level = 15;
    account.relics.get_mut("9200005").unwrap().locked = false;
    let mut e = DecisionEngine::new(account, MockEvaluator);
    e.set_goal("1205").unwrap();

    assert_eq!(
        e.select_relic("missing").unwrap_err(),
        RelicOperationError::RelicNotFound {
            relic_id: "missing".into()
        }
    );
    assert_eq!(
        e.select_relic("9100002").unwrap_err(),
        RelicOperationError::Discarded {
            relic_id: "9100002".into()
        }
    );
    assert_eq!(
        e.select_relic("9100005").unwrap_err(),
        RelicOperationError::Locked {
            relic_id: "9100005".into()
        }
    );
    assert_eq!(
        e.select_relic("9100006").unwrap_err(),
        RelicOperationError::MaxLevel {
            relic_id: "9100006".into(),
            level: 15
        }
    );
    assert_eq!(
        e.select_relic("9200005").unwrap_err(),
        RelicOperationError::EquippedByOtherCharacter {
            relic_id: "9200005".into(),
            character_id: "1102".into()
        }
    );

    let mut no_budget = engine(0);
    assert_eq!(
        no_budget.select_relic("9100001").unwrap_err(),
        RelicOperationError::BudgetExhausted
    );
}

#[test]
fn hold_requires_explicit_resume_and_stop_cannot_be_resumed() {
    let mut e = engine(8);
    e.select_relic("9100001").unwrap();
    e.apply_upgrade(upgrade("9100001", 0, Stat::DefPercent, 5.4))
        .unwrap();

    assert_eq!(
        e.apply_upgrade(upgrade("9100001", 3, Stat::DefPercent, 5.4))
            .unwrap_err(),
        RelicOperationError::HoldRequiresExplicitResume {
            relic_id: "9100001".into(),
            character_id: "1205".into()
        }
    );
    assert_eq!(
        e.select_relic("9100001").unwrap(),
        RelicSelection::ResumedFromHold {
            relic_id: "9100001".into()
        }
    );
    e.apply_upgrade(upgrade("9100001", 3, Stat::DefPercent, 5.4))
        .unwrap();
    assert_eq!(
        e.select_relic("9100001").unwrap_err(),
        RelicOperationError::StoppedForTarget {
            relic_id: "9100001".into(),
            character_id: "1205".into()
        }
    );
    assert_eq!(
        e.apply_upgrade(upgrade("9100001", 6, Stat::DefPercent, 5.4))
            .unwrap_err(),
        RelicOperationError::StoppedForTarget {
            relic_id: "9100001".into(),
            character_id: "1205".into()
        }
    );
}

#[test]
fn upgrade_input_failures_return_specific_reasons() {
    let mut e = engine(8);
    assert_eq!(
        e.apply_upgrade(upgrade("missing", 0, Stat::CritDamage, 6.48))
            .unwrap_err(),
        RelicOperationError::RelicNotFound {
            relic_id: "missing".into()
        }
    );
    assert_eq!(
        e.apply_upgrade(upgrade("9100001", 0, Stat::CritDamage, 6.48))
            .unwrap_err(),
        RelicOperationError::NoRelicSelected
    );
    e.select_relic("9100001").unwrap();
    assert_eq!(
        e.apply_upgrade(upgrade("9100002", 3, Stat::CritRate, 3.24))
            .unwrap_err(),
        RelicOperationError::DifferentRelicSelected {
            selected_relic_id: "9100001".into(),
            result_relic_id: "9100002".into()
        }
    );
    assert_eq!(
        e.apply_upgrade(upgrade("9100001", 3, Stat::CritDamage, 6.48))
            .unwrap_err(),
        RelicOperationError::StaleUpgradeResult {
            relic_id: "9100001".into(),
            reported_level: 3,
            current_level: 0
        }
    );
    assert_eq!(
        e.apply_upgrade(upgrade("9100001", 0, Stat::CritDamage, -1.0))
            .unwrap_err(),
        RelicOperationError::InvalidIncrease
    );
    assert_eq!(
        e.apply_upgrade(upgrade("9100001", 0, Stat::EnergyRegen, 1.0))
            .unwrap_err(),
        RelicOperationError::InvalidSubstat {
            stat: Stat::EnergyRegen
        }
    );
    assert_eq!(
        e.apply_upgrade(upgrade("9100001", 0, Stat::Hp, 42.0))
            .unwrap_err(),
        RelicOperationError::MainStatConflict { stat: Stat::Hp }
    );
    assert_eq!(
        e.apply_upgrade(upgrade("9100001", 0, Stat::CritRate, 3.24))
            .unwrap_err(),
        RelicOperationError::MustAddFourthSubstat
    );

    e.apply_upgrade(upgrade("9100001", 0, Stat::CritDamage, 6.48))
        .unwrap();
    assert_eq!(
        e.apply_upgrade(upgrade("9100001", 3, Stat::DefPercent, 5.4))
            .unwrap_err(),
        RelicOperationError::MustUpgradeExistingSubstat
    );
}

#[test]
fn observed_upgrade_must_advance_exactly_three_levels_transactionally() {
    let mut e = engine(8);
    e.select_relic("9100002").unwrap();
    let before_account = e.account().clone();
    let before_selected = e.selected().unwrap().id.clone();

    assert_eq!(
        e.apply_upgrade_observation(upgrade("9100002", 3, Stat::CritRate, 3.24), 9,)
            .unwrap_err(),
        RelicOperationError::InvalidLevelTransition {
            from_level: 3,
            to_level: 9,
        }
    );
    assert_eq!(*e.account(), before_account);
    assert_eq!(e.selected().unwrap().id, before_selected);
}

#[test]
fn overflowing_stat_result_is_rejected_with_specific_reason() {
    let mut account = load_scanner_v4(DEMO_ACCOUNT, 8).unwrap();
    account
        .relics
        .get_mut("9100002")
        .unwrap()
        .substats
        .insert(Stat::CritRate, f64::MAX);
    let mut e = DecisionEngine::new(account, MockEvaluator);
    e.set_goal("1205").unwrap();
    e.select_relic("9100002").unwrap();
    assert_eq!(
        e.apply_upgrade(upgrade("9100002", 3, Stat::CritRate, f64::MAX))
            .unwrap_err(),
        RelicOperationError::StatOverflow {
            stat: Stat::CritRate
        }
    );
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
fn structured_preferences_select_only_controlled_strategies() {
    let conservative = CultivationPreferences {
        material_pressure: MaterialPressure::Tight,
        risk_tolerance: RiskTolerance::Conservative,
        objective: CultivationObjective::Balanced,
    };
    let high_potential = CultivationPreferences {
        material_pressure: MaterialPressure::Relaxed,
        risk_tolerance: RiskTolerance::Aggressive,
        objective: CultivationObjective::Balanced,
    };
    assert_eq!(conservative.strategy(), CultivationStrategy::Conservative);
    assert_eq!(
        CultivationPreferences::default().strategy(),
        CultivationStrategy::Balanced
    );
    assert_eq!(
        high_potential.strategy(),
        CultivationStrategy::HighPotential
    );
    assert_eq!(
        CultivationPreferences {
            objective: CultivationObjective::ImmediatePower,
            ..high_potential
        }
        .strategy(),
        CultivationStrategy::Conservative
    );
    assert_eq!(
        CultivationPreferences {
            objective: CultivationObjective::MaxPotential,
            ..conservative
        }
        .strategy(),
        CultivationStrategy::HighPotential
    );
}

#[test]
fn default_goal_preserves_the_original_balanced_priority_formula() {
    let mut e = DecisionEngine::new(load_scanner_v4(DEMO_ACCOUNT, 8).unwrap(), MockEvaluator);
    e.set_goal("1205").unwrap();
    assert_eq!(
        e.goal().unwrap().preferences,
        CultivationPreferences::default()
    );
    assert_eq!(e.goal().unwrap().strategy(), CultivationStrategy::Balanced);
    for candidate in e.rank_candidates().unwrap() {
        let relic = &e.account().relics[&candidate.relic_id];
        let remaining = f64::from((15 - relic.level) / 3);
        let original = (candidate.projected_score - candidate.baseline_score).max(0.0) / remaining;
        assert!((candidate.priority - original).abs() < 1e-12);
        assert_eq!(candidate.strategy, CultivationStrategy::Balanced);
    }
}

#[test]
fn old_goal_json_defaults_to_balanced_preferences() {
    let goal: CultivationGoal = serde_json::from_str(r#"{"character_id":"1205"}"#).unwrap();
    assert_eq!(goal, CultivationGoal::balanced("1205"));
    assert!(
        serde_json::from_str::<CultivationPreferences>(
            r#"{"material_pressure":"scarce","risk_tolerance":"balanced","objective":"balanced"}"#
        )
        .is_err()
    );
}

#[test]
fn account_state_round_trips_for_versioned_session_storage() {
    let mut account = load_scanner_v4(DEMO_ACCOUNT, 8).unwrap();
    account
        .decisions
        .insert(("1205".into(), "9100001".into()), UpgradeDecision::Hold);
    let json = serde_json::to_string(&account).unwrap();
    let restored: AccountState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, account);
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
    assert_eq!(
        e.apply_upgrade(upgrade("9100001", 0, Stat::CritDamage, 6.48))
            .unwrap_err(),
        RelicOperationError::EvaluationFailed(Error("模拟评估器失败".into()))
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
