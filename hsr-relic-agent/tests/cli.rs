use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn default_demo_completes_and_shows_all_decisions() {
    let output = Command::new(env!("CARGO_BIN_EXE_hsr-relic-agent"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "判断 Continue",
        "判断 Hold",
        "判断 Stop",
        "目标：Blade",
        "目标：Seele",
        "同一遗器 9100001：+0 → +3",
        "同一遗器 9100001：+3 → +6",
        "演示结束",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
}

#[test]
fn interactive_loop_accepts_results_and_recovers_from_bad_input() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hsr-relic-agent"))
        .arg("--interactive")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"target Blade\nchoose missing\nchoose 9100005\nupgrade cr -1\nupgrade cr 3.24\nchoose 9100001\nupgrade def_pct 5.4\nchoose 9100001\nupgrade def_pct 5.4\nchoose 9100001\ntarget Seele\nupgrade cr 3.24\nhistory\nquit\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "判断 Continue",
        "判断 Hold",
        "判断 Stop",
        "未执行：找不到遗器 missing，请检查 ID。",
        "未执行：遗器 9100005 已锁定，解除锁定后才能选择或强化。",
        "未执行：强化增量必须是大于 0 的有限数值。",
        "遗器 9100001 此前处于 Hold；已按显式 choose 恢复。",
        "未执行：遗器 9100001 对当前目标 1205 已判定为 Stop，请选择其他候选。",
        "强化历史（4 条，剩余 4 步）",
        "已退出",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
}
