use std::net::{IpAddr, Ipv4Addr};
use std::process::ExitCode;

use lumi_remote_gateway::{DEFAULT_CRITICAL_QUEUE_CAPACITY, GatewayConfig, ReleaseChannel};

fn main() -> ExitCode {
    let config = GatewayConfig {
        release_channel: ReleaseChannel::Dev,
        engine_host: IpAddr::V4(Ipv4Addr::LOCALHOST),
        maximum_clients: 1,
        critical_queue_capacity: DEFAULT_CRITICAL_QUEUE_CAPACITY,
    };
    if let Err(error) = config.validate() {
        eprintln!("Lumi Remote Gateway configuration rejected: {error}");
        return ExitCode::FAILURE;
    }

    // LAN binding stays fail-closed until E9-02 supplies certificate pinning,
    // physical pairing and a persistent trust store. This binary is packaged
    // only after that boundary is complete.
    println!(
        "Lumi Remote Gateway foundation ready ({})",
        config.release_channel.bonjour_service_type()
    );
    ExitCode::SUCCESS
}
