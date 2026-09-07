use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use hsr_relic_agent::{RELIQUARY_DEMO_ACCOUNT, load_reliquary_v4};
use hsr_relic_web::{App, router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::path::PathBuf;
use tower::ServiceExt;

fn app() -> Router {
    router(
        App::new(true),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."),
    )
}
fn reliquary_app() -> Router {
    router(
        App::new(true).with_demo_account(load_reliquary_v4(RELIQUARY_DEMO_ACCOUNT, 8).unwrap()),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."),
    )
}
async fn request(
    app: &Router,
    method: &str,
    path: &str,
    token: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .header("x-demo-session", token)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}
async fn session(app: &Router) -> (String, Value) {
    let (status, value) = request(app, "POST", "/api/session", "", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    (
        value["session"].as_str().unwrap().into(),
        value["state"].clone(),
    )
}
async fn act(app: &Router, token: &str, state: &mut Value, mut action: Value) {
    action["expected_revision"] = state["revision"].clone();
    let (status, value) = request(app, "POST", "/api/action", token, action).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    *state = value;
}
fn relic<'a>(state: &'a Value, id: &str) -> &'a Value {
    state["inventory"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap()
}

#[tokio::test]
async fn core_loop_updates_same_relic_and_resumes_hold_explicitly() {
    let app = app();
    let (token, mut state) = session(&app).await;
    act(
        &app,
        &token,
        &mut state,
        json!({"action":"target","character_id":"1205"}),
    )
    .await;
    act(
        &app,
        &token,
        &mut state,
        json!({"action":"select","relic_id":"9100002"}),
    )
    .await;
    act(&app, &token, &mut state, json!({"action":"upgrade","relic_id":"9100002","expected_level":3,"stat":"crit_rate","increase":3.24})).await;
    assert_eq!(state["last_result"]["decision"], "Continue");
    assert_eq!(relic(&state, "9100002")["level"], 6);
    act(
        &app,
        &token,
        &mut state,
        json!({"action":"select","relic_id":"9100001"}),
    )
    .await;
    act(&app, &token, &mut state, json!({"action":"upgrade","relic_id":"9100001","expected_level":0,"stat":"def_percent","increase":5.4})).await;
    assert_eq!(state["last_result"]["decision"], "Hold");
    assert_eq!(relic(&state, "9100001")["decision"], "Hold");
    act(
        &app,
        &token,
        &mut state,
        json!({"action":"select","relic_id":"9100001"}),
    )
    .await;
    act(&app, &token, &mut state, json!({"action":"upgrade","relic_id":"9100001","expected_level":3,"stat":"def_percent","increase":5.4})).await;
    assert_eq!(state["last_result"]["decision"], "Stop");
    assert_eq!(relic(&state, "9100001")["level"], 6);
    assert_eq!(
        relic(&state, "9100001")["blocked"]["code"],
        "stopped_for_target"
    );
    assert_ne!(state["selected_id"], "9100001");
    assert_eq!(state["history"].as_array().unwrap().len(), 3);
    assert_eq!(state["remaining_budget"], 5);
}

#[tokio::test]
async fn demo_and_uploaded_reliquary_accounts_share_imported_shape_and_fail_transactionally() {
    let demo_app = reliquary_app();
    let (demo_token, demo_state) = session(&demo_app).await;
    assert_eq!(demo_state["account_summary"]["characters"], 64);
    assert_eq!(demo_state["account_summary"]["relics_in_file"], 3001);
    assert_eq!(demo_state["account_summary"]["relics_imported"], 2971);
    assert_eq!(demo_state["account_summary"]["relics_skipped"], 30);
    assert_eq!(demo_state["account_summary"]["light_cones"], 391);
    assert_eq!(demo_state["inventory"].as_array().unwrap().len(), 2971);

    let legacy_app = app();
    let (token, before) = session(&legacy_app).await;
    let account: Value = serde_json::from_str(RELIQUARY_DEMO_ACCOUNT).unwrap();
    let (status, uploaded) = request(
        &legacy_app,
        "POST",
        "/api/account/import",
        &token,
        json!({"expected_revision":before["revision"],"account":account}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{uploaded}");
    assert_eq!(uploaded["account_summary"], demo_state["account_summary"]);
    assert_eq!(uploaded["inventory"].as_array().unwrap().len(), 2971);

    let mut invalid: Value = serde_json::from_str(RELIQUARY_DEMO_ACCOUNT).unwrap();
    invalid["source"] = json!("not_reliquary");
    let (status, error) = request(
        &legacy_app,
        "POST",
        "/api/account/import",
        &token,
        json!({"expected_revision":uploaded["revision"],"account":invalid}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["error"]["code"], "unsupported_source");
    assert_eq!(error["error"]["path"], "source");
    let (_, unchanged) = request(&legacy_app, "GET", "/api/state", &token, Value::Null).await;
    assert_eq!(unchanged, uploaded);

    let (status, reloaded) = request(
        &demo_app,
        "POST",
        "/api/account/demo",
        &demo_token,
        json!({"expected_revision":demo_state["revision"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reloaded["account_summary"], demo_state["account_summary"]);

    let (status, saved) = request(
        &demo_app,
        "GET",
        "/api/session/export",
        &demo_token,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(serde_json::to_vec(&saved).unwrap().len() < 7 * 1024 * 1024);
    let (status, restored) = request(
        &demo_app,
        "POST",
        "/api/session/import",
        &demo_token,
        json!({"expected_revision":reloaded["revision"],"session":saved}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(restored["account_summary"], demo_state["account_summary"]);
    assert_eq!(restored["inventory"].as_array().unwrap().len(), 2971);
}

#[tokio::test]
async fn errors_and_repeated_submissions_preserve_committed_state() {
    let app = app();
    let (token, mut state) = session(&app).await;
    act(
        &app,
        &token,
        &mut state,
        json!({"action":"target","character_id":"1205"}),
    )
    .await;
    act(
        &app,
        &token,
        &mut state,
        json!({"action":"select","relic_id":"9100002"}),
    )
    .await;
    for (action, expected, code) in [
        (
            json!({"action":"upgrade","relic_id":"9100002","expected_level":3,"stat":"crit_rate","increase":-1,"expected_revision":2}),
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_increase",
        ),
        (
            json!({"action":"select","relic_id":"9100005","expected_revision":2}),
            StatusCode::UNPROCESSABLE_ENTITY,
            "locked",
        ),
        (
            json!({"action":"select","relic_id":"9100001","expected_revision":1}),
            StatusCode::CONFLICT,
            "stale_revision",
        ),
        (
            json!({"action":"upgrade","expected_revision":2}),
            StatusCode::BAD_REQUEST,
            "invalid_request",
        ),
    ] {
        let (status, error) = request(&app, "POST", "/api/action", &token, action).await;
        assert_eq!(status, expected, "{error}");
        assert_eq!(error["error"]["code"], code);
        let (_, unchanged) = request(&app, "GET", "/api/state", &token, Value::Null).await;
        assert_eq!(unchanged, state);
    }
    let command = json!({"action":"upgrade","relic_id":"9100002","expected_level":3,"stat":"crit_rate","increase":3.24,"expected_revision":2});
    let (status, after) = request(&app, "POST", "/api/action", &token, command.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = request(&app, "POST", "/api/action", &token, command).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, unchanged) = request(&app, "GET", "/api/state", &token, Value::Null).await;
    assert_eq!(unchanged, after);
}

#[tokio::test]
async fn targets_differ_and_sessions_and_reset_are_isolated() {
    let app = app();
    let (a, mut blade) = session(&app).await;
    let (b, mut seele) = session(&app).await;
    act(
        &app,
        &a,
        &mut blade,
        json!({"action":"target","character_id":"1205"}),
    )
    .await;
    act(
        &app,
        &b,
        &mut seele,
        json!({"action":"target","character_id":"1102"}),
    )
    .await;
    assert_ne!(
        blade["recommendations"][0]["relic_id"],
        seele["recommendations"][0]["relic_id"]
    );
    act(&app, &a, &mut blade, json!({"action":"reset"})).await;
    assert!(blade["target_id"].is_null());
    assert_eq!(blade["remaining_budget"], 8);
    let (_, other) = request(&app, "GET", "/api/state", &b, Value::Null).await;
    assert_eq!(other, seele);
    let (status, _) = request(&app, "GET", "/api/state", "missing", Value::Null).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (_, progress) = request(&app, "GET", "/api/progress", &b, Value::Null).await;
    assert_eq!(progress["running"], false);
}

#[tokio::test]
async fn model_config_is_server_side_and_secret_is_never_returned() {
    let app = app();
    let (token, state) = session(&app).await;
    let body = json!({
        "expected_revision":state["revision"],
        "endpoint":"http://localhost:1234/v1",
        "api_key":"super-secret-value",
        "clear_api_key":false,
        "model":"local-model",
        "context_length":8192,
        "reasoning_mode":"medium",
        "input_price_per_million":1.25,
        "output_price_per_million":5.0,
        "token_budget":12345
    });
    let (status, configured) = request(&app, "POST", "/api/model-config", &token, body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(configured["model_config"]["api_key_configured"], true);
    assert_eq!(configured["model_config"]["model"], "local-model");
    assert_eq!(configured["model_config"]["token_budget"], 12345);
    assert!(!configured.to_string().contains("super-secret-value"));
    let (_, reread) = request(&app, "GET", "/api/state", &token, Value::Null).await;
    assert!(!reread.to_string().contains("super-secret-value"));
}

#[tokio::test]
async fn versioned_session_json_restores_business_state_without_api_key() {
    let app = app();
    let (source, mut state) = session(&app).await;
    act(
        &app,
        &source,
        &mut state,
        json!({"action":"target","character_id":"1205"}),
    )
    .await;
    act(
        &app,
        &source,
        &mut state,
        json!({"action":"select","relic_id":"9100002"}),
    )
    .await;
    act(&app, &source, &mut state, json!({"action":"upgrade","relic_id":"9100002","expected_level":3,"stat":"crit_rate","increase":3.24})).await;
    let (status, _) = request(
        &app,
        "POST",
        "/api/model-config",
        &source,
        json!({"expected_revision":state["revision"],"endpoint":"http://localhost:1234/v1",
            "api_key":"must-not-be-exported","clear_api_key":false,"model":"saved-model",
            "context_length":4096,"reasoning_mode":"disabled","input_price_per_million":1.0,
            "output_price_per_million":2.0,"token_budget":5000}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, saved) = request(&app, "GET", "/api/session/export", &source, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["schema_version"], 1);
    assert_eq!(saved["engine"]["account"]["relics"]["9100002"]["level"], 6);
    assert_eq!(saved["engine"]["goal"]["character_id"], "1205");
    assert_eq!(saved["model_config"]["model"], "saved-model");
    assert!(!saved.to_string().contains("must-not-be-exported"));

    let (target, target_state) = session(&app).await;
    let (status, restored) = request(
        &app,
        "POST",
        "/api/session/import",
        &target,
        json!({"expected_revision":target_state["revision"],"session":saved}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["target_id"], "1205");
    assert_eq!(restored["selected_id"], "9100002");
    assert_eq!(relic(&restored, "9100002")["level"], 6);
    assert_eq!(restored["history"].as_array().unwrap().len(), 1);
    assert_eq!(restored["model_config"]["model"], "saved-model");
    assert_eq!(restored["model_config"]["api_key_configured"], false);
}
