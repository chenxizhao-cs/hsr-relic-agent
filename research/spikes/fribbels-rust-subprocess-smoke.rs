use std::env;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

fn main() {
    let args: Vec<String> = env::args().collect();
    let input = fs::read(&args[2]).unwrap();
    let mut child = Command::new("node")
        .arg(&args[1])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    child.stdin.take().unwrap().write_all(&input).unwrap();
    let output = child.wait_with_output().unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(text.starts_with('{'));
    assert!(text.contains("\"scores\""));
    assert!(text.contains("\"panel\""));
    assert!(text.contains("\"actionDamage\""));
    println!("status=success bytes={} json_object=true expected_keys=true", text.len());
}
