use crate::ToolDefinition;
use hsr_relic_agent::{
    CultivationGoal, CultivationObjective, CultivationPreferences, DecisionEngine, Evaluator,
    MaterialPressure, Relic, RelicOperationError, RiskTolerance, Stat, UpgradeDecision,
    UpgradeRecommendation, UpgradeResult,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const TOOL_NAMES: [&str; 6] = [
    "set_cultivation_intent",
    "get_current_state",
    "get_relic_candidates",
    "get_next_relic_recommendation",
    "record_upgrade_result",
    "get_upgrade_history",
];

pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "set_cultivation_intent".into(),
            description: "把用户自然语言中的单目标培养意图提交给 Rust 校验。三个偏好必须使用受控枚举；Rust 会选择保守、均衡或高潜力策略，不接受评分、排序或决策覆盖。".into(),
            parameters: json!({"type":"object","properties":{
                "character":{"type":"string","description":"角色 ID、英文名或中文名，例如 Blade、刃、Seele、希儿"},
                "material_pressure":{"type":"string","enum":["relaxed","normal","tight"],"description":"材料宽松/一般/紧张；没有表达时用 normal"},
                "risk_tolerance":{"type":"string","enum":["conservative","balanced","aggressive"],"description":"风险保守/均衡/激进；没有表达时用 balanced"},
                "objective":{"type":"string","enum":["immediate_power","balanced","max_potential"],"description":"当前即战力/平衡/满级潜力；没有表达时用 balanced"}
            },"required":["character","material_pressure","risk_tolerance","objective"],"additionalProperties":false}),
        },
        ToolDefinition {
            name: "get_current_state".into(),
            description: "查询可用角色、当前目标、当前选中遗器、剩余强化步数和历史数量。".into(),
            parameters: empty_schema(),
        },
        ToolDefinition {
            name: "get_relic_candidates".into(),
            description: "让 Rust Decision Engine 按当前目标返回候选排序及真实 Evaluator 指标，不改变当前选择。".into(),
            parameters: json!({"type":"object","properties":{"limit":{"type":"integer","minimum":1,"maximum":10,"description":"返回数量，默认 3"}},"additionalProperties":false}),
        },
        ToolDefinition {
            name: "get_next_relic_recommendation".into(),
            description: "让 Rust Decision Engine 获取并选中下一件推荐遗器。只有用户明确说换一件、先不升当前遗器时，才把 exclude_selected 设为 true；这不会伪造 Hold/Stop。".into(),
            parameters: json!({"type":"object","properties":{"exclude_selected":{"type":"boolean","description":"是否排除当前选中遗器；默认 false"}},"additionalProperties":false}),
        },
        ToolDefinition {
            name: "record_upgrade_result".into(),
            description: "记录玩家真实完成的一次 +3 强化观察。Rust 校验当前选择、强化前等级、属性、精确增量、预算和历史，更新同一件遗器并返回 Continue/Hold/Stop。缺少精确属性类别或增量时不要调用，应先查询状态并追问。".into(),
            parameters: json!({"type":"object","properties":{
                "relic_id":{"type":"string","description":"遗器 ID；省略时使用 Rust 当前选中的遗器"},
                "expected_level":{"type":"integer","enum":[0,3,6,9,12],"description":"本次强化前等级；不是强化后的等级。若用户说升到 +6，这里应为 3"},
                "resulting_level":{"type":"integer","enum":[3,6,9,12,15],"description":"玩家观察到的强化后等级；必须恰好比 expected_level 高 3"},
                "stat":{"type":"string","enum":["hp","atk","def","hp_percent","atk_percent","def_percent","speed","crit_rate","crit_damage","effect_hit","effect_res","break_effect"],"description":"实际新增或强化的副属性；生命/攻击/防御必须区分固定值和百分比"},
                "increase":{"type":"number","exclusiveMinimum":0,"description":"本次精确增加量；百分比属性使用百分点，不是强化后的总值"}
            },"required":["expected_level","resulting_level","stat","increase"],"additionalProperties":false}),
        },
        ToolDefinition {
            name: "get_upgrade_history".into(),
            description: "查询本次模拟账号已接受的强化观察历史及 Continue/Hold/Stop 结果。".into(),
            parameters: empty_schema(),
        },
    ]
}

fn empty_schema() -> Value {
    json!({"type":"object","properties":{},"additionalProperties":false})
}

pub struct CoreTools<'a, E> {
    engine: &'a mut DecisionEngine<E>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolError {
    pub code: String,
    pub message: String,
}

impl ToolError {
    fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_arguments", message)
    }

    fn evaluation(error: impl std::fmt::Display) -> Self {
        Self::new("evaluation_failed", error.to_string())
    }
}

impl From<RelicOperationError> for ToolError {
    fn from(error: RelicOperationError) -> Self {
        Self::new(error.code(), error.message())
    }
}

impl<'a, E: Evaluator> CoreTools<'a, E> {
    pub fn new(engine: &'a mut DecisionEngine<E>) -> Self {
        Self { engine }
    }

    pub fn execute(&mut self, name: &str, arguments: &Value) -> Result<Value, ToolError> {
        if !arguments.is_object() {
            return Err(ToolError::invalid("工具参数必须是 JSON object"));
        }
        match name {
            "set_cultivation_intent" => self.set_intent(arguments),
            // Kept as a non-advertised compatibility path for older saved model histories.
            "set_target_character" => self.set_target(arguments),
            "get_current_state" => Ok(self.current_state()),
            "get_relic_candidates" => self.candidates(arguments),
            "get_next_relic_recommendation" => self.next_recommendation(arguments),
            "record_upgrade_result" => self.record_upgrade(arguments),
            "get_upgrade_history" => Ok(self.history()),
            _ => Err(ToolError::new(
                "unknown_tool",
                format!("未知 Agent Tool：{name}"),
            )),
        }
    }

    fn set_target(&mut self, arguments: &Value) -> Result<Value, ToolError> {
        let requested = arguments
            .get("character")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::invalid("set_target_character 缺少 character"))?;
        let id = resolve_character(self.engine, requested).ok_or_else(|| {
            ToolError::new(
                "character_not_found",
                format!("账号中找不到目标角色 {requested}"),
            )
        })?;
        self.engine.set_goal(&id).map_err(ToolError::evaluation)?;
        Ok(intent_result(self.engine))
    }

    fn set_intent(&mut self, arguments: &Value) -> Result<Value, ToolError> {
        let input: SetCultivationIntent = serde_json::from_value(arguments.clone())
            .map_err(|error| ToolError::invalid(format!("培养意图字段无效：{error}")))?;
        let id = resolve_character(self.engine, &input.character).ok_or_else(|| {
            ToolError::new(
                "character_not_found",
                format!("账号中找不到目标角色 {}", input.character),
            )
        })?;
        self.engine
            .set_cultivation_goal(CultivationGoal {
                character_id: id,
                preferences: CultivationPreferences {
                    material_pressure: input.material_pressure,
                    risk_tolerance: input.risk_tolerance,
                    objective: input.objective,
                },
            })
            .map_err(ToolError::evaluation)?;
        Ok(intent_result(self.engine))
    }

    fn current_state(&self) -> Value {
        let account = self.engine.account();
        let characters: Vec<_> = account
            .characters
            .values()
            .map(|c| {
                json!({
                    "id":c.id,"name":c.name,"level":c.level,"eidolon":c.eidolon
                })
            })
            .collect();
        let intent = self.engine.goal().map(|goal| {
            let character = &account.characters[&goal.character_id];
            json!({"character_id":goal.character_id,"character_name":character.name,
                "preferences":goal.preferences,"strategy":goal.strategy(),
                "strategy_explanation":goal.strategy().explanation()})
        });
        let selected_relic = self.engine.selected().map(|relic| {
            json!({"relic_id":relic.id,"level":relic.level,"slot":relic.slot,
                "main_stat":relic.main_stat,"substats":relic.substats})
        });
        let last_observation = account.history.last().map(|record| {
            json!({"relic_id":record.after.id,"before_level":record.before.level,
                "after_level":record.after.level,"stat":record.result.stat,
                "increase":record.result.increase,"decision":decision(record.decision),
                "reason":record.reason})
        });
        json!({"characters":characters,
            "relic_count":account.relics.len(),
            "light_cone_count":account.light_cones.len(),
            "target":self.engine.goal().and_then(|g| account.characters.get(&g.character_id)).map(|c| json!({"id":c.id,"name":c.name})),
            "cultivation_intent":intent,
            "selected_relic_id":self.engine.selected().map(|r| &r.id),
            "selected_relic":selected_relic,
            "remaining_upgrade_steps":account.upgrade_steps,
            "upgrade_history_count":account.history.len(),
            "last_upgrade_observation":last_observation})
    }

    fn candidates(&self, arguments: &Value) -> Result<Value, ToolError> {
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(3)
            .clamp(1, 10) as usize;
        let ranked = self
            .engine
            .rank_candidates()
            .map_err(ToolError::evaluation)?;
        let total = ranked.len();
        let candidates: Vec<_> = ranked
            .iter()
            .take(limit)
            .map(|r| recommendation(self.engine, r))
            .collect();
        Ok(json!({"total_candidates":total,"candidates":candidates}))
    }

    fn next_recommendation(&mut self, arguments: &Value) -> Result<Value, ToolError> {
        let request: NextRecommendation = serde_json::from_value(arguments.clone())
            .map_err(|error| ToolError::invalid(format!("推荐参数无效：{error}")))?;
        let excluded_relic_id = request
            .exclude_selected
            .then(|| self.engine.selected().map(|relic| relic.id.clone()))
            .flatten();
        let next = if request.exclude_selected {
            self.engine.recommend_alternative()
        } else {
            self.engine.recommend_next()
        }
        .map_err(ToolError::evaluation)?;
        Ok(match next {
            Some(ref recommendation_value) => {
                json!({"recommendation":recommendation(self.engine,recommendation_value),
                    "excluded_relic_id":excluded_relic_id})
            }
            None => json!({"recommendation":null,"excluded_relic_id":excluded_relic_id,
                "reason":"当前没有可强化候选"}),
        })
    }

    fn record_upgrade(&mut self, arguments: &Value) -> Result<Value, ToolError> {
        let input: RecordUpgradeResult = serde_json::from_value(arguments.clone())
            .map_err(|error| ToolError::invalid(format!("强化观察字段无效：{error}")))?;
        let relic_id = match input.relic_id {
            Some(relic_id) => relic_id,
            None => self
                .engine
                .selected()
                .map(|relic| relic.id.clone())
                .ok_or_else(|| {
                    ToolError::new("no_relic_selected", "当前没有选中遗器，无法确定强化对象")
                })?,
        };
        let result = self
            .engine
            .apply_upgrade_observation(
                UpgradeResult {
                    relic_id: relic_id.clone(),
                    expected_level: input.expected_level,
                    stat: input.stat,
                    increase: input.increase,
                },
                input.resulting_level,
            )
            .map_err(ToolError::from)?;
        let after = &self.engine.account().relics[&relic_id];
        let next = result
            .next
            .as_ref()
            .map(|item| recommendation(self.engine, item));
        Ok(json!({
            "observation":{"relic_id":relic_id,"before_level":input.expected_level,
                "after_level":after.level,"stat":input.stat,"increase":input.increase},
            "decision":decision(result.decision),"reason":result.reason,
            "details":result.details,"next_recommendation":next,
            "state_update":{"remaining_upgrade_steps":self.engine.account().upgrade_steps,
                "upgrade_history_count":self.engine.account().history.len(),
                "selected_relic_id":self.engine.selected().map(|relic| &relic.id)}
        }))
    }

    fn history(&self) -> Value {
        let history: Vec<_> = self
            .engine
            .account()
            .history
            .iter()
            .map(|record| {
                json!({
                    "character_id":record.goal.character_id,
                    "relic_id":record.after.id,
                    "before_level":record.before.level,
                    "after_level":record.after.level,
                    "stat":record.result.stat,
                    "increase":record.result.increase,
                    "decision":format!("{:?}",record.decision),
                    "reason":record.reason,
                })
            })
            .collect();
        json!({"count":history.len(),"history":history})
    }
}

fn resolve_character<E: Evaluator>(engine: &DecisionEngine<E>, requested: &str) -> Option<String> {
    let normalized = requested.trim().to_lowercase();
    engine
        .account()
        .characters
        .values()
        .find(|character| {
            character.id == requested
                || character.name.to_lowercase() == normalized
                || matches!(
                    (normalized.as_str(), character.id.as_str()),
                    ("刃" | "blade", "1205") | ("希儿" | "seele", "1102")
                )
        })
        .map(|character| character.id.clone())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetCultivationIntent {
    character: String,
    material_pressure: MaterialPressure,
    risk_tolerance: RiskTolerance,
    objective: CultivationObjective,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct NextRecommendation {
    #[serde(default)]
    exclude_selected: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordUpgradeResult {
    #[serde(default)]
    relic_id: Option<String>,
    expected_level: u8,
    resulting_level: u8,
    stat: Stat,
    increase: f64,
}

fn decision(value: UpgradeDecision) -> &'static str {
    match value {
        UpgradeDecision::Continue => "Continue",
        UpgradeDecision::Hold => "Hold",
        UpgradeDecision::Stop => "Stop",
    }
}

fn intent_result<E: Evaluator>(engine: &DecisionEngine<E>) -> Value {
    let goal = engine.goal().expect("goal was just set");
    let character = &engine.account().characters[&goal.character_id];
    json!({"ok":true,"intent":{"character_id":goal.character_id,
        "character_name":character.name,"preferences":goal.preferences},
        "strategy":goal.strategy(),"strategy_explanation":goal.strategy().explanation()})
}

fn recommendation<E: Evaluator>(
    engine: &DecisionEngine<E>,
    recommendation: &UpgradeRecommendation,
) -> Value {
    let relic: Option<&Relic> = engine.account().relics.get(&recommendation.relic_id);
    json!({
        "relic_id":recommendation.relic_id,
        "strategy":recommendation.strategy,
        "set_match":recommendation.set_match,
        "slot":relic.map(|r| r.slot),
        "set_id":relic.map(|r| &r.set_id),
        "level":relic.map(|r| r.level),
        "main_stat":relic.map(|r| r.main_stat),
        "substats":relic.map(|r| &r.substats),
        "current_score":recommendation.current_score,
        "projected_score":recommendation.projected_score,
        "baseline_score":recommendation.baseline_score,
        "priority":recommendation.priority,
        "reason":recommendation.reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hsr_relic_agent::{DEMO_ACCOUNT, MockEvaluator, load_scanner_v4};

    #[test]
    fn tools_wrap_core_and_do_not_reimplement_ranking() {
        let account = load_scanner_v4(DEMO_ACCOUNT, 8).unwrap();
        let mut engine = DecisionEngine::new(account, MockEvaluator);
        let mut tools = CoreTools::new(&mut engine);
        tools
            .execute(
                "set_cultivation_intent",
                &json!({"character":"刃","material_pressure":"normal",
                    "risk_tolerance":"balanced","objective":"balanced"}),
            )
            .unwrap();
        let result = tools
            .execute("get_next_relic_recommendation", &json!({}))
            .unwrap();
        assert_eq!(result["recommendation"]["relic_id"], "9100002");
        assert_eq!(
            tools.execute("get_current_state", &json!({})).unwrap()["selected_relic_id"],
            "9100002"
        );
    }

    #[test]
    fn rust_rejects_invalid_or_model_invented_intent_fields() {
        let account = load_scanner_v4(DEMO_ACCOUNT, 8).unwrap();
        let mut engine = DecisionEngine::new(account, MockEvaluator);
        let mut tools = CoreTools::new(&mut engine);
        let invalid = tools
            .execute(
                "set_cultivation_intent",
                &json!({"character":"Blade","material_pressure":"scarce",
                    "risk_tolerance":"balanced","objective":"balanced"}),
            )
            .unwrap_err();
        assert_eq!(invalid.code, "invalid_arguments");
        assert!(invalid.message.contains("培养意图字段无效"));
        let override_attempt = tools
            .execute(
                "set_cultivation_intent",
                &json!({"character":"Blade","material_pressure":"tight",
                    "risk_tolerance":"conservative","objective":"immediate_power",
                    "priority":999999,"decision":"continue"}),
            )
            .unwrap_err();
        assert!(override_attempt.message.contains("unknown field"));
        assert!(engine.goal().is_none());
    }

    #[test]
    fn record_upgrade_uses_the_same_core_state_transition() {
        let account = load_scanner_v4(DEMO_ACCOUNT, 8).unwrap();
        let mut engine = DecisionEngine::new(account, MockEvaluator);
        engine.set_goal("1205").unwrap();
        engine.recommend_next().unwrap();
        let result = CoreTools::new(&mut engine)
            .execute(
                "record_upgrade_result",
                &json!({"expected_level":3,"resulting_level":6,
                    "stat":"crit_rate","increase":3.24}),
            )
            .unwrap();
        assert_eq!(result["observation"]["relic_id"], "9100002");
        assert_eq!(result["observation"]["after_level"], 6);
        assert_eq!(result["decision"], "Continue");
        assert_eq!(result["state_update"]["remaining_upgrade_steps"], 7);
        assert_eq!(result["state_update"]["upgrade_history_count"], 1);
        assert_eq!(engine.account().relics["9100002"].level, 6);
        assert_eq!(engine.account().history.len(), 1);
    }

    #[test]
    fn invalid_upgrade_feedback_is_rejected_transactionally() {
        let account = load_scanner_v4(DEMO_ACCOUNT, 8).unwrap();
        let mut engine = DecisionEngine::new(account, MockEvaluator);
        engine.set_goal("1205").unwrap();
        engine.recommend_next().unwrap();
        let before = engine.account().clone();

        let missing = CoreTools::new(&mut engine)
            .execute(
                "record_upgrade_result",
                &json!({"expected_level":3,"resulting_level":6,"stat":"crit_rate"}),
            )
            .unwrap_err();
        assert_eq!(missing.code, "invalid_arguments");

        let invalid_transition = CoreTools::new(&mut engine)
            .execute(
                "record_upgrade_result",
                &json!({"expected_level":3,"resulting_level":9,
                    "stat":"crit_rate","increase":3.24}),
            )
            .unwrap_err();
        assert_eq!(invalid_transition.code, "invalid_level_transition");

        let stale = CoreTools::new(&mut engine)
            .execute(
                "record_upgrade_result",
                &json!({"expected_level":0,"resulting_level":3,
                    "stat":"crit_rate","increase":3.24}),
            )
            .unwrap_err();
        assert_eq!(stale.code, "stale_upgrade");

        let missing_relic = CoreTools::new(&mut engine)
            .execute(
                "record_upgrade_result",
                &json!({"relic_id":"missing","expected_level":3,"resulting_level":6,
                    "stat":"crit_rate","increase":3.24}),
            )
            .unwrap_err();
        assert_eq!(missing_relic.code, "relic_not_found");
        assert_eq!(*engine.account(), before);
        assert_eq!(engine.selected().unwrap().id, "9100002");
    }

    #[test]
    fn record_upgrade_returns_hold_stop_and_budget_outcomes_from_core() {
        let account = load_scanner_v4(DEMO_ACCOUNT, 8).unwrap();
        let mut hold_engine = DecisionEngine::new(account.clone(), MockEvaluator);
        hold_engine.set_goal("1205").unwrap();
        hold_engine.select_relic("9100001").unwrap();
        let hold = CoreTools::new(&mut hold_engine)
            .execute(
                "record_upgrade_result",
                &json!({"expected_level":0,"resulting_level":3,
                    "stat":"def_percent","increase":5.4}),
            )
            .unwrap();
        assert_eq!(hold["decision"], "Hold");
        assert_eq!(hold["next_recommendation"]["relic_id"], "9100002");

        let mut stop_engine = DecisionEngine::new(account.clone(), MockEvaluator);
        stop_engine.set_goal("1205").unwrap();
        stop_engine.select_relic("9100003").unwrap();
        let stop = CoreTools::new(&mut stop_engine)
            .execute(
                "record_upgrade_result",
                &json!({"expected_level":6,"resulting_level":9,
                    "stat":"def_percent","increase":5.4}),
            )
            .unwrap();
        assert_eq!(stop["decision"], "Stop");
        assert_ne!(stop["next_recommendation"]["relic_id"], "9100003");

        let mut one_step = account;
        one_step.upgrade_steps = 1;
        let mut budget_engine = DecisionEngine::new(one_step, MockEvaluator);
        budget_engine.set_goal("1205").unwrap();
        budget_engine.select_relic("9100002").unwrap();
        let budget = CoreTools::new(&mut budget_engine)
            .execute(
                "record_upgrade_result",
                &json!({"expected_level":3,"resulting_level":6,
                    "stat":"crit_rate","increase":3.24}),
            )
            .unwrap();
        assert_eq!(budget["decision"], "Hold");
        assert_eq!(budget["state_update"]["remaining_upgrade_steps"], 0);
        assert!(budget["next_recommendation"].is_null());
    }

    #[test]
    fn explicit_switch_selects_the_best_alternative_without_faking_a_decision() {
        let account = load_scanner_v4(DEMO_ACCOUNT, 8).unwrap();
        let mut engine = DecisionEngine::new(account, MockEvaluator);
        engine.set_goal("1205").unwrap();
        engine.recommend_next().unwrap();
        let current = engine.selected().unwrap().id.clone();
        let alternative = CoreTools::new(&mut engine)
            .execute(
                "get_next_relic_recommendation",
                &json!({"exclude_selected":true}),
            )
            .unwrap();
        assert_eq!(alternative["excluded_relic_id"], current);
        assert_ne!(alternative["recommendation"]["relic_id"], current);
        assert!(engine.account().decisions.is_empty());
    }
}
