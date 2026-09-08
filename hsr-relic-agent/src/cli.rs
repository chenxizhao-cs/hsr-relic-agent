use std::io::{self, Write};

use hsr_relic_agent::{
    DEMO_ACCOUNT, DecisionEngine, Error, EvaluationProgress, Evaluator, FribbelsConfig,
    FribbelsEvaluator, MockEvaluator, RelicOperationError, RelicSelection, Stat,
    UpgradeRecommendation, UpgradeResult, load_character_relic_database, load_scanner_v4,
};
use std::sync::Arc;

type Engine = DecisionEngine<Box<dyn Evaluator>>;
type CliResult<T> = std::result::Result<T, CliError>;

#[derive(Debug)]
pub(crate) enum CliError {
    Core(Error),
    RelicOperation(RelicOperationError),
}

impl From<Error> for CliError {
    fn from(error: Error) -> Self {
        Self::Core(error)
    }
}

impl From<RelicOperationError> for CliError {
    fn from(error: RelicOperationError) -> Self {
        Self::RelicOperation(error)
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Core(error) => error.fmt(f),
            Self::RelicOperation(error) => f.write_str(&operation_message(error)),
        }
    }
}

fn operation_message(error: &RelicOperationError) -> String {
    match error {
        RelicOperationError::TargetNotSelected => {
            "尚未选择目标角色，请先输入 target Blade 或 target Seele。".into()
        }
        RelicOperationError::RelicNotFound { relic_id } => {
            format!("找不到遗器 {relic_id}，请检查 ID。")
        }
        RelicOperationError::NoRelicSelected => {
            "当前没有选中的遗器，请先使用 next 或 choose ID。".into()
        }
        RelicOperationError::DifferentRelicSelected {
            selected_relic_id,
            result_relic_id,
        } => format!(
            "当前选中的是遗器 {selected_relic_id}，不能把遗器 {result_relic_id} 的强化结果记到它上面。"
        ),
        RelicOperationError::BudgetExhausted => "强化预算已经用完，无法再选择或强化遗器。".into(),
        RelicOperationError::Discarded { relic_id } => {
            format!("遗器 {relic_id} 已标记为 discard（遗弃），不能选择或强化。")
        }
        RelicOperationError::Locked { relic_id } => {
            format!("遗器 {relic_id} 已锁定，解除锁定后才能选择或强化。")
        }
        RelicOperationError::MaxLevel { relic_id, level } => {
            format!("遗器 {relic_id} 已经达到 +{level}，不能继续强化。")
        }
        RelicOperationError::EquippedByOtherCharacter {
            relic_id,
            character_id,
        } => format!("遗器 {relic_id} 正装备在其他角色 {character_id} 身上，当前规则不允许操作。"),
        RelicOperationError::SetNotRecommendedForTarget {
            relic_id,
            character_id,
            set_id,
        } => format!(
            "遗器 {relic_id} 的套装 {set_id} 未列入目标角色 {character_id} 的游戏静态推荐，不能作为本轮强化候选。"
        ),
        RelicOperationError::StoppedForTarget {
            relic_id,
            character_id,
        } => format!("遗器 {relic_id} 对当前目标 {character_id} 已判定为 Stop，请选择其他候选。"),
        RelicOperationError::HoldRequiresExplicitResume {
            relic_id,
            character_id,
        } => format!(
            "遗器 {relic_id} 对当前目标 {character_id} 处于 Hold；请先输入 choose {relic_id} 显式恢复，再录入强化结果。"
        ),
        RelicOperationError::StaleUpgradeResult {
            relic_id,
            reported_level,
            current_level,
        } => format!(
            "遗器 {relic_id} 当前是 +{current_level}，收到的结果基于 +{reported_level}；已拒绝这条过期或重复结果。"
        ),
        RelicOperationError::InvalidLevelTransition {
            from_level,
            to_level,
        } => format!("不能把一次强化记录为 +{from_level} → +{to_level}；每次只能增加 3 级。"),
        RelicOperationError::InvalidIncrease => "强化增量必须是大于 0 的有限数值。".into(),
        RelicOperationError::InvalidSubstat { stat } => {
            format!("{stat:?} 不是可录入的遗器副属性。")
        }
        RelicOperationError::MainStatConflict { stat } => {
            format!("{stat:?} 是这件遗器的主属性，不能同时作为副属性录入。")
        }
        RelicOperationError::MustAddFourthSubstat => {
            "这件遗器当前只有三条副属性，本次强化必须录入新增的第四条副属性。".into()
        }
        RelicOperationError::MustUpgradeExistingSubstat => {
            "这件遗器已有四条副属性，本次强化只能增加其中一条已有副属性。".into()
        }
        RelicOperationError::InvalidSubstatCount { count } => {
            format!("这件遗器当前有 {count} 条副属性，状态不符合当前 Demo 的强化规则。")
        }
        RelicOperationError::StatOverflow { stat } => {
            format!("{stat:?} 的结果超出可表示范围，账号状态未更新。")
        }
        RelicOperationError::EvaluationFailed(error) => {
            format!("重新评价遗器失败：{error}")
        }
    }
}

pub(crate) fn run() -> CliResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "cargo run：Fribbels 自动演示；--interactive：交互操作；--mock：使用 v0.1 Mock 对照。\n先在 workspace 根目录依次执行 node adapters/recommendations/prepare.mjs 和 node adapters/fribbels/build.mjs。\n每次启动从同一 fixture 加载，内存状态不写回文件；Ctrl+C 中断。"
        );
        return Ok(());
    }
    if args
        .iter()
        .any(|arg| !["--interactive", "--mock"].contains(&arg.as_str()))
    {
        return Err(Error("仅支持 --interactive / --mock / --help".into()).into());
    }
    let interactive = args.iter().any(|a| a == "--interactive");
    let evaluator: Box<dyn Evaluator> = if args.iter().any(|a| a == "--mock") {
        println!("HSR 遗器强化 Demo v0.2 — Mock 对照，不代表真实战斗收益");
        Box::new(MockEvaluator)
    } else {
        println!("HSR 遗器强化 Demo v0.2 — Fribbels 真实评分 / 潜力 / 单角色 Build");
        println!(
            "伤害口径：无队友、95 级单体、有属性弱点且未击破，上游默认光锥/套装开关、满行迹。当前旧版 Blade/Seele 仅有简化普通攻击（100% ATK），不含 Blade 强化普攻/完整技能，非实战 DPS。预算/决策阈值仍为 Demo 规则。"
        );
        let config = FribbelsConfig {
            progress: Some(std::sync::Arc::new(|event| {
                if let EvaluationProgress::Waiting { seconds } = event {
                    eprintln!("Fribbels 正在计算（{seconds} 秒），Ctrl+C 可中断……");
                }
            })),
            ..FribbelsConfig::default()
        };
        Box::new(FribbelsEvaluator::new(config))
    };
    let database_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../data/.generated/character-relic-recommendations-v1.json");
    let database_json = std::fs::read_to_string(&database_path).map_err(|_| {
        Error("缺少静态推荐数据库；请在 workspace 根目录执行 node adapters/recommendations/prepare.mjs".into())
    })?;
    let database = Arc::new(load_character_relic_database(&database_json)?);
    let mut engine = Engine::new(load_scanner_v4(DEMO_ACCOUNT, 8)?, evaluator)
        .with_recommendation_database(database.clone());
    println!(
        "已加载 fixtures/scanner-v4-demo.json：{} 个角色 / {} 件遗器，预算 {} 步；静态推荐数据库 {} 个角色。",
        engine.account().characters.len(),
        engine.account().relics.len(),
        engine.account().upgrade_steps,
        database.len()
    );
    if interactive {
        interact(&mut engine)
    } else {
        demo(&mut engine)
    }
}

fn show_recommendation(recommendation: Option<&UpgradeRecommendation>) {
    if let Some(r) = recommendation {
        println!("推荐 {}：{}", r.relic_id, r.reason);
        if let Some(d) = &r.details {
            let p = &d.candidate_build.panel;
            println!(
                "  遗器原始评分 {:.2}（{}）；满级潜力 worst / average / best：{:.2} / {:.2} / {:.2}",
                d.relic.raw_current_score,
                d.relic.rating,
                d.relic.worst,
                d.relic.average,
                d.relic.best
            );
            println!(
                "  候选替换后的基础面板：HP {:.0} / ATK {:.0} / DEF {:.0} / SPD {:.2} / 暴击 {:.2}% / 暴伤 {:.2}%",
                p.hp, p.atk, p.def, p.speed, p.crit_rate_pct, p.crit_damage_pct
            );
            println!(
                "  当前等级简化普攻伤害：候选 {:.2} / 参考 {:.2}（不是满级伤害预测，也不是完整角色伤害）",
                d.candidate_build.basic_damage, d.reference_build.basic_damage
            );
            println!(
                "  参考六件：{}；未装备部位按库存 ID 补齐 {:?}，仅作比较假设，未修改账号配装。",
                d.reference.relic_ids.join(", "),
                d.reference.assumed_slots
            );
        }
    } else {
        println!("暂无可自动推荐的候选：可能预算耗尽、已暂缓/停止，或无正向收益。");
    }
}

fn show_selected(engine: &Engine) {
    if let Some(r) = engine.selected() {
        println!(
            "当前 {} +{} / {:?} / 主属性 {:?} / 副属性 {:?}",
            r.id, r.level, r.slot, r.main_stat, r.substats
        );
    }
}

fn set_target(engine: &mut Engine, target: &str) -> CliResult<()> {
    let id = match target.to_ascii_lowercase().as_str() {
        "blade" | "1205" => "1205",
        "seele" | "1102" => "1102",
        _ => return Err(Error("目标需为 Blade / Seele / 1205 / 1102".into()).into()),
    };
    engine.set_goal(id)?;
    println!(
        "\n目标：{} ({id})；保留现有账号、预算与历史。",
        engine.account().characters[id].name
    );
    show_ranking(engine)?;
    show_recommendation(engine.recommend_next()?.as_ref());
    show_selected(engine);
    Ok(())
}

fn show_ranking(engine: &Engine) -> CliResult<()> {
    let ranked = engine.rank_candidates()?;
    println!("候选排序（评分潜力 / 剩余步数，真实模式另计 Build 伤害比）：");
    for (i, r) in ranked.iter().enumerate() {
        println!(
            "  {}. {} +{}，优先级 {:.2}，当前 {:.2} / 预计 {:.2}",
            i + 1,
            r.relic_id,
            engine.account().relics[&r.relic_id].level,
            r.priority,
            r.current_score,
            r.projected_score
        );
    }
    if ranked.is_empty() {
        println!("  无候选");
    }
    Ok(())
}

fn observe(engine: &mut Engine, stat: Stat, increase: f64) -> CliResult<()> {
    let selected = engine
        .selected()
        .ok_or(RelicOperationError::NoRelicSelected)?;
    let id = selected.id.clone();
    let old_level = selected.level;
    let out = engine.apply_upgrade_observation(
        UpgradeResult {
            relic_id: id.clone(),
            expected_level: old_level,
            stat,
            increase,
        },
        old_level.saturating_add(3),
    )?;
    let updated = &engine.account().relics[&id];
    println!(
        "\n已更新同一遗器 {id}：+{old_level} → +{}，{stat:?} 增加 {increase}，现值 {}；剩余 {} 步。",
        updated.level,
        updated.substats[&stat],
        engine.account().upgrade_steps
    );
    println!("判断 {:?}：{}", out.decision, out.reason);
    if let Some(d) = &out.details {
        println!(
            "本件更新后：当前分 {:.2} / 平均潜力 {:.2}，基础 HP {:.0} / ATK {:.0}，简化普攻 {:.2}。",
            d.relic.current,
            d.relic.average,
            d.candidate_build.panel.hp,
            d.candidate_build.panel.atk,
            d.candidate_build.basic_damage
        );
    }
    show_recommendation(out.next.as_ref());
    show_selected(engine);
    Ok(())
}

fn show_history(engine: &Engine) {
    println!(
        "强化历史（{} 条，剩余 {} 步）：",
        engine.account().history.len(),
        engine.account().upgrade_steps
    );
    for (i, record) in engine.account().history.iter().enumerate() {
        println!(
            "  {}. 目标 {} / 同一遗器 {}：+{} → +{}，{:?} +{} → {:?}",
            i + 1,
            record.goal.character_id,
            record.after.id,
            record.before.level,
            record.after.level,
            record.result.stat,
            record.result.increase,
            record.decision
        );
    }
}

fn demo(engine: &mut Engine) -> CliResult<()> {
    println!("\n自动脚本：先比较目标，再录入固定的模拟观察；未使用 fixture 的 preview_substats。");
    set_target(engine, "Seele")?;
    set_target(engine, "Blade")?;
    println!("\n[1] 选择手部候选 9100002，录入一次暴击率增加：");
    engine.select_relic("9100002")?;
    observe(engine, Stat::CritRate, 3.24)?;
    println!("\n[2] 手动观察三词条候选 9100001，录入新增防御百分比：");
    engine.select_relic("9100001")?;
    observe(engine, Stat::DefPercent, 5.4)?;
    println!("\n[3] 显式恢复刚才 Hold 的同一件 9100001，再录入防御百分比增加：");
    engine.select_relic("9100001")?;
    observe(engine, Stat::DefPercent, 5.4)?;
    println!("\n[4] Stop 后已有其他推荐；切换培养目标继续观察：");
    set_target(engine, "Seele")?;
    engine.select_relic("9200002")?;
    observe(engine, Stat::CritRate, 3.24)?;
    show_history(engine);
    println!("\n演示结束。手动操作：cargo run -- --interactive；不修改原始 fixture。");
    Ok(())
}

fn help() {
    println!(
        "\n命令：\n  target Blade|Seele       选择目标，显示排序并推荐\n  rank                    查看当前排序\n  next                    重新推荐并选中\n  choose ID               手动选择；可恢复 Hold，不能恢复当前目标的 Stop\n  upgrade STAT DELTA      当前遗器 +3，录入一次副属性增加（不是最终总值）\n  show                    查看当前遗器与剩余预算\n  history                 查看本次运行的强化记录\n  help / quit             帮助 / 退出\n\nSTAT：hp atk def hp_pct atk_pct def_pct spd cr cd ehr res be\n百分比使用百分点，如 cr 3.24；三词条必须新增第四条，四词条只能增加已有词条。"
    );
}

fn parse_stat(value: &str) -> CliResult<Stat> {
    Ok(match value {
        "hp" => Stat::Hp,
        "atk" => Stat::Atk,
        "def" => Stat::Def,
        "hp_pct" => Stat::HpPercent,
        "atk_pct" => Stat::AtkPercent,
        "def_pct" => Stat::DefPercent,
        "spd" => Stat::Speed,
        "cr" => Stat::CritRate,
        "cd" => Stat::CritDamage,
        "ehr" => Stat::EffectHit,
        "res" => Stat::EffectRes,
        "be" => Stat::BreakEffect,
        _ => return Err(Error("未知副属性缩写，请输入 help".into()).into()),
    })
}

fn command(engine: &mut Engine, words: &[&str]) -> CliResult<()> {
    match words {
        ["target", target] => set_target(engine, target)?,
        ["rank"] => show_ranking(engine)?,
        ["next"] => {
            show_recommendation(engine.recommend_next()?.as_ref());
            show_selected(engine);
        }
        ["choose", id] => {
            match engine.select_relic(id)? {
                RelicSelection::Selected { relic_id } => {
                    println!("已选择遗器 {relic_id}。")
                }
                RelicSelection::ResumedFromHold { relic_id } => {
                    println!("遗器 {relic_id} 此前处于 Hold；已按显式 choose 恢复。")
                }
            }
            show_selected(engine);
        }
        ["upgrade", stat, delta] => {
            let stat = parse_stat(stat)?;
            let delta = delta.parse().map_err(|_| Error("增量必须是正数".into()))?;
            observe(engine, stat, delta)?;
        }
        ["show"] => {
            show_selected(engine);
            println!("剩余预算：{} 步", engine.account().upgrade_steps);
        }
        ["history"] => show_history(engine),
        ["help"] => help(),
        [] => (),
        _ => return Err(Error("命令或参数数量不正确，请输入 help".into()).into()),
    }
    Ok(())
}

fn interact(engine: &mut Engine) -> CliResult<()> {
    help();
    println!("请输入 target Blade 或 target Seele 开始。");
    let mut line = String::new();
    loop {
        print!("> ");
        io::stdout().flush().map_err(|e| Error(e.to_string()))?;
        line.clear();
        if io::stdin()
            .read_line(&mut line)
            .map_err(|e| Error(e.to_string()))?
            == 0
        {
            break;
        }
        let words: Vec<_> = line.split_whitespace().collect();
        if words == ["quit"] {
            break;
        }
        if let Err(error) = command(engine, &words) {
            println!("未执行：{error}");
        }
    }
    println!("已退出，原始 fixture 未改变；本次内存状态不保存。");
    Ok(())
}
