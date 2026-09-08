use crate::ToolDefinition;
use hsr_relic_agent::{
    CultivationGoal, CultivationObjective, CultivationPreferences, DecisionEngine, Evaluator,
    MaterialPressure, Relic, RiskTolerance, UpgradeRecommendation,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub const TOOL_NAMES: [&str; 5] = [
    "set_cultivation_intent",
    "get_current_state",
    "get_relic_candidates",
    "get_next_relic_recommendation",
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
            description: "让 Rust Decision Engine 获取并选中当前目标的下一件推荐遗器。需要先设置目标。".into(),
            parameters: empty_schema(),
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

impl<'a, E: Evaluator> CoreTools<'a, E> {
    pub fn new(engine: &'a mut DecisionEngine<E>) -> Self {
        Self { engine }
    }

    pub fn execute(&mut self, name: &str, arguments: &Value) -> Result<Value, String> {
        if !arguments.is_object() {
            return Err("工具参数必须是 JSON object".into());
        }
        match name {
            "set_cultivation_intent" => self.set_intent(arguments),
            // Kept as a non-advertised compatibility path for older saved model histories.
            "set_target_character" => self.set_target(arguments),
            "get_current_state" => Ok(self.current_state()),
            "get_relic_candidates" => self.candidates(arguments),
            "get_next_relic_recommendation" => self.next_recommendation(),
            "get_upgrade_history" => Ok(self.history()),
            _ => Err(format!("未知 Agent Tool：{name}")),
        }
    }

    fn set_target(&mut self, arguments: &Value) -> Result<Value, String> {
        let requested = arguments
            .get("character")
            .and_then(Value::as_str)
            .ok_or_else(|| "set_target_character 缺少 character".to_string())?;
        let id = resolve_character(self.engine, requested)
            .ok_or_else(|| format!("账号中找不到目标角色 {requested}"))?;
        self.engine.set_goal(&id).map_err(|e| e.to_string())?;
        Ok(intent_result(self.engine))
    }

    fn set_intent(&mut self, arguments: &Value) -> Result<Value, String> {
        let input: SetCultivationIntent = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("培养意图字段无效：{error}"))?;
        let id = resolve_character(self.engine, &input.character)
            .ok_or_else(|| format!("账号中找不到目标角色 {}", input.character))?;
        self.engine
            .set_cultivation_goal(CultivationGoal {
                character_id: id,
                preferences: CultivationPreferences {
                    material_pressure: input.material_pressure,
                    risk_tolerance: input.risk_tolerance,
                    objective: input.objective,
                },
            })
            .map_err(|error| error.to_string())?;
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
        json!({"characters":characters,
            "relic_count":account.relics.len(),
            "light_cone_count":account.light_cones.len(),
            "target":self.engine.goal().and_then(|g| account.characters.get(&g.character_id)).map(|c| json!({"id":c.id,"name":c.name})),
            "cultivation_intent":intent,
            "selected_relic_id":self.engine.selected().map(|r| &r.id),
            "remaining_upgrade_steps":account.upgrade_steps,
            "upgrade_history_count":account.history.len()})
    }

    fn candidates(&self, arguments: &Value) -> Result<Value, String> {
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(3)
            .clamp(1, 10) as usize;
        let ranked = self.engine.rank_candidates().map_err(|e| e.to_string())?;
        let total = ranked.len();
        let candidates: Vec<_> = ranked
            .iter()
            .take(limit)
            .map(|r| recommendation(self.engine, r))
            .collect();
        Ok(json!({"total_candidates":total,"candidates":candidates}))
    }

    fn next_recommendation(&mut self) -> Result<Value, String> {
        let next = self.engine.recommend_next().map_err(|e| e.to_string())?;
        Ok(match next {
            Some(ref recommendation_value) => {
                json!({"recommendation":recommendation(self.engine,recommendation_value)})
            }
            None => json!({"recommendation":null,"reason":"当前没有可强化候选"}),
        })
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
        assert!(invalid.contains("培养意图字段无效"));
        let override_attempt = tools
            .execute(
                "set_cultivation_intent",
                &json!({"character":"Blade","material_pressure":"tight",
                    "risk_tolerance":"conservative","objective":"immediate_power",
                    "priority":999999,"decision":"continue"}),
            )
            .unwrap_err();
        assert!(override_attempt.contains("unknown field"));
        assert!(engine.goal().is_none());
    }
}
