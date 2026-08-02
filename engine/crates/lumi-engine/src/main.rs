//! Lumi's local engine process.

#![forbid(unsafe_code)]

use lumi_domain::OperationState;

fn main() {
    let initial_state = OperationState::default();
    println!(
        "lumi-engine {} ready in {initial_state:?}",
        env!("CARGO_PKG_VERSION")
    );
}
