use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use thiserror::Error;

use crate::{BridgeDecodeError, BridgeDecoder, BridgeMessage};

const STDERR_TAIL_CAPACITY: usize = 40;
pub const PRO_DJ_LINK_UDP_PORTS: [u16; 3] = [50_000, 50_001, 50_002];

/// Refuse to start a second Pro DJ Link application on the same network host.
///
/// Rekordbox Export Mode and Beat Link based applications need the same fixed
/// UDP ports. Starting both on one interface can interrupt a track being read
/// from rekordbox by a player, so this check deliberately fails closed before
/// the Java bridge is launched or sends any network traffic.
pub fn ensure_prolink_network_available() -> Result<(), BridgeSupervisorError> {
    ensure_udp_ports_available(&PRO_DJ_LINK_UDP_PORTS)
}

fn ensure_udp_ports_available(ports: &[u16]) -> Result<(), BridgeSupervisorError> {
    ensure_udp_ports_available_with(ports, |port| {
        UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port))
    })
}

fn ensure_udp_ports_available_with(
    ports: &[u16],
    bind: impl FnMut(u16) -> std::io::Result<UdpSocket>,
) -> Result<(), BridgeSupervisorError> {
    let (occupied_ports, owners) = occupied_udp_ports(ports, bind);
    if !occupied_ports.is_empty() {
        return Err(BridgeSupervisorError::NetworkConflict(
            ProLinkNetworkConflict {
                ports: occupied_ports,
                owners,
            },
        ));
    }
    Ok(())
}

fn occupied_udp_ports(
    ports: &[u16],
    mut bind: impl FnMut(u16) -> std::io::Result<UdpSocket>,
) -> (Vec<u16>, Vec<String>) {
    let mut occupied_ports = Vec::new();
    let mut owners = Vec::new();

    #[cfg(target_os = "macos")]
    for &port in ports {
        if let Some(owner) = macos_udp_port_owner(port) {
            occupied_ports.push(port);
            if !owners.contains(&owner) {
                owners.push(owner);
            }
        }
    }

    // Keep successful reservations alive until every port has been checked so
    // another local process cannot claim one halfway through this preflight.
    let mut reservations = Vec::new();
    for &port in ports {
        if occupied_ports.contains(&port) {
            continue;
        }
        match bind(port) {
            Ok(socket) => reservations.push(socket),
            Err(_) => occupied_ports.push(port),
        }
    }
    occupied_ports.sort_unstable();
    occupied_ports.dedup();
    (occupied_ports, owners)
}

#[cfg(target_os = "macos")]
fn macos_udp_port_owner(port: u16) -> Option<String> {
    let output = Command::new("/usr/sbin/lsof")
        .args(["-nP", &format!("-iUDP:{port}")])
        .output()
        .ok()?;
    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout
        .lines()
        .skip(1)
        .find_map(|line| line.split_whitespace().next().map(str::to_owned))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProLinkNetworkConflict {
    pub ports: Vec<u16>,
    pub owners: Vec<String>,
}

impl std::fmt::Display for ProLinkNetworkConflict {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Pro DJ Link cannot start because UDP ports {:?} are already in use",
            self.ports
        )?;
        if !self.owners.is_empty() {
            write!(formatter, " by {}", self.owners.join(", "))?;
        }
        write!(
            formatter,
            ". Close rekordbox or other Pro DJ Link software on this Mac, or run it on a different computer."
        )
    }
}

impl std::error::Error for ProLinkNetworkConflict {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeLaunchConfiguration {
    executable: PathBuf,
    arguments: Vec<String>,
}

impl BridgeLaunchConfiguration {
    #[must_use]
    pub fn java_jar(java_executable: impl Into<PathBuf>, bridge_jar: impl AsRef<Path>) -> Self {
        Self {
            executable: java_executable.into(),
            arguments: vec![
                "-Djava.awt.headless=true".to_owned(),
                "-jar".to_owned(),
                bridge_jar.as_ref().to_string_lossy().into_owned(),
            ],
        }
    }

    #[must_use]
    pub fn command(executable: impl Into<PathBuf>, arguments: Vec<String>) -> Self {
        Self {
            executable: executable.into(),
            arguments,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeProcessDiagnostics {
    pub running: bool,
    pub last_sequence: Option<u64>,
    pub stderr_tail: Vec<String>,
}

enum ProcessOutput {
    Line(String),
    StdoutClosed,
    ReadFailed(String),
}

pub struct BridgeProcessSupervisor {
    child: Child,
    stdin: Option<ChildStdin>,
    output: mpsc::Receiver<ProcessOutput>,
    decoder: BridgeDecoder,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    stdout_closed: bool,
}

impl BridgeProcessSupervisor {
    pub fn spawn(configuration: &BridgeLaunchConfiguration) -> Result<Self, BridgeSupervisorError> {
        let mut child = Command::new(&configuration.executable)
            .args(&configuration.arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(BridgeSupervisorError::Launch)?;
        let stdin = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or(BridgeSupervisorError::MissingPipe("stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(BridgeSupervisorError::MissingPipe("stderr"))?;

        let (sender, output) = mpsc::channel();
        thread::Builder::new()
            .name("lumi-prolink-stdout".to_owned())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    match line {
                        Ok(line) => {
                            if sender.send(ProcessOutput::Line(line)).is_err() {
                                return;
                            }
                        }
                        Err(error) => {
                            let _ = sender.send(ProcessOutput::ReadFailed(error.to_string()));
                            return;
                        }
                    }
                }
                let _ = sender.send(ProcessOutput::StdoutClosed);
            })
            .map_err(BridgeSupervisorError::ReaderThread)?;

        let stderr_tail = Arc::new(Mutex::new(VecDeque::with_capacity(STDERR_TAIL_CAPACITY)));
        let stderr_lines = Arc::clone(&stderr_tail);
        thread::Builder::new()
            .name("lumi-prolink-stderr".to_owned())
            .spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let Ok(mut lines) = stderr_lines.lock() else {
                        return;
                    };
                    if lines.len() == STDERR_TAIL_CAPACITY {
                        lines.pop_front();
                    }
                    lines.push_back(line);
                }
            })
            .map_err(BridgeSupervisorError::ReaderThread)?;

        Ok(Self {
            child,
            stdin,
            output,
            decoder: BridgeDecoder::new(),
            stderr_tail,
            stdout_closed: false,
        })
    }

    pub fn drain_messages(&mut self) -> Result<Vec<BridgeMessage>, BridgeSupervisorError> {
        let mut messages = Vec::new();
        loop {
            match self.output.try_recv() {
                Ok(ProcessOutput::Line(line)) => {
                    messages.push(self.decoder.decode_line(&line)?);
                }
                Ok(ProcessOutput::StdoutClosed) => {
                    self.stdout_closed = true;
                }
                Ok(ProcessOutput::ReadFailed(message)) => {
                    return Err(BridgeSupervisorError::Read(message));
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.stdout_closed = true;
                    break;
                }
            }
        }
        Ok(messages)
    }

    pub fn diagnostics(&mut self) -> Result<BridgeProcessDiagnostics, BridgeSupervisorError> {
        let running = self
            .child
            .try_wait()
            .map_err(BridgeSupervisorError::Status)?
            .is_none()
            && !self.stdout_closed;
        let stderr_tail = self
            .stderr_tail
            .lock()
            .map_err(|_| BridgeSupervisorError::StderrLock)?
            .iter()
            .cloned()
            .collect();
        Ok(BridgeProcessDiagnostics {
            running,
            last_sequence: self.decoder.last_sequence(),
            stderr_tail,
        })
    }

    pub fn stop(&mut self) -> Result<(), BridgeSupervisorError> {
        self.stdin.take();
        if self
            .child
            .try_wait()
            .map_err(BridgeSupervisorError::Status)?
            .is_none()
        {
            self.child.kill().map_err(BridgeSupervisorError::Stop)?;
        }
        self.child.wait().map_err(BridgeSupervisorError::Stop)?;
        Ok(())
    }
}

impl Drop for BridgeProcessSupervisor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[derive(Debug, Error)]
pub enum BridgeSupervisorError {
    #[error("{0}")]
    NetworkConflict(#[from] ProLinkNetworkConflict),
    #[error("failed to launch Pro DJ Link bridge: {0}")]
    Launch(std::io::Error),
    #[error("the Pro DJ Link bridge did not expose its {0} pipe")]
    MissingPipe(&'static str),
    #[error("failed to start Pro DJ Link bridge reader: {0}")]
    ReaderThread(std::io::Error),
    #[error("failed to read Pro DJ Link bridge output: {0}")]
    Read(String),
    #[error("invalid Pro DJ Link bridge message: {0}")]
    Decode(#[from] BridgeDecodeError),
    #[error("failed to inspect Pro DJ Link bridge process: {0}")]
    Status(std::io::Error),
    #[error("failed to stop Pro DJ Link bridge process: {0}")]
    Stop(std::io::Error),
    #[error("the Pro DJ Link bridge diagnostics lock is poisoned")]
    StderrLock,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occupied_udp_port_fails_the_same_preflight_used_by_pro_dj_link() {
        let port = 65_535;
        let result = ensure_udp_ports_available_with(&[port], |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                "test reservation",
            ))
        });
        let Err(error) = result else {
            panic!("an occupied UDP port must fail closed");
        };
        assert!(matches!(error, BridgeSupervisorError::NetworkConflict(_)));
        assert!(error.to_string().contains(&port.to_string()));
    }
}
