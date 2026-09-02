use std::env;
use std::error::Error;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::process::ExitCode;

use lumi_remote_gateway::{
    DEFAULT_CRITICAL_QUEUE_CAPACITY, EngineRelayHandle, GatewayAdminServer, GatewayConfig,
    GatewayNetworkServer, InstallationIdentity, PersistentTrustStore, ReleaseChannel,
    SharedGatewayState,
};

const ENGINE_SERVICE_RECORD: &str = "engine-service.json";
const GATEWAY_SERVICE_RECORD: &str = "remote-gateway-service.json";

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Lumi Remote Gateway stopped: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let release_channel = release_channel()?;
    let config = GatewayConfig {
        release_channel,
        engine_host: IpAddr::V4(Ipv4Addr::LOCALHOST),
        maximum_clients: 4,
        critical_queue_capacity: DEFAULT_CRITICAL_QUEUE_CAPACITY,
    };
    config.validate()?;

    let data_directory = channel_data_directory()?;
    let remote_directory = data_directory.join("Remote Gateway");
    let identity = InstallationIdentity::load_or_create(&remote_directory.join("Identity"))?;
    let trust_store = PersistentTrustStore::new(remote_directory.join("trust.json"));
    let state = SharedGatewayState::load(identity, trust_store)?;
    let relay = EngineRelayHandle::start(data_directory.join(ENGINE_SERVICE_RECORD));
    let display_name = display_name();
    let network = GatewayNetworkServer::bind(
        state.clone(),
        relay.clone(),
        release_channel,
        &display_name,
        config.maximum_clients,
    )
    .await?;
    let lan_port = network.local_addr()?.port();
    let product_version = required_environment("LUMI_PRODUCT_VERSION")?;
    let (admin, _admin_record) = GatewayAdminServer::bind(
        state,
        relay,
        &data_directory.join(GATEWAY_SERVICE_RECORD),
        product_version,
        lan_port,
    )
    .await?;

    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        result = network.run() => result?,
        result = admin.run() => result?,
        _ = terminate.recv() => {},
        result = tokio::signal::ctrl_c() => result?,
    }
    Ok(())
}

fn channel_data_directory() -> Result<PathBuf, Box<dyn Error>> {
    let home = PathBuf::from(required_environment("HOME")?);
    if !home.is_absolute() {
        return Err("HOME must be absolute".into());
    }
    let directory_name = required_environment("LUMI_DATA_DIRECTORY_NAME")?;
    if directory_name == "."
        || directory_name == ".."
        || directory_name.contains('/')
        || directory_name.contains('\0')
    {
        return Err("LUMI_DATA_DIRECTORY_NAME is invalid".into());
    }
    Ok(home
        .join("Library")
        .join("Application Support")
        .join(directory_name))
}

fn release_channel() -> Result<ReleaseChannel, Box<dyn Error>> {
    match required_environment("LUMI_RELEASE_CHANNEL")?.as_str() {
        "dev" => Ok(ReleaseChannel::Dev),
        "rc" => Ok(ReleaseChannel::Rc),
        "production" | "release" => Ok(ReleaseChannel::Production),
        _ => Err("LUMI_RELEASE_CHANNEL is invalid".into()),
    }
}

fn display_name() -> String {
    env::var("LUMI_REMOTE_DISPLAY_NAME")
        .ok()
        .or_else(|| env::var("HOSTNAME").ok())
        .filter(|value| {
            !value.is_empty() && value.len() <= 63 && !value.chars().any(char::is_control)
        })
        .map(|value| format!("Lumi on {value}"))
        .unwrap_or_else(|| "Lumi Remote".to_owned())
}

fn required_environment(key: &'static str) -> Result<String, Box<dyn Error>> {
    env::var(key)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{key} is required").into())
}
