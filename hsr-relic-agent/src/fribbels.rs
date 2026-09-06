//! Versioned, UI-independent subprocess boundary. No Fribbels types enter the core.
use crate::*;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub const FRIBBELS_COMMIT: &str = "df630a0488a64eeb740e4e0c14f265d96b9f6f8f";
const SLOTS: [Slot; 6] = [
    Slot::Head,
    Slot::Hands,
    Slot::Body,
    Slot::Feet,
    Slot::Sphere,
    Slot::Rope,
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombatConditions {
    pub preset: String,
    pub enemy_level: u8,
    pub enemy_resistance_pct: f64,
    pub elemental_weakness: bool,
    pub weakness_broken: bool,
}

impl Default for CombatConditions {
    fn default() -> Self {
        Self {
            preset: "solo-default-v1".into(),
            enemy_level: 95,
            enemy_resistance_pct: 20.0,
            elemental_weakness: true,
            weakness_broken: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelicMetrics {
    pub id: String,
    pub raw_current_score: f64,
    pub rating: String,
    /// Potential-compatible current score, including upstream main-stat penalties.
    pub current: f64,
    pub average: f64,
    pub best: f64,
    pub worst: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildPanel {
    pub hp: f64,
    pub atk: f64,
    pub def: f64,
    pub speed: f64,
    pub crit_rate_pct: f64,
    pub crit_damage_pct: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageModel {
    /// Pinned unbuffed Blade/Seele configs only implement BASIC (100% ATK) and BREAK.
    LegacyAtkBasicV1,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildMetrics {
    pub id: String,
    /// Basic (out-of-combat) stats, not the primary action's buffed panel.
    pub panel: BuildPanel,
    pub damage_model: DamageModel,
    /// Upstream BASIC expected damage at the pinned preset, not DPS or team damage.
    pub basic_damage: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceBuild {
    pub relic_ids: Vec<String>,
    /// Missing equipped slots filled by first eligible inventory ID, NOT optimized.
    pub assumed_slots: Vec<Slot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationDetails {
    pub upstream_commit: String,
    pub relic: RelicMetrics,
    pub reference: ReferenceBuild,
    pub baseline_slot_score: f64,
    pub reference_build: BuildMetrics,
    pub candidate_build: BuildMetrics,
    pub conditions: CombatConditions,
}

impl EvaluationDetails {
    pub fn damage_ratio(&self) -> f64 {
        self.candidate_build.basic_damage / self.reference_build.basic_damage
    }
}

#[derive(Debug, Clone, Copy)]
pub enum EvaluationProgress {
    Started,
    Waiting { seconds: u64 },
    Finished,
}

pub struct FribbelsConfig {
    pub node: PathBuf,
    pub adapter: PathBuf,
    pub timeout: Duration,
    pub conditions: CombatConditions,
    pub cancelled: Arc<AtomicBool>,
    pub progress: Option<Arc<dyn Fn(EvaluationProgress) + Send + Sync>>,
}

impl Default for FribbelsConfig {
    fn default() -> Self {
        Self {
            node: "node".into(),
            adapter: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../adapters/fribbels/dist/adapter.mjs"),
            timeout: Duration::from_secs(20),
            conditions: CombatConditions::default(),
            cancelled: Arc::new(AtomicBool::new(false)),
            progress: None,
        }
    }
}

#[derive(Serialize)]
struct CharacterInput<'a> {
    id: &'a str,
    level: u8,
    eidolon: u8,
    light_cone: &'a LightCone,
}
#[derive(Serialize)]
struct RelicInput<'a> {
    id: &'a str,
    slot: Slot,
    set_id: &'a str,
    rarity: u8,
    level: u8,
    main_stat: Stat,
    substats: &'a BTreeMap<Stat, f64>,
}
#[derive(Serialize)]
struct BuildInput {
    id: String,
    relic_ids: Vec<String>,
}
#[derive(Serialize)]
struct Request<'a> {
    schema_version: u8,
    character: CharacterInput<'a>,
    relics: Vec<RelicInput<'a>>,
    builds: Vec<BuildInput>,
    conditions: &'a CombatConditions,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    schema_version: u8,
    upstream_commit: String,
    ok: bool,
    result: Option<Batch>,
    error: Option<ToolError>,
}
#[derive(Deserialize)]
struct ToolError {
    code: String,
    message: String,
}
#[derive(Deserialize)]
struct Batch {
    scores: Vec<RelicMetrics>,
    builds: Vec<BuildMetrics>,
}
type CachedBatch = (Vec<u8>, BTreeMap<String, EvaluationDetails>);

pub struct FribbelsEvaluator {
    config: FribbelsConfig,
    // Account stats, goal, LC, build IDs and conditions are all in the key.
    // History/budget/status are deliberately excluded; they are Rust policy inputs.
    cache: RefCell<VecDeque<CachedBatch>>,
}

impl FribbelsEvaluator {
    pub fn new(config: FribbelsConfig) -> Self {
        Self {
            config,
            cache: RefCell::new(VecDeque::new()),
        }
    }

    pub fn reference_build(
        account: &AccountState,
        goal: &CultivationGoal,
    ) -> Result<ReferenceBuild> {
        let mut reference = ReferenceBuild {
            relic_ids: vec![],
            assumed_slots: vec![],
        };
        for slot in SLOTS {
            let equipped: Vec<_> = account
                .relics
                .values()
                .filter(|r| r.slot == slot && r.equipped_by.as_ref() == Some(&goal.character_id))
                .collect();
            if equipped.len() > 1 {
                return Err(Error(format!("同部位重复装备：{slot:?}")));
            }
            let chosen = if let Some(relic) = equipped.first() {
                *relic
            } else {
                reference.assumed_slots.push(slot);
                account
                    .relics
                    .values()
                    .find(|r| {
                        r.slot == slot && r.equipped_by.is_none() && !r.locked && !r.discarded
                    })
                    .ok_or_else(|| {
                        Error(format!(
                            "无法构造六件参考 Build：缺少可用的 {slot:?}，未伪造装备"
                        ))
                    })?
            };
            reference.relic_ids.push(chosen.id.clone());
        }
        Ok(reference)
    }

    pub fn metrics(
        &self,
        account: &AccountState,
        goal: &CultivationGoal,
        relic: &Relic,
    ) -> Result<EvaluationDetails> {
        if self.config.cancelled.load(Ordering::Relaxed) {
            return Err(Error("Fribbels 评价已取消".into()));
        }
        if account.relics.get(&relic.id) != Some(relic) {
            return Err(Error("待评价遗器必须与当前 AccountState 一致".into()));
        }
        let character = account
            .characters
            .get(&goal.character_id)
            .ok_or_else(|| Error("目标角色不存在".into()))?;
        if character.id != goal.character_id {
            return Err(Error("角色 ID 与 AccountState 索引不一致".into()));
        }
        let light_cone = character
            .light_cone
            .as_ref()
            .ok_or_else(|| Error("Fribbels v0.2 需要目标角色装备光锥".into()))?;
        let reference = Self::reference_build(account, goal)?;
        let mut builds = vec![BuildInput {
            id: "reference".into(),
            relic_ids: reference.relic_ids.clone(),
        }];
        for item in account.relics.values() {
            let mut ids = reference.relic_ids.clone();
            ids[SLOTS.iter().position(|s| *s == item.slot).unwrap()] = item.id.clone();
            builds.push(BuildInput {
                id: format!("candidate:{}", item.id),
                relic_ids: ids,
            });
        }
        let request = Request {
            schema_version: 1,
            character: CharacterInput {
                id: &character.id,
                level: character.level,
                eidolon: character.eidolon,
                light_cone,
            },
            relics: account
                .relics
                .values()
                .map(|r| RelicInput {
                    id: &r.id,
                    slot: r.slot,
                    set_id: &r.set_id,
                    rarity: r.rarity,
                    level: r.level,
                    main_stat: r.main_stat,
                    substats: &r.substats,
                })
                .collect(),
            builds,
            conditions: &self.config.conditions,
        };
        let key = serde_json::to_vec(&request).map_err(|e| Error(e.to_string()))?;
        if let Some((_, batch)) = self.cache.borrow().iter().find(|(old, _)| *old == key) {
            let mut details = batch[&relic.id].clone();
            details.reference = reference;
            return Ok(details);
        }
        let batch = self.call(&key)?;
        let scores: BTreeMap<_, _> = batch.scores.iter().map(|s| (s.id.clone(), s)).collect();
        let builds: BTreeMap<_, _> = batch.builds.iter().map(|b| (b.id.clone(), b)).collect();
        if batch.scores.len() != account.relics.len()
            || scores.len() != batch.scores.len()
            || builds.len() != account.relics.len() + 1
            || builds.len() != batch.builds.len()
        {
            return Err(Error("Fribbels 返回重复或不完整的结果".into()));
        }
        let mut details = BTreeMap::new();
        for item in account.relics.values() {
            let score = scores
                .get(&item.id)
                .ok_or_else(|| Error("Fribbels 缺少遗器评分".into()))?;
            validate_score(score)?;
            let baseline_id =
                &reference.relic_ids[SLOTS.iter().position(|s| *s == item.slot).unwrap()];
            let baseline = scores
                .get(baseline_id)
                .ok_or_else(|| Error("Fribbels 缺少基线评分".into()))?;
            let reference_build = builds
                .get("reference")
                .ok_or_else(|| Error("Fribbels 缺少参考 Build".into()))?;
            let candidate = builds
                .get(&format!("candidate:{}", item.id))
                .ok_or_else(|| Error("Fribbels 缺少候选 Build".into()))?;
            validate_build(reference_build)?;
            validate_build(candidate)?;
            if reference_build.basic_damage <= 0.0 {
                return Err(Error("参考 Build 普攻伤害为零，无法比较".into()));
            }
            details.insert(
                item.id.clone(),
                EvaluationDetails {
                    upstream_commit: FRIBBELS_COMMIT.into(),
                    relic: (*score).clone(),
                    reference: reference.clone(),
                    baseline_slot_score: baseline.current,
                    reference_build: (*reference_build).clone(),
                    candidate_build: (*candidate).clone(),
                    conditions: self.config.conditions.clone(),
                },
            );
        }
        let answer = details[&relic.id].clone();
        let mut cache = self.cache.borrow_mut();
        if cache.len() == 4 {
            cache.pop_front();
        }
        cache.push_back((key, details));
        Ok(answer)
    }

    fn call(&self, input: &[u8]) -> Result<Batch> {
        if !self.config.adapter.is_file() {
            return Err(Error("找不到 Fribbels Adapter；请在 workspace 根目录运行 node adapters/fribbels/build.mjs（不会自动退回 Mock）".into()));
        }
        self.progress(EvaluationProgress::Started);
        let result = self.run_process(input);
        self.progress(EvaluationProgress::Finished);
        result
    }

    fn progress(&self, event: EvaluationProgress) {
        if let Some(callback) = &self.config.progress {
            callback(event);
        }
    }

    fn run_process(&self, input: &[u8]) -> Result<Batch> {
        let mut child = Command::new(&self.config.node)
            .arg(&self.config.adapter)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error(format!("无法启动 Node：{e}")))?;
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let (status, output, errors) = thread::scope(|scope| -> Result<_> {
            let output = scope.spawn(|| read_limited(stdout));
            let errors = scope.spawn(|| read_limited(stderr));
            let writer = scope.spawn(move || stdin.write_all(input));
            let started = Instant::now();
            let mut reported = 0;
            let status = loop {
                let cancelled = self.config.cancelled.load(Ordering::Relaxed);
                if cancelled || started.elapsed() >= self.config.timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(Error(
                        if cancelled {
                            "Fribbels 评价已取消"
                        } else {
                            "Fribbels 评价超时，子进程已终止"
                        }
                        .into(),
                    ));
                }
                match child.try_wait() {
                    Ok(Some(status)) => break Ok(status),
                    Ok(None) => (),
                    Err(error) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        break Err(Error(format!("等待 Fribbels 失败：{error}")));
                    }
                }
                let seconds = started.elapsed().as_secs();
                if seconds > reported {
                    self.progress(EvaluationProgress::Waiting { seconds });
                    reported = seconds;
                }
                thread::sleep(Duration::from_millis(10));
            };
            let written = writer
                .join()
                .map_err(|_| Error("Adapter 输入线程失败".into()))?;
            let out = output
                .join()
                .map_err(|_| Error("Adapter 输出线程失败".into()))??;
            let err = errors
                .join()
                .map_err(|_| Error("Adapter 错误线程失败".into()))??;
            let status = status?;
            written.map_err(|e| Error(format!("写入 Adapter 失败：{e}")))?;
            Ok((status, out, err))
        })?;
        let response: Response = serde_json::from_slice(&output).map_err(|e| {
            Error(format!(
                "Fribbels 返回无效协议 JSON：{e}；{}",
                String::from_utf8_lossy(&errors)
            ))
        })?;
        if response.schema_version != 1 || response.upstream_commit != FRIBBELS_COMMIT {
            return Err(Error(
                "Fribbels 协议或上游版本不匹配，请重新构建固定版本 Adapter".into(),
            ));
        }
        if !response.ok {
            let error = response
                .error
                .ok_or_else(|| Error("Fribbels 错误响应缺少原因".into()))?;
            return Err(Error(format!("Fribbels {}：{}", error.code, error.message)));
        }
        if !status.success() || response.error.is_some() {
            return Err(Error("Fribbels 进程异常退出或响应状态矛盾".into()));
        }
        response
            .result
            .ok_or_else(|| Error("Fribbels 响应缺少 result".into()))
    }
}

fn read_limited(reader: impl Read) -> Result<Vec<u8>> {
    let mut bytes = vec![];
    reader
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error(e.to_string()))?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(Error("Adapter 输出超过 4 MiB".into()));
    }
    Ok(bytes)
}

fn validate_score(s: &RelicMetrics) -> Result<()> {
    if [s.raw_current_score, s.current, s.average, s.best, s.worst]
        .iter()
        .any(|n| !n.is_finite() || *n < 0.0)
        || s.average + 1e-6 < s.current
        || s.best + 1e-6 < s.average
        || s.worst > s.average + 1e-6
    {
        return Err(Error("Fribbels 返回无效评分/潜力".into()));
    }
    Ok(())
}
fn validate_build(b: &BuildMetrics) -> Result<()> {
    let p = &b.panel;
    if [
        p.hp,
        p.atk,
        p.def,
        p.speed,
        p.crit_rate_pct,
        p.crit_damage_pct,
        b.basic_damage,
    ]
    .iter()
    .any(|n| !n.is_finite() || *n < 0.0)
    {
        return Err(Error("Fribbels 返回无效 Build 指标".into()));
    }
    Ok(())
}

impl Evaluator for FribbelsEvaluator {
    fn name(&self) -> &'static str {
        "Fribbels"
    }
    fn minimum_potential(&self) -> f64 {
        20.0
    }
    fn evaluate(
        &self,
        account: &AccountState,
        goal: &CultivationGoal,
        relic: &Relic,
    ) -> Result<Evaluation> {
        let metrics = self.metrics(account, goal, relic)?;
        Ok(Evaluation {
            current_score: metrics.relic.current,
            projected_score: metrics.relic.average,
        })
    }
    fn details(
        &self,
        account: &AccountState,
        goal: &CultivationGoal,
        relic: &Relic,
    ) -> Result<Option<EvaluationDetails>> {
        self.metrics(account, goal, relic).map(Some)
    }
}
