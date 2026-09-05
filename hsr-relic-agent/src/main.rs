mod cli;

fn main() -> std::process::ExitCode {
    match cli::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("错误：{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
