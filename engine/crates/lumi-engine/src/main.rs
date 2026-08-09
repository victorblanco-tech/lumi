//! Lumi engine executable entry point.

#![forbid(unsafe_code)]

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match lumi_engine::run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("lumi-engine failed: {error}");
            ExitCode::FAILURE
        }
    }
}
