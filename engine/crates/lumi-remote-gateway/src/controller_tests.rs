//! Regression tests through production authentication and Mac administration.
use super::*;
use crate::PairingInvitationRequest;
use crate::admin::{GatewayAdminRequest, GatewayAdminResponse, apply_request};
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

type TestTls = tokio_rustls::client::TlsStream<TcpStream>;
type TestReader = BoundedLineReader<BufReader<tokio::io::ReadHalf<TestTls>>>;

async fn tls_client(
    fixture: &Fixture,
    id: &str,
) -> Result<
    (
        RemoteServerHello,
        TestReader,
        tokio::io::WriteHalf<TestTls>,
        tokio::task::JoinHandle<Result<(), GatewayNetworkError>>,
    ),
    Box<dyn Error>,
> {
    use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};
    use tokio_rustls::rustls::{ClientConfig, RootCertStore};
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let acceptor = TlsAcceptor::from(fixture.state.identity.tls_server_config()?);
    let state = fixture.state.clone();
    let relay = fixture.relay.clone();
    let server = tokio::spawn(async move {
        let (stream, peer) = listener.accept().await?;
        serve_tls_client(
            stream,
            peer,
            acceptor,
            state,
            relay,
            Arc::new(Mutex::new(AttemptRateLimiter::new(8, 60_000)?)),
        )
        .await
    });
    let mut roots = RootCertStore::empty();
    roots.add(CertificateDer::from(
        fixture.state.identity.certificate_der.clone(),
    ))?;
    let config = ClientConfig::builder_with_provider(Arc::new(
        tokio_rustls::rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_root_certificates(roots)
    .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let stream = connector
        .connect(
            ServerName::try_from(fixture.state.identity.server_name.clone())?,
            TcpStream::connect(address).await?,
        )
        .await?;
    let (reader, mut writer) = tokio::io::split(stream);
    write_frame(
        &mut writer,
        &RemoteFrame {
            protocol_version: REMOTE_PROTOCOL_VERSION,
            frame_kind: RemoteFrameKind::Hello,
            sequence: 1,
            correlation_id: None,
            payload: serde_json::to_value(hello(id))?,
        },
    )
    .await?;
    let mut reader = BoundedLineReader::new(BufReader::new(reader));
    let frame = read_frame(&mut reader)
        .await?
        .ok_or_else(|| io::Error::other("missing authentication"))?;
    let response = serde_json::from_value(frame.payload)?;
    Ok((response, reader, writer, server))
}

struct Fixture {
    directory: PathBuf,
    state: SharedGatewayState,
    relay: EngineRelayHandle,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let directory = std::env::temp_dir().join(format!("lumi-controller-{}", random_hex(12)?));
        std::fs::create_dir_all(&directory)?;
        let identity = InstallationIdentity::load_or_create(&directory.join("identity"))?;
        let state = SharedGatewayState::load(
            identity,
            PersistentTrustStore::new(directory.join("trust.json")),
        )?;
        let relay = EngineRelayHandle::start(
            directory.join("unused-engine.json"),
            state.command_guard.clone(),
        );
        Ok(Self {
            directory,
            state,
            relay,
        })
    }

    async fn pair(&self, id: &str) -> Result<RemoteServerHello, Box<dyn Error>> {
        let invitation = format!("invitation-{id}");
        let secret = "s".repeat(32);
        {
            let mut registry = self.state.registry.lock().await;
            registry.create_invitation(PairingInvitationRequest {
                invitation_id: invitation.clone(),
                invitation_secret: secret.clone(),
                short_code: "123456".into(),
                certificate_fingerprint_sha256: "a".repeat(64),
                created_at_unix_millis: 1,
                expires_at_unix_millis: 1000,
            })?;
            registry.approve(&invitation, "123456")?;
        }
        Ok(self
            .state
            .authenticate(
                RemoteClientHello::Pair {
                    invitation_id: invitation,
                    invitation_secret: secret,
                    device_id: id.into(),
                    display_name: id.into(),
                    device_credential: "c".repeat(32),
                    client_version: Some("0.1.1-dev-3".into()),
                },
                10,
            )
            .await?
            .1)
    }

    async fn admin(&self, action: GatewayAdminRequest) -> GatewayAdminResponse {
        apply_request(action, &self.state, &self.relay, 12345).await
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[tokio::test]
async fn two_real_tls_clients_reauthenticate_after_explicit_transfer_without_role_drift()
-> TestResult {
    let f = Fixture::new()?;
    f.pair("simulator").await?;
    f.pair("iphone").await?;
    let (a, _ar, _aw, at) = tls_client(&f, "simulator").await?;
    let (b, _br, _bw, bt) = tls_client(&f, "iphone").await?;
    assert!(lease(&a).is_some());
    assert!(lease(&b).is_none());
    assert!(
        f.admin(GatewayAdminRequest::TransferControl {
            device_id: "iphone".into()
        })
        .await
        .ok
    );
    // Both old sessions must close, even when transfer arrives while the
    // initial Hello/unavailable-state messages are still being written.
    timeout(Duration::from_secs(2), at).await???;
    timeout(Duration::from_secs(2), bt).await???;
    let (a, _ar2, _aw2, at2) = tls_client(&f, "simulator").await?;
    let (b, _br2, _bw2, bt2) = tls_client(&f, "iphone").await?;
    assert!(lease(&a).is_none());
    assert!(lease(&b).is_some());
    at2.abort();
    bt2.abort();
    let _ = at2.await;
    let _ = bt2.await;
    Ok(())
}

fn hello(id: &str) -> RemoteClientHello {
    RemoteClientHello::Authenticate {
        device_id: id.into(),
        credential: "c".repeat(32),
        client_version: None,
    }
}

fn lease(hello: &RemoteServerHello) -> Option<&str> {
    match hello {
        RemoteServerHello::Authenticated {
            controller_lease_id,
            ..
        }
        | RemoteServerHello::Paired {
            controller_lease_id,
            ..
        } => controller_lease_id.as_deref(),
    }
}

#[tokio::test]
async fn offline_owner_survives_reconnect_order_and_gateway_restart() -> TestResult {
    let f = Fixture::new()?;
    assert!(lease(&f.pair("simulator").await?).is_some());
    assert!(lease(&f.pair("iphone").await?).is_none());
    for _ in 0..5 {
        let state =
            SharedGatewayState::load(f.state.identity.clone(), f.state.trust_store.clone())?;
        for order in [["iphone", "simulator"], ["simulator", "iphone"]] {
            for id in order {
                let response = state.authenticate(hello(id), 20).await?.1;
                assert_eq!(lease(&response).is_some(), id == "simulator");
                match response {
                    RemoteServerHello::Authenticated {
                        controller_display_name,
                        ..
                    } => {
                        assert_eq!(controller_display_name.as_deref(), Some("simulator"));
                    }
                    _ => panic!("expected authenticated response"),
                }
            }
        }
        assert_eq!(state.registry.lock().await.controller_changes().len(), 1);
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_clients_never_take_over_after_controller_revocation() -> TestResult {
    let f = Fixture::new()?;
    f.pair("owner").await?;
    f.pair("simulator").await?;
    f.pair("iphone").await?;
    assert!(
        f.admin(GatewayAdminRequest::RevokeDevice {
            device_id: "owner".into()
        })
        .await
        .ok
    );
    for _ in 0..50 {
        let (a, b) = tokio::join!(
            f.state.authenticate(hello("simulator"), 30),
            f.state.authenticate(hello("iphone"), 30)
        );
        assert!(lease(&a?.1).is_none());
        assert!(lease(&b?.1).is_none());
    }
    let reloaded = SharedGatewayState::load(f.state.identity.clone(), f.state.trust_store.clone())?;
    assert!(reloaded.command_guard.lock().await.controller().is_none());
    assert!(
        reloaded
            .registry
            .lock()
            .await
            .controller_device_id()
            .is_none()
    );
    assert_eq!(reloaded.registry.lock().await.controller_changes().len(), 2);
    Ok(())
}

#[tokio::test]
async fn explicit_mac_transfer_is_durable_and_old_lease_is_rejected() -> TestResult {
    let f = Fixture::new()?;
    let old = f.pair("simulator").await?;
    f.pair("iphone").await?;
    let response = f
        .admin(GatewayAdminRequest::TransferControl {
            device_id: "iphone".into(),
        })
        .await;
    assert!(response.ok);
    assert_eq!(
        response.status.controller_device_id.as_deref(),
        Some("iphone")
    );
    assert_eq!(
        response
            .status
            .controller_changes
            .last()
            .ok_or_else(|| io::Error::other("missing transfer history"))?
            .reason,
        "macTransfer"
    );
    assert!(lease(&f.state.authenticate(hello("simulator"), 30).await?.1).is_none());
    let controller = f.state.authenticate(hello("iphone"), 30).await?.1;
    assert!(lease(&controller).is_some());
    assert_ne!(lease(&controller), lease(&old));
    let invalid_command = RemoteCommand {
        command_id: "old-owner-command".into(),
        controller_lease_id: lease(&old)
            .ok_or_else(|| io::Error::other("missing original lease"))?
            .into(),
        issued_at_unix_millis: 30,
        command: lumi_remote_protocol::RemoteCommandKind::SetOperationState {
            operation_state: lumi_remote_protocol::OperationTarget::Armed,
            expected_state_revision: 1,
        },
    };
    assert!(
        f.state
            .command_guard
            .lock()
            .await
            .admit("simulator", &invalid_command)
            .is_err()
    );
    let reloaded = SharedGatewayState::load(f.state.identity.clone(), f.state.trust_store.clone())?;
    assert_eq!(
        reloaded.registry.lock().await.controller_device_id(),
        Some("iphone")
    );
    assert_eq!(
        reloaded
            .command_guard
            .lock()
            .await
            .controller()
            .ok_or_else(|| io::Error::other("missing restored Controller"))?
            .device_id,
        "iphone"
    );
    Ok(())
}

#[tokio::test]
async fn failed_transfer_and_revoke_preserve_registry_lease_and_history() -> TestResult {
    let f = Fixture::new()?;
    f.pair("simulator").await?;
    f.pair("iphone").await?;
    let before = f.state.registry.lock().await.snapshot();
    let before_lease = f
        .state
        .command_guard
        .lock()
        .await
        .controller_lease_for("simulator");
    // Inject a real filesystem failure before the atomic rename, in a disposable fixture.
    std::fs::rename(
        f.directory.join("trust.json"),
        f.directory.join("original.json"),
    )?;
    std::fs::create_dir(f.directory.join("trust.json"))?;
    for action in [
        GatewayAdminRequest::TransferControl {
            device_id: "iphone".into(),
        },
        GatewayAdminRequest::RevokeDevice {
            device_id: "simulator".into(),
        },
    ] {
        assert!(!f.admin(action).await.ok);
        assert_eq!(f.state.registry.lock().await.snapshot(), before);
        assert_eq!(
            f.state
                .command_guard
                .lock()
                .await
                .controller_lease_for("simulator"),
            before_lease
        );
    }
    assert!(f.state.authenticate(hello("iphone"), 100).await.is_err());
    assert_eq!(f.state.registry.lock().await.snapshot(), before);
    std::fs::remove_dir(f.directory.join("trust.json"))?;
    std::fs::rename(
        f.directory.join("original.json"),
        f.directory.join("trust.json"),
    )?;
    assert!(
        f.admin(GatewayAdminRequest::TransferControl {
            device_id: "iphone".into()
        })
        .await
        .ok
    );
    Ok(())
}

#[tokio::test]
async fn re_pairing_and_legacy_metadata_do_not_clear_the_owner_or_known_version() -> TestResult {
    let f = Fixture::new()?;
    f.pair("owner").await?;
    f.pair("observer").await?;
    assert!(lease(&f.pair("owner").await?).is_some());
    assert!(lease(&f.pair("observer").await?).is_none());
    f.state.authenticate(hello("owner"), 30).await?;
    let registry = f.state.registry.lock().await;
    assert_eq!(registry.controller_device_id(), Some("owner"));
    assert_eq!(
        registry
            .paired_devices()
            .find(|d| d.device_id == "owner")
            .ok_or_else(|| io::Error::other("missing paired owner"))?
            .client_version
            .as_deref(),
        Some("0.1.1-dev-3")
    );
    assert_eq!(registry.controller_changes().len(), 1);
    Ok(())
}

#[tokio::test]
async fn legacy_trust_and_empty_registry_after_revoke_do_not_reenable_auto_selection() -> TestResult
{
    let f = Fixture::new()?;
    f.pair("owner").await?;
    // Old snapshots have neither the initialization flag nor the history/version fields.
    let mut json = serde_json::to_value(f.state.registry.lock().await.snapshot())?;
    json.as_object_mut()
        .ok_or_else(|| io::Error::other("snapshot is not an object"))?
        .remove("controllerSelectionInitialized");
    json.as_object_mut()
        .ok_or_else(|| io::Error::other("snapshot is not an object"))?
        .remove("controllerChanges");
    json["devices"][0]
        .as_object_mut()
        .ok_or_else(|| io::Error::other("missing first paired device"))?
        .remove("clientVersion");
    let mut registry = PairingRegistry::from_snapshot(serde_json::from_value(json)?)?;
    assert_eq!(registry.controller_device_id(), Some("owner"));
    assert!(registry.revoke("owner"));
    f.state
        .commit_registry(&mut *f.state.registry.lock().await, registry, false)
        .await?;
    assert!(lease(&f.pair("new-device").await?).is_none());
    Ok(())
}
