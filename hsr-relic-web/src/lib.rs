mod dto;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use hsr_agent_runtime::{
    AgentRuntime, ModelConfig, ModelConfigPatch, OpenAiCompatibleProvider, RuntimeError,
    UsageLedger,
};
use hsr_relic_agent::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};
use uuid::Uuid;

#[derive(Clone)]
enum WebEvaluator {
    Fribbels(Arc<Mutex<FribbelsEvaluator>>),
    Mock,
}
impl Evaluator for WebEvaluator {
    fn evaluate(
        &self,
        a: &AccountState,
        g: &CultivationGoal,
        r: &Relic,
    ) -> hsr_relic_agent::Result<Evaluation> {
        match self {
            Self::Mock => MockEvaluator.evaluate(a, g, r),
            Self::Fribbels(e) => e.lock().unwrap().evaluate(a, g, r),
        }
    }
    fn details(
        &self,
        a: &AccountState,
        g: &CultivationGoal,
        r: &Relic,
    ) -> hsr_relic_agent::Result<Option<EvaluationDetails>> {
        match self {
            Self::Mock => Ok(None),
            Self::Fribbels(e) => e.lock().unwrap().details(a, g, r),
        }
    }
    fn name(&self) -> &'static str {
        match self {
            Self::Mock => "Mock",
            Self::Fribbels(_) => "Fribbels",
        }
    }
    fn minimum_potential(&self) -> f64 {
        match self {
            Self::Mock => MockEvaluator.minimum_potential(),
            Self::Fribbels(e) => e.lock().unwrap().minimum_potential(),
        }
    }
}

type Engine = DecisionEngine<WebEvaluator>;
#[derive(Clone)]
struct SessionData {
    engine: Engine,
    evaluator: WebEvaluator,
    revision: u64,
    last_result: Option<Value>,
    model_config: ModelConfig,
    usage: UsageLedger,
    last_agent: Option<hsr_agent_runtime::AgentRun>,
}
struct Session {
    data: Mutex<SessionData>,
    snapshot: Mutex<Value>,
    cancelled: Arc<AtomicBool>,
    running: AtomicBool,
    started: Mutex<Option<Instant>>,
    touched: Mutex<Instant>,
}
#[derive(Clone)]
pub struct App {
    sessions: Arc<Mutex<HashMap<String, Arc<Session>>>>,
    mock: bool,
    default_model_config: ModelConfig,
}
impl App {
    pub fn new(mock: bool) -> Self {
        Self::with_model_config(mock, ModelConfig::default())
    }
    pub fn with_model_config(mock: bool, default_model_config: ModelConfig) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            mock,
            default_model_config,
        }
    }
    fn session(&self, headers: &HeaderMap) -> ApiResult<Arc<Session>> {
        let token = headers
            .get("x-demo-session")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        let s = self
            .sessions
            .lock()
            .unwrap()
            .get(token)
            .cloned()
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::UNAUTHORIZED,
                    "session_expired",
                    "试用会话已结束，请重新加载页面。",
                )
            })?;
        *s.touched.lock().unwrap() = Instant::now();
        Ok(s)
    }
}

pub fn router(app: App, root: PathBuf) -> Router {
    Router::new()
        .route("/api/session", post(create_session))
        .route("/api/state", get(state))
        .route("/api/action", post(action))
        .route("/api/model-config", post(update_model_config))
        .route("/api/agent", post(agent))
        .route("/api/progress", get(progress))
        .route("/api/cancel", post(cancel))
        .route("/api/health", get(|| async { Json(json!({"ok":true})) }))
        .nest_service(
            "/assets",
            ServeDir::new(root.join("upstream/hsr-optimizer/public/assets")),
        )
        .fallback_service(ServeDir::new(root.join("web")))
        .layer(DefaultBodyLimit::max(16384))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .with_state(app)
}

type ApiResult<T> = std::result::Result<T, ApiError>;
pub(crate) struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}
impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error":{"code":self.code,"message":self.message}})),
        )
            .into_response()
    }
}
impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, "evaluation_failed", e.0)
    }
}
impl From<RelicOperationError> for ApiError {
    fn from(e: RelicOperationError) -> Self {
        let (code, message) = dto::operation_error(&e);
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, code, message)
    }
}

async fn create_session(State(app): State<App>) -> ApiResult<Json<Value>> {
    let mut sessions = app.sessions.lock().unwrap();
    sessions.retain(|_, s| {
        s.running.load(Ordering::Relaxed)
            || s.touched.lock().unwrap().elapsed() < Duration::from_secs(7200)
    });
    if sessions.len() >= 64 {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "session_limit",
            "试用人数已达上限，请稍后再试。",
        ));
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let evaluator = if app.mock {
        WebEvaluator::Mock
    } else {
        WebEvaluator::Fribbels(Arc::new(Mutex::new(FribbelsEvaluator::new(
            FribbelsConfig {
                cancelled: cancelled.clone(),
                ..FribbelsConfig::default()
            },
        ))))
    };
    let data = SessionData {
        engine: Engine::new(load_scanner_v4(DEMO_ACCOUNT, 8)?, evaluator.clone()),
        evaluator,
        revision: 0,
        last_result: None,
        model_config: app.default_model_config.clone(),
        usage: UsageLedger::default(),
        last_agent: None,
    };
    let snapshot = dto::snapshot(&data)?;
    let token = Uuid::new_v4().to_string();
    sessions.insert(
        token.clone(),
        Arc::new(Session {
            data: Mutex::new(data),
            snapshot: Mutex::new(snapshot.clone()),
            cancelled,
            running: AtomicBool::new(false),
            started: Mutex::new(None),
            touched: Mutex::new(Instant::now()),
        }),
    );
    Ok(Json(json!({"session":token,"state":snapshot})))
}
async fn state(State(app): State<App>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    let s = app.session(&headers)?;
    let snapshot = s.snapshot.lock().unwrap().clone();
    Ok(Json(snapshot))
}

#[derive(Deserialize)]
struct Command {
    expected_revision: u64,
    #[serde(flatten)]
    action: Action,
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Action {
    Target {
        character_id: String,
    },
    Select {
        relic_id: String,
    },
    Recommend,
    Upgrade {
        relic_id: String,
        expected_level: u8,
        stat: Stat,
        increase: f64,
    },
    Reset,
}

async fn action(
    State(app): State<App>,
    headers: HeaderMap,
    payload: std::result::Result<Json<Command>, axum::extract::rejection::JsonRejection>,
) -> ApiResult<Json<Value>> {
    let command = payload
        .map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "输入格式不正确，请检查强化属性和增量。",
            )
        })?
        .0;
    let s = app.session(&headers)?;
    if s.running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "busy",
            "上一项操作仍在计算，请稍候或取消。",
        ));
    }
    s.cancelled.store(false, Ordering::SeqCst);
    *s.started.lock().unwrap() = Some(Instant::now());
    tokio::task::spawn_blocking(move || {
        // Keep the committed engine and browser snapshot together. Transport errors
        // or a cancelled evaluation never expose a half-applied Web action.
        let result = execute(&s, command);
        *s.started.lock().unwrap() = None;
        s.running.store(false, Ordering::SeqCst);
        result.map(Json)
    })
    .await
    .map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "worker_failed",
            "计算任务异常，请重置试用。",
        )
    })?
}

fn execute(s: &Session, command: Command) -> ApiResult<Value> {
    let mut committed = s.data.lock().unwrap();
    if committed.revision != command.expected_revision {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "stale_revision",
            "账号状态已更新；页面已同步，请核对后重新操作。",
        ));
    }
    let mut staged = committed.clone();
    match command.action {
        Action::Target { character_id } => {
            staged.engine.set_goal(&character_id)?;
            staged.engine.recommend_next()?;
            staged.last_result = None;
        }
        Action::Select { relic_id } => {
            staged.engine.select_relic(&relic_id)?;
        }
        Action::Recommend => {
            staged.engine.recommend_next()?;
        }
        Action::Upgrade {
            relic_id,
            expected_level,
            stat,
            increase,
        } => {
            let result = staged.engine.apply_upgrade(UpgradeResult {
                relic_id: relic_id.clone(),
                expected_level,
                stat,
                increase,
            })?;
            staged.last_result = Some(
                json!({"relic_id":relic_id, "decision":dto::decision(result.decision),
                "reason":result.reason,"details":result.details,"next_relic_id":result.next.as_ref().map(|r| &r.relic_id)}),
            );
        }
        Action::Reset => {
            staged.engine =
                Engine::new(load_scanner_v4(DEMO_ACCOUNT, 8)?, staged.evaluator.clone());
            staged.last_result = None;
            staged.last_agent = None;
        }
    }
    staged.revision += 1;
    let snapshot = dto::snapshot(&staged)?;
    if s.cancelled.load(Ordering::SeqCst) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "cancelled",
            "已取消，账号保持操作前的状态。",
        ));
    }
    *committed = staged;
    *s.snapshot.lock().unwrap() = snapshot.clone();
    Ok(snapshot)
}

#[derive(Deserialize)]
struct ModelConfigCommand {
    expected_revision: u64,
    #[serde(flatten)]
    patch: ModelConfigPatch,
}

async fn update_model_config(
    State(app): State<App>,
    headers: HeaderMap,
    payload: std::result::Result<Json<ModelConfigCommand>, axum::extract::rejection::JsonRejection>,
) -> ApiResult<Json<Value>> {
    let command = payload
        .map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_model_config",
                "模型配置格式不正确。",
            )
        })?
        .0;
    let session = app.session(&headers)?;
    let mut data = session.data.lock().unwrap();
    if data.revision != command.expected_revision {
        return Err(stale_revision());
    }
    data.model_config.apply(command.patch).map_err(|message| {
        ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_model_config",
            message,
        )
    })?;
    data.revision += 1;
    let snapshot = dto::snapshot(&data)?;
    *session.snapshot.lock().unwrap() = snapshot.clone();
    Ok(Json(snapshot))
}

#[derive(Deserialize)]
struct AgentCommand {
    expected_revision: u64,
    input: String,
}

async fn agent(
    State(app): State<App>,
    headers: HeaderMap,
    payload: std::result::Result<Json<AgentCommand>, axum::extract::rejection::JsonRejection>,
) -> ApiResult<Json<Value>> {
    let command = payload
        .map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_agent_input",
                "Agent 输入格式不正确。",
            )
        })?
        .0;
    let session = app.session(&headers)?;
    if session
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "busy",
            "上一项操作仍在进行，请稍候或取消。",
        ));
    }
    session.cancelled.store(false, Ordering::SeqCst);
    *session.started.lock().unwrap() = Some(Instant::now());
    tokio::task::spawn_blocking(move || {
        let result = execute_agent(&session, command);
        *session.started.lock().unwrap() = None;
        session.running.store(false, Ordering::SeqCst);
        result.map(Json)
    })
    .await
    .map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "worker_failed",
            "Agent 任务异常，请稍后重试。",
        )
    })?
}

fn execute_agent(session: &Session, command: AgentCommand) -> ApiResult<Value> {
    let mut data = session.data.lock().unwrap();
    if data.revision != command.expected_revision {
        return Err(stale_revision());
    }
    let before_calls = data.usage.summary().calls;
    let mut engine = data.engine.clone();
    let config = data.model_config.clone();
    let runtime = AgentRuntime::new(OpenAiCompatibleProvider::default());
    let result = runtime.run(
        &command.input,
        &config,
        &mut data.usage,
        &mut engine,
        &session.cancelled,
    );
    match result {
        Ok(run) => {
            data.engine = engine;
            data.last_agent = Some(run);
            data.revision += 1;
        }
        Err(error) => {
            if data.usage.summary().calls != before_calls {
                data.revision += 1;
            }
            let snapshot = dto::snapshot(&data)?;
            *session.snapshot.lock().unwrap() = snapshot;
            return Err(agent_error(error));
        }
    }
    let snapshot = dto::snapshot(&data)?;
    *session.snapshot.lock().unwrap() = snapshot.clone();
    Ok(snapshot)
}

fn stale_revision() -> ApiError {
    ApiError::new(
        StatusCode::CONFLICT,
        "stale_revision",
        "账号或配置已更新；页面已同步，请核对后重新操作。",
    )
}

fn agent_error(error: RuntimeError) -> ApiError {
    let (status, code) = match error {
        RuntimeError::InvalidInput(_) => (StatusCode::BAD_REQUEST, "invalid_agent_input"),
        RuntimeError::ModelNotConfigured => {
            (StatusCode::UNPROCESSABLE_ENTITY, "model_not_configured")
        }
        RuntimeError::BudgetReached { .. } => (StatusCode::CONFLICT, "token_budget_reached"),
        RuntimeError::Cancelled => (StatusCode::CONFLICT, "cancelled"),
        RuntimeError::Provider(_) => (StatusCode::BAD_GATEWAY, "model_provider_failed"),
        RuntimeError::ToolLoopLimit => (StatusCode::UNPROCESSABLE_ENTITY, "tool_loop_limit"),
        RuntimeError::MissingToolCall => (StatusCode::UNPROCESSABLE_ENTITY, "tool_call_required"),
        RuntimeError::MissingReply => (StatusCode::UNPROCESSABLE_ENTITY, "model_reply_missing"),
    };
    ApiError::new(status, code, error.to_string())
}

async fn progress(State(app): State<App>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    let s = app.session(&headers)?;
    let seconds = s
        .started
        .lock()
        .unwrap()
        .map_or(0, |t| t.elapsed().as_secs());
    Ok(Json(
        json!({"running":s.running.load(Ordering::SeqCst), "seconds":seconds}),
    ))
}
async fn cancel(State(app): State<App>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    let s = app.session(&headers)?;
    if s.running.load(Ordering::SeqCst) {
        s.cancelled.store(true, Ordering::SeqCst);
    }
    Ok(Json(json!({"cancellation_requested":true})))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_prevents_commit_and_busy_session_still_reports_progress() {
        let app = App::new(true);
        let Json(created) = create_session(State(app.clone())).await.ok().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-demo-session",
            created["session"].as_str().unwrap().parse().unwrap(),
        );
        let session = app.session(&headers).ok().unwrap();
        session.running.store(true, Ordering::SeqCst);
        *session.started.lock().unwrap() = Some(Instant::now());
        let Json(p) = progress(State(app.clone()), headers.clone())
            .await
            .ok()
            .unwrap();
        assert_eq!(p["running"], true);
        let result = action(
            State(app.clone()),
            headers.clone(),
            Ok(Json(Command {
                expected_revision: 0,
                action: Action::Reset,
            })),
        )
        .await;
        assert_eq!(result.err().unwrap().code, "busy");
        let _ = cancel(State(app), headers).await.ok().unwrap();
        let error = execute(
            &session,
            Command {
                expected_revision: 0,
                action: Action::Target {
                    character_id: "1205".into(),
                },
            },
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "cancelled");
        assert_eq!(*session.snapshot.lock().unwrap(), created["state"]);
        let committed = session.data.lock().unwrap();
        assert_eq!(committed.revision, 0);
        assert!(committed.engine.goal().is_none());
    }
}
