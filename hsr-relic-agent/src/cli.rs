use std::io::{self, Write};

use hsr_relic_agent::{
    DEMO_ACCOUNT, DecisionEngine, Error, MockEvaluator, Result, Stat, UpgradeRecommendation,
    UpgradeResult, load_scanner_v4,
};

type Engine = DecisionEngine<MockEvaluator>;

pub fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "cargo run：自动演示；cargo run -- --interactive：交互操作。\n每次启动从同一 fixture 加载，内存状态不写回文件。"
        );
        return Ok(());
    }
    let interactive = match args.as_slice() {
        [] => false,
        [arg] if arg == "--interactive" => true,
        _ => return Err(Error("仅支持 --interactive / --help".into())),
    };
    let mut engine = Engine::new(load_scanner_v4(DEMO_ACCOUNT, 8)?, MockEvaluator);
    println!("HSR 遗器强化 Demo v0.1 — Mock 评分，不代表真实战斗收益");
    println!(
        "已加载 fixtures/scanner-v4-demo.json：{} 个角色 / {} 件遗器，预算 {} 步（一次 +3 消耗一步）。",
        engine.account().characters.len(),
        engine.account().relics.len(),
        engine.account().upgrade_steps
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

fn set_target(engine: &mut Engine, target: &str) -> Result<()> {
    let id = match target.to_ascii_lowercase().as_str() {
        "blade" | "1205" => "1205",
        "seele" | "1102" => "1102",
        _ => return Err(Error("目标需为 Blade / Seele / 1205 / 1102".into())),
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

fn show_ranking(engine: &Engine) -> Result<()> {
    let ranked = engine.rank_candidates()?;
    println!("候选排序（Mock 单位步数收益）：");
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

fn observe(engine: &mut Engine, stat: Stat, increase: f64) -> Result<()> {
    let selected = engine
        .selected()
        .ok_or_else(|| Error("没有选中遗器，请先 target / next / choose".into()))?;
    let id = selected.id.clone();
    let old_level = selected.level;
    let out = engine.apply_upgrade(UpgradeResult {
        relic_id: id.clone(),
        expected_level: old_level,
        stat,
        increase,
    })?;
    let updated = &engine.account().relics[&id];
    println!(
        "\n已更新同一遗器 {id}：+{old_level} → +{}，{stat:?} 增加 {increase}，现值 {}；剩余 {} 步。",
        updated.level,
        updated.substats[&stat],
        engine.account().upgrade_steps
    );
    println!("判断 {:?}：{}", out.decision, out.reason);
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

fn demo(engine: &mut Engine) -> Result<()> {
    println!("\n自动脚本：先比较目标，再录入固定的模拟观察；未使用 fixture 的 preview_substats。");
    set_target(engine, "Seele")?;
    set_target(engine, "Blade")?;
    println!("\n[1] 给推荐的遗器录入一次暴击率增加：");
    observe(engine, Stat::CritRate, 3.24)?;
    println!("\n[2] 手动观察三词条候选 9100001，录入新增防御百分比：");
    engine.select_relic("9100001")?;
    observe(engine, Stat::DefPercent, 5.4)?;
    println!("\n[3] 显式恢复刚才 Hold 的同一件 9100001，再录入防御百分比增加：");
    engine.select_relic("9100001")?;
    observe(engine, Stat::DefPercent, 5.4)?;
    println!("\n[4] Stop 后已有其他推荐；切换培养目标继续观察：");
    set_target(engine, "Seele")?;
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

fn parse_stat(value: &str) -> Result<Stat> {
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
        _ => return Err(Error("未知副属性缩写，请输入 help".into())),
    })
}

fn command(engine: &mut Engine, words: &[&str]) -> Result<()> {
    match words {
        ["target", target] => set_target(engine, target)?,
        ["rank"] => show_ranking(engine)?,
        ["next"] => {
            show_recommendation(engine.recommend_next()?.as_ref());
            show_selected(engine);
        }
        ["choose", id] => {
            engine.select_relic(id)?;
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
        _ => return Err(Error("命令或参数数量不正确，请输入 help".into())),
    }
    Ok(())
}

fn interact(engine: &mut Engine) -> Result<()> {
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
