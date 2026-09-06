use hsr_agent_runtime::ModelConfig;
use hsr_relic_web::{App, router};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|a| !["--mock", "--lan"].contains(&a.as_str()))
    {
        return Err("支持 --mock（对照模式）、--lan（同一局域网试用）".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    if !root.join("web/.generated/assets.json").is_file() {
        return Err("请先在 workspace 根目录执行 node web/prepare.mjs".into());
    }
    let mock = args.iter().any(|a| a == "--mock");
    if !mock && !root.join("adapters/fribbels/dist/adapter.mjs").is_file() {
        return Err("缺少 Fribbels Adapter；请执行 node web/prepare.mjs".into());
    }
    let host = if args.iter().any(|a| a == "--lan") {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };
    let port: u16 = std::env::var("HSR_WEB_PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()?;
    let listener = tokio::net::TcpListener::bind((host, port)).await?;
    println!(
        "遗器培养终端：http://127.0.0.1:{port}  模式：{}",
        if mock { "Mock 对照" } else { "Fribbels" }
    );
    if host == "0.0.0.0" {
        println!("已开启局域网访问；同学可使用本机局域网 IP 与端口 {port}，每个标签页独立试用。");
    }
    let model_config =
        ModelConfig::from_env().map_err(|error| format!("模型环境配置无效：{error}"))?;
    axum::serve(
        listener,
        router(App::with_model_config(mock, model_config), root),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}
