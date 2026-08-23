//! Lumi engine executable entry point.

#![forbid(unsafe_code)]

use std::path::Path;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = std::env::args().collect::<Vec<_>>();
    if arguments.get(1).map(String::as_str) == Some("--usb-worker") {
        let Some(request_path) = arguments.get(2) else {
            eprintln!("lumi-engine USB worker requires a request path");
            return ExitCode::FAILURE;
        };
        let Some(response_path) = arguments.get(3) else {
            eprintln!("lumi-engine USB worker requires a response path");
            return ExitCode::FAILURE;
        };
        return match lumi_engine::run_usb_worker(Path::new(request_path), Path::new(response_path))
        {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("lumi-engine USB worker failed: {error}");
                ExitCode::FAILURE
            }
        };
    }
    match lumi_engine::run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("lumi-engine failed: {error}");
            ExitCode::FAILURE
        }
    }
}
