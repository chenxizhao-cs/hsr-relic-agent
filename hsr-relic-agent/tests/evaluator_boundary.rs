//! Pure Rust boundary checks; no Node or adapter installation needed.
use hsr_relic_agent::*;

fn account() -> AccountState {
    load_scanner_v4(DEMO_ACCOUNT, 8).unwrap()
}
fn goal() -> CultivationGoal {
    CultivationGoal {
        character_id: "1205".into(),
    }
}

#[test]
fn reference_build_preserves_equipped_slots_and_respects_inventory_protection() {
    let mut a = account();
    a.relics.get_mut("9100001").unwrap().locked = true;
    a.relics.get_mut("9100002").unwrap().discarded = true;
    a.relics.get_mut("9100003").unwrap().equipped_by = Some("1102".into());
    let reference = FribbelsEvaluator::reference_build(&a, &goal()).unwrap();
    assert_eq!(
        reference.relic_ids,
        [
            "9200001", "9200002", "9200003", "9100004", "9100005", "9100006"
        ]
    );
    a.relics.get_mut("9200001").unwrap().locked = true;
    assert!(
        FribbelsEvaluator::reference_build(&a, &goal())
            .unwrap_err()
            .0
            .contains("Head")
    );
}

#[test]
fn missing_bundle_missing_node_and_cancel_are_errors_not_mock_results() {
    let a = account();
    let r = &a.relics["9100002"];
    let config = FribbelsConfig {
        adapter: "/nonexistent/hsr-demo-adapter.mjs".into(),
        ..FribbelsConfig::default()
    };
    assert!(
        FribbelsEvaluator::new(config)
            .evaluate(&a, &goal(), r)
            .unwrap_err()
            .0
            .contains("找不到")
    );
    let config = FribbelsConfig {
        node: "/nonexistent/hsr-demo-node".into(),
        adapter: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
        ..FribbelsConfig::default()
    };
    assert!(
        FribbelsEvaluator::new(config)
            .evaluate(&a, &goal(), r)
            .unwrap_err()
            .0
            .contains("无法启动 Node")
    );
    let config = FribbelsConfig::default();
    config
        .cancelled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(
        FribbelsEvaluator::new(config)
            .evaluate(&a, &goal(), r)
            .unwrap_err()
            .0
            .contains("取消")
    );
}

#[test]
fn unequipped_or_mismatched_input_does_not_silently_gain_default_metadata() {
    let mut a = account();
    a.characters.get_mut("1205").unwrap().light_cone = None;
    assert!(
        FribbelsEvaluator::new(FribbelsConfig::default())
            .evaluate(&a, &goal(), &a.relics["9100002"])
            .unwrap_err()
            .0
            .contains("光锥")
    );
    let a = account();
    let mut r = a.relics["9100002"].clone();
    r.level = 6;
    assert!(
        FribbelsEvaluator::new(FribbelsConfig::default())
            .evaluate(&a, &goal(), &r)
            .unwrap_err()
            .0
            .contains("AccountState")
    );
}
