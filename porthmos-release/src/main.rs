use std::process::ExitCode;

use porthmos_release::command::run;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args, &|name| std::env::var(name).ok()) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("porthmos-release: {err:#}");
            ExitCode::FAILURE
        }
    }
}
