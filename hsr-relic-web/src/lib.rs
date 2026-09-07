mod dto;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use futures_util::{Stream, stream};
use hsr_agent_runtime::{
    AgentRun, AgentRunContext, AgentRuntime, ChatMessage, ModelConfig, ModelConfigPatch,
    ModelConfigView, OpenAiCompatibleProvider, RuntimeError, TraceEvent, UsageLedger,
};
use hsr_relic_agent::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    convert::Infallible,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::broadcast;
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
impl WebEvaluator {
    fn supports_character(&self, character: &Character) -> bool {
        let has_levelled_cone = character
            .light_cone
            .as_ref()
            .is_some_and(|light_cone| light_cone.level == 80);
        character.level == 80
            && has_levelled_cone
            && match self {
                Self::Mock => matches!(character.id.as_str(), "1205" | "1102"),
                Self::Fribbels(_) => true,
            }
    }
}

type Engine = DecisionEngine<WebEvaluator>;
#[derive(Clone)]
struct SessionData {
    engine: Engine,
    initial_account: AccountState,
    import_summary: AccountImportSummary,
    evaluator: WebEvaluator,
    revision: u64,
    last_result: Option<Value>,
    model_config: ModelConfig,
    usage: UsageLedger,
    last_agent: Option<hsr_agent_runtime::AgentRun>,
    conversation: Vec<ChatMessage>,
}
struct Session {
    data: Mutex<SessionData>,
    snapshot: Mutex<Value>,
    cancelled: Arc<AtomicBool>,
    running: AtomicBool,
    started: Mutex<Option<Instant>>,
    touched: Mutex<Instant>,
    traces: Mutex<Vec<AgentRun>>,
    active_trace: Mutex<Option<AgentRun>>,
    events: broadcast::Sender<TraceEvent>,
}
#[derive(Clone)]
pub struct App {
    sessions: Arc<Mutex<HashMap<String, Arc<Session>>>>,
    mock: bool,
    default_model_config: ModelConfig,
    recommendation_database: Option<Arc<CharacterRelicDatabase>>,
    demo_account: AccountState,
    demo_summary: AccountImportSummary,
}
impl App {
    pub fn new(mock: bool) -> Self {
        Self::with_model_config(mock, ModelConfig::default())
    }
    pub fn with_model_config(mock: bool, default_model_config: ModelConfig) -> Self {
        let demo_account = load_scanner_v4(DEMO_ACCOUNT, 8).expect("bundled scanner demo is valid");
        let demo_summary = summary_from_account(&demo_account, "HSR-Scanner", 4, "v1.2.0");
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            mock,
            default_model_config,
            recommendation_database: None,
            demo_account,
            demo_summary,
        }
    }
    pub fn with_recommendation_database(mut self, database: Arc<CharacterRelicDatabase>) -> Self {
        self.recommendation_database = Some(database);
        self
    }
    pub fn with_demo_account(mut self, imported: ImportedAccount) -> Self {
        self.demo_account = imported.account;
        self.demo_summary = imported.summary;
        self
    }
    fn session(&self, headers: &HeaderMap) -> ApiResult<Arc<Session>> {
        let token = headers
            .get("x-demo-session")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        self.session_by_token(token)
    }
    fn session_by_token(&self, token: &str) -> ApiResult<Arc<Session>> {
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
        .route("/api/events", get(events))
        .route("/api/tasks", get(tasks))
        .route("/api/session/export", get(export_session))
        .route("/api/session/import", post(import_session))
        .route("/api/account/demo", post(load_demo_account))
        .route("/api/account/import", post(import_account))
        .route("/api/progress", get(progress))
        .route("/api/cancel", post(cancel))
        .route("/api/health", get(|| async { Json(json!({"ok":true})) }))
        .nest_service(
            "/assets",
            ServeDir::new(root.join("upstream/hsr-optimizer/public/assets")),
        )
        .fallback_service(ServeDir::new(root.join("web")))
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
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
    path: Option<String>,
}
impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            path: None,
        }
    }

    fn from_import(error: AccountImportError) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: error.code.as_str(),
            message: error.message,
            path: error.path,
        }
    }
}

fn summary_from_account(
    account: &AccountState,
    source: &str,
    version: u32,
    build: &str,
) -> AccountImportSummary {
    let equipped_relics = account
        .relics
        .values()
        .filter(|relic| relic.equipped_by.is_some())
        .count();
    AccountImportSummary {
        source: source.into(),
        version,
        build: build.into(),
        characters: account.characters.len(),
        relics_in_file: account.relics.len(),
        relics_imported: account.relics.len(),
        relics_skipped: 0,
        light_cones: account.light_cones.len(),
        equipped_relics,
        imported_equipped_relics: equipped_relics,
        equipped_light_cones: account
            .light_cones
            .values()
            .filter(|light_cone| light_cone.equipped_by.is_some())
            .count(),
        equipment_relations_recognized: true,
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error":{"code":self.code,"message":self.message,"path":self.path}})),
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
    let (events, _) = broadcast::channel(256);
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
    let initial_account = app.demo_account.clone();
    let mut engine = Engine::new(initial_account.clone(), evaluator.clone());
    if let Some(database) = &app.recommendation_database {
        engine = engine.with_recommendation_database(database.clone());
    }
    let data = SessionData {
        engine,
        initial_account,
        import_summary: app.demo_summary.clone(),
        evaluator,
        revision: 0,
        last_result: None,
        model_config: app.default_model_config.clone(),
        usage: UsageLedger::default(),
        last_agent: None,
        conversation: vec![],
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
            traces: Mutex::new(vec![]),
            active_trace: Mutex::new(None),
            events,
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
struct RevisionCommand {
    expected_revision: u64,
}

#[derive(Deserialize)]
struct AccountImportCommand {
    expected_revision: u64,
    account: Value,
}

async fn load_demo_account(
    State(app): State<App>,
    headers: HeaderMap,
    payload: std::result::Result<Json<RevisionCommand>, axum::extract::rejection::JsonRejection>,
) -> ApiResult<Json<Value>> {
    let command = payload
        .map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "示例账号请求格式不正确。",
            )
        })?
        .0;
    let session = app.session(&headers)?;
    replace_account(
        &session,
        command.expected_revision,
        ImportedAccount {
            account: app.demo_account.clone(),
            summary: app.demo_summary.clone(),
        },
    )
    .map(Json)
}

async fn import_account(
    State(app): State<App>,
    headers: HeaderMap,
    payload: std::result::Result<
        Json<AccountImportCommand>,
        axum::extract::rejection::JsonRejection,
    >,
) -> ApiResult<Json<Value>> {
    let session = app.session(&headers)?;
    if session.running.load(Ordering::SeqCst) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "busy",
            "任务运行中，请完成或取消后再导入账号。",
        ));
    }
    let command = payload
        .map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_account_file",
                "账号 JSON 格式不正确或文件超过 8 MiB。",
            )
        })?
        .0;
    let json = serde_json::to_string(&command.account).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_account_file",
            "账号 JSON 无法读取。",
        )
    })?;
    let imported = load_reliquary_v4(&json, 8).map_err(ApiError::from_import)?;
    replace_account(&session, command.expected_revision, imported).map(Json)
}

fn replace_account(
    session: &Session,
    expected_revision: u64,
    imported: ImportedAccount,
) -> ApiResult<Value> {
    if session.running.load(Ordering::SeqCst) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "busy",
            "任务运行中，请完成或取消后再切换账号。",
        ));
    }
    let mut committed = session.data.lock().unwrap();
    if committed.revision != expected_revision {
        return Err(stale_revision());
    }
    let database = committed.engine.recommendation_database_handle();
    let mut engine = Engine::new(imported.account.clone(), committed.evaluator.clone());
    if let Some(database) = database {
        engine = engine.with_recommendation_database(database);
    }
    let mut staged = committed.clone();
    staged.engine = engine;
    staged.initial_account = imported.account;
    staged.import_summary = imported.summary;
    staged.last_result = None;
    staged.revision += 1;
    let snapshot = dto::snapshot(&staged)?;
    *committed = staged;
    *session.snapshot.lock().unwrap() = snapshot.clone();
    Ok(snapshot)
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
            let database = staged.engine.recommendation_database_handle();
            staged.engine = Engine::new(staged.initial_account.clone(), staged.evaluator.clone());
            if let Some(database) = database {
                staged.engine = staged.engine.with_recommendation_database(database);
            }
            staged.last_result = None;
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
    let mut engine = data.engine.clone();
    let config = data.model_config.clone();
    let mut conversation = std::mem::take(&mut data.conversation);
    let runtime = AgentRuntime::new(OpenAiCompatibleProvider::default());
    let mut last_live_sequence = 0;
    let mut on_update = |run: &AgentRun| {
        *session.active_trace.lock().unwrap() = Some(run.clone());
        if let Some(event) = run.events.last()
            && event.sequence > last_live_sequence
        {
            last_live_sequence = event.sequence;
            let _ = session.events.send(event.clone());
        }
    };
    let result = runtime.run_with_context(
        &command.input,
        &config,
        AgentRunContext {
            usage: &mut data.usage,
            engine: &mut engine,
            history: &mut conversation,
            cancelled: &session.cancelled,
            on_update: &mut on_update,
        },
    );
    data.conversation = conversation;
    let (run, error) = match result {
        Ok(run) => {
            data.engine = engine;
            (run, None)
        }
        Err(failure) => {
            if matches!(failure.error, RuntimeError::Cancelled) {
                data.engine = engine;
            }
            (*failure.run, Some(failure.error))
        }
    };
    data.last_agent = Some(run.clone());
    session.traces.lock().unwrap().push(run);
    *session.active_trace.lock().unwrap() = None;
    data.revision += 1;
    let snapshot = dto::snapshot(&data)?;
    *session.snapshot.lock().unwrap() = snapshot.clone();
    if let Some(error) = error {
        return Err(agent_error(error));
    }
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
    let status = match &error {
        RuntimeError::InvalidInput(_) | RuntimeError::InvalidHistory(_) => StatusCode::BAD_REQUEST,
        RuntimeError::ModelNotConfigured => StatusCode::UNPROCESSABLE_ENTITY,
        RuntimeError::BudgetReached { .. } | RuntimeError::Cancelled => StatusCode::CONFLICT,
        RuntimeError::Provider(_) => StatusCode::BAD_GATEWAY,
        RuntimeError::ToolLoopLimit
        | RuntimeError::MissingToolCall
        | RuntimeError::MissingReply => StatusCode::UNPROCESSABLE_ENTITY,
    };
    ApiError::new(status, error.code(), error.to_string())
}

#[derive(Deserialize)]
struct EventQuery {
    session: String,
}

async fn events(
    State(app): State<App>,
    Query(query): Query<EventQuery>,
) -> ApiResult<Sse<impl Stream<Item = std::result::Result<Event, Infallible>>>> {
    let session = app.session_by_token(&query.session)?;
    let stream = stream::unfold(session.events.subscribe(), |mut receiver| async move {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    let data = serde_json::to_string(&event).ok()?;
                    let message = Event::default()
                        .event("trace")
                        .id(format!("{}:{}", event.run_id, event.sequence))
                        .data(data);
                    return Some((Ok(message), receiver));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    ))
}

async fn tasks(State(app): State<App>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    let session = app.session(&headers)?;
    let active = session.active_trace.lock().unwrap().clone();
    let runs = session.traces.lock().unwrap().clone();
    Ok(Json(json!({"active":active,"tasks":runs})))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedEngine {
    account: AccountState,
    goal: Option<CultivationGoal>,
    selected_relic_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedSession {
    schema_version: u32,
    saved_at_unix_ms: u64,
    evaluator: String,
    source_revision: u64,
    engine: SavedEngine,
    #[serde(default)]
    initial_account: Option<AccountState>,
    #[serde(default)]
    account_summary: Option<AccountImportSummary>,
    last_result: Option<Value>,
    model_config: ModelConfigView,
    usage: UsageLedger,
    conversation: Vec<ChatMessage>,
    tasks: Vec<AgentRun>,
}

async fn export_session(State(app): State<App>, headers: HeaderMap) -> ApiResult<Json<Value>> {
    let session = app.session(&headers)?;
    if session.running.load(Ordering::SeqCst) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "busy",
            "任务运行中，请完成或取消后再保存会话。",
        ));
    }
    let data = session.data.lock().unwrap();
    let saved = SavedSession {
        schema_version: 1,
        saved_at_unix_ms: now_ms(),
        evaluator: data.evaluator.name().into(),
        source_revision: data.revision,
        engine: SavedEngine {
            account: data.engine.account().clone(),
            goal: data.engine.goal().cloned(),
            selected_relic_id: data.engine.selected().map(|relic| relic.id.clone()),
        },
        initial_account: Some(data.initial_account.clone()),
        account_summary: Some(data.import_summary.clone()),
        last_result: data.last_result.clone(),
        model_config: data.model_config.view(),
        usage: data.usage.clone(),
        conversation: data.conversation.clone(),
        tasks: session.traces.lock().unwrap().clone(),
    };
    Ok(Json(serde_json::to_value(saved).map_err(|error| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session_export_failed",
            error.to_string(),
        )
    })?))
}

#[derive(Deserialize)]
struct ImportCommand {
    expected_revision: u64,
    session: SavedSession,
}

async fn import_session(
    State(app): State<App>,
    headers: HeaderMap,
    payload: std::result::Result<Json<ImportCommand>, axum::extract::rejection::JsonRejection>,
) -> ApiResult<Json<Value>> {
    let command = payload
        .map_err(|error| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_session_file",
                format!("会话 JSON 格式不正确或超过 4 MiB：{}", error.body_text()),
            )
        })?
        .0;
    let session = app.session(&headers)?;
    if session.running.load(Ordering::SeqCst) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "busy",
            "任务运行中，请完成或取消后再加载会话。",
        ));
    }
    if command.session.schema_version != 1 {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported_session_version",
            "只支持 schema_version 1 的会话文件。",
        ));
    }
    let mut data = session.data.lock().unwrap();
    if data.revision != command.expected_revision {
        return Err(stale_revision());
    }
    if command.session.evaluator != data.evaluator.name() {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "evaluator_mismatch",
            format!(
                "会话使用 {}，当前服务是 {}；请用相同模式启动后加载。",
                command.session.evaluator,
                data.evaluator.name()
            ),
        ));
    }
    command.session.usage.validate().map_err(|message| {
        ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_usage_history",
            message,
        )
    })?;
    validate_history(&command.session.conversation, &command.session.tasks)?;
    let mut model_config = data.model_config.clone();
    let view = command.session.model_config;
    model_config
        .apply(ModelConfigPatch {
            endpoint: view.endpoint,
            api_key: None,
            clear_api_key: false,
            model: view.model,
            context_length: view.context_length,
            reasoning_mode: view.reasoning_mode,
            input_price_per_million: view.input_price_per_million,
            output_price_per_million: view.output_price_per_million,
            token_budget: view.token_budget,
        })
        .map_err(|message| {
            ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_model_config",
                message,
            )
        })?;
    let saved_engine = command.session.engine;
    let restored_account = saved_engine.account;
    let initial_account = command
        .session
        .initial_account
        .unwrap_or_else(|| restored_account.clone());
    let import_summary = command
        .session
        .account_summary
        .unwrap_or_else(|| summary_from_account(&restored_account, "session", 1, "legacy"));
    let mut engine = DecisionEngine::restore(
        restored_account,
        data.evaluator.clone(),
        saved_engine.goal,
        saved_engine.selected_relic_id,
    )?;
    if let Some(database) = data.engine.recommendation_database_handle() {
        engine = engine.with_recommendation_database(database);
    }
    let runs = command.session.tasks;
    let mut staged = SessionData {
        engine,
        initial_account,
        import_summary,
        evaluator: data.evaluator.clone(),
        revision: data.revision + 1,
        last_result: command.session.last_result,
        model_config,
        usage: command.session.usage,
        last_agent: runs.last().cloned(),
        conversation: command.session.conversation,
    };
    let snapshot = dto::snapshot(&staged)?;
    staged.revision = data.revision + 1;
    *data = staged;
    *session.traces.lock().unwrap() = runs;
    *session.active_trace.lock().unwrap() = None;
    *session.snapshot.lock().unwrap() = snapshot.clone();
    Ok(Json(snapshot))
}

fn validate_history(messages: &[ChatMessage], runs: &[AgentRun]) -> ApiResult<()> {
    if messages.len() > 10_000 || runs.len() > 1_000 {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "session_history_too_large",
            "会话历史条目过多。",
        ));
    }
    if !messages.is_empty() && !matches!(messages.first(), Some(ChatMessage::System { .. })) {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_agent_history",
            "Agent 上下文缺少 system message。",
        ));
    }
    for run in runs {
        if run.id.is_empty() || run.events.len() > 10_000 {
            return Err(ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_agent_trace",
                "Agent Trace 包含无效任务。",
            ));
        }
        for (index, event) in run.events.iter().enumerate() {
            if event.run_id != run.id || event.sequence != index as u64 + 1 {
                return Err(ApiError::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_agent_trace",
                    "Agent Trace 的任务 ID 或事件序号无效。",
                ));
            }
        }
    }
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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
