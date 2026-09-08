//! Opt-in wire-level Agent loop with the real Fribbels adapter. The model side
//! is a scripted OpenAI-compatible HTTP server, not a real LLM.
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use hsr_relic_web::{App, router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    thread,
};
use tower::ServiceExt;

async fn request(
    app: &axum::Router,
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

fn scripted_model() -> (String, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let responses = [
        json!({"id":"agent-1","choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call-1","type":"function","function":{"name":"set_cultivation_intent","arguments":"{\"character\":\"Blade\",\"material_pressure\":\"tight\",\"risk_tolerance\":\"conservative\",\"objective\":\"immediate_power\"}"}}]}}],"usage":{"prompt_tokens":100,"completion_tokens":20,"total_tokens":120}}),
        json!({"id":"agent-2","choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call-2","type":"function","function":{"name":"get_next_relic_recommendation","arguments":"{}"}}]}}],"usage":{"prompt_tokens":150,"completion_tokens":25,"total_tokens":175}}),
        json!({"id":"agent-3","choices":[{"message":{"role":"assistant","content":"为 Blade 推荐遗器 #9200003。数值来自 Fribbels，排序来自 Rust Decision Engine。"}}],"usage":{"prompt_tokens":200,"completion_tokens":30,"total_tokens":230}}),
        json!({"id":"agent-4","choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call-4","type":"function","function":{"name":"record_upgrade_result","arguments":"{\"expected_level\":6,\"resulting_level\":9,\"stat\":\"hp_percent\",\"increase\":4.32}"}}]}}],"usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12}}),
        json!({"id":"agent-5","choices":[{"message":{"role":"assistant","content":"Rust 判断为 Continue，请继续观察下一次强化。"}}],"usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12}}),
        json!({"id":"agent-6","choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call-6","type":"function","function":{"name":"get_current_state","arguments":"{}"}}]}}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}),
        json!({"id":"agent-7","choices":[{"message":{"role":"assistant","content":"已从加载的上下文继续，会话目标仍是 Blade。"}}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}),
    ];
    let handle = thread::spawn(move || {
        let mut requests = vec![];
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut received = Vec::new();
            let mut buffer = [0; 8192];
            loop {
                let count = stream.read(&mut buffer).unwrap();
                received.extend_from_slice(&buffer[..count]);
                if let Some(end) = received.windows(4).position(|v| v == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&received[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if received.len() >= end + 4 + length {
                        let body = &received[end + 4..end + 4 + length];
                        requests.push(serde_json::from_slice(body).unwrap());
                        break;
                    }
                }
            }
            let body = response.to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        }
        requests
    });
    (format!("http://{address}/v1"), handle)
}

#[tokio::test]
#[ignore = "requires a local socket and the built Fribbels adapter"]
async fn scripted_model_calls_tools_real_fribbels_and_budget_blocks_next_call() {
    let (endpoint, model_server) = scripted_model();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let app = router(App::new(false), root);
    let (status, created) = request(&app, "POST", "/api/session", "", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    let token = created["session"].as_str().unwrap();
    let (status, configured) = request(
        &app,
        "POST",
        "/api/model-config",
        token,
        json!({"expected_revision":0,"endpoint":endpoint.clone(),"api_key":"wire-secret","clear_api_key":false,
            "model":"scripted-model","context_length":2048,"reasoning_mode":"disabled",
            "input_price_per_million":2.0,"output_price_per_million":8.0,"token_budget":1000}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!configured.to_string().contains("wire-secret"));
    let (status, state) = request(
        &app,
        "POST",
        "/api/agent",
        token,
        json!({"expected_revision":1,"input":"我想培养 Blade，材料比较紧，帮我看看下一件最值得强化什么。"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{state}");
    assert_eq!(state["target_id"], "1205");
    assert_eq!(state["cultivation_intent"]["strategy"], "conservative");
    assert_eq!(state["selected_id"], "9200003");
    assert_eq!(state["usage"]["summary"]["input_tokens"], 450);
    assert_eq!(state["usage"]["summary"]["output_tokens"], 75);
    assert_eq!(state["usage"]["summary"]["total_tokens"], 525);
    assert!((state["usage"]["summary"]["total_cost"].as_f64().unwrap() - 0.0015).abs() < 1e-12);
    assert!(
        state["last_agent"]["reply"]
            .as_str()
            .unwrap()
            .contains("#9200003")
    );
    assert!(
        state["last_agent"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["type"] == "cultivation_intent_resolved"
                && event["strategy"] == "conservative")
    );
    let (status, state) = request(
        &app,
        "POST",
        "/api/agent",
        token,
        json!({"expected_revision":state["revision"],
            "input":"刚才那件从 +6 升到 +9，生命百分比增加了 4.32"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{state}");
    let upgraded = state["inventory"]
        .as_array()
        .unwrap()
        .iter()
        .find(|relic| relic["id"] == "9200003")
        .unwrap();
    assert_eq!(upgraded["level"], 9);
    assert_eq!(upgraded["substats"]["hp_percent"], 12.96);
    assert_eq!(state["remaining_budget"], 7);
    assert_eq!(state["history"].as_array().unwrap().len(), 1);
    assert_eq!(state["history"][0]["decision"], "Continue");
    assert_eq!(state["last_result"]["relic_id"], "9200003");
    assert_eq!(
        state["last_result"]["decision"],
        state["history"][0]["decision"]
    );
    assert!(
        state["last_agent"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["type"] == "decision_recorded"
                && event["tool_name"] == "record_upgrade_result")
    );
    assert_eq!(state["usage"]["summary"]["total_tokens"], 549);
    let (status, saved) = request(&app, "GET", "/api/session/export", token, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["schema_version"], 1);
    assert_eq!(saved["conversation"].as_array().unwrap().len(), 11);
    assert_eq!(saved["tasks"].as_array().unwrap().len(), 2);
    assert!(!saved.to_string().contains("wire-secret"));
    let (_, fresh) = request(&app, "POST", "/api/session", "", Value::Null).await;
    let restored_token = fresh["session"].as_str().unwrap();
    let (status, restored) = request(
        &app,
        "POST",
        "/api/session/import",
        restored_token,
        json!({"expected_revision":0,"session":saved}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["target_id"], "1205");
    assert_eq!(restored["usage"]["summary"]["total_tokens"], 549);
    let (_, restored_tasks) = request(&app, "GET", "/api/tasks", restored_token, Value::Null).await;
    assert_eq!(restored_tasks["tasks"].as_array().unwrap().len(), 2);
    let (status, continued) = request(
        &app,
        "POST",
        "/api/agent",
        restored_token,
        json!({"expected_revision":restored["revision"],"input":"加载后继续，确认当前目标"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{continued}");
    assert_eq!(continued["target_id"], "1205");
    assert!(
        continued["last_agent"]["reply"]
            .as_str()
            .unwrap()
            .contains("加载的上下文")
    );
    let (_, continued_tasks) =
        request(&app, "GET", "/api/tasks", restored_token, Value::Null).await;
    assert_eq!(continued_tasks["tasks"].as_array().unwrap().len(), 3);
    let requests = model_server.join().unwrap();
    assert_eq!(requests.len(), 7);
    assert_eq!(requests[0]["tool_choice"], "required");
    assert_eq!(
        requests[1]["messages"].as_array().unwrap().last().unwrap()["role"],
        "tool"
    );
    assert_eq!(
        requests[4]["messages"].as_array().unwrap().last().unwrap()["role"],
        "tool"
    );
    assert!(requests[5]["messages"].as_array().unwrap().len() > 11);

    let (status, limited) = request(
        &app,
        "POST",
        "/api/model-config",
        token,
        json!({"expected_revision":3,"endpoint":endpoint,"api_key":null,"clear_api_key":false,
            "model":"scripted-model","context_length":2048,"reasoning_mode":"disabled",
            "input_price_per_million":2.0,"output_price_per_million":8.0,"token_budget":549}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, error) = request(
        &app,
        "POST",
        "/api/agent",
        token,
        json!({"expected_revision":limited["revision"],"input":"再推荐一次"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["error"]["code"], "token_budget_reached");
}
