//! Bounded desktop socket I/O. No task in this module owns show state or a DB.
//!
//! A peer waiting for authentication or not reading its responses must not
//! suspend the engine's integration pump. Dropping either owner cancels all
//! associated socket tasks; shutdown never leaves detached sessions behind.

use super::*;
use lumi_stream::BoundedLineReader;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::oneshot;
use tokio::task::{JoinHandle, JoinSet};

const AUTHENTICATING_CLIENT_LIMIT: usize = 4;
const DESKTOP_QUEUE_CAPACITY: usize = 8;
const DESKTOP_WRITE_TIMEOUT: Duration = Duration::from_secs(2);

pub(super) struct AuthenticatedConnection {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
}

pub(super) struct DesktopAcceptor {
    receiver: mpsc::Receiver<AuthenticatedConnection>,
    task: JoinHandle<()>,
}

impl DesktopAcceptor {
    pub fn start(listener: TcpListener, token: String) -> Self {
        let (sender, receiver) = mpsc::channel(1);
        let task = tokio::spawn(async move {
            let mut pending = JoinSet::new();
            loop {
                tokio::select! {
                    _ = sender.closed() => break,
                    accepted = listener.accept() => {
                        let Ok((stream, peer)) = accepted else { break };
                        if !peer.ip().is_loopback() || pending.len() >= AUTHENTICATING_CLIENT_LIMIT {
                            continue;
                        }
                        let expected = token.clone();
                        let authenticated = sender.clone();
                        pending.spawn(async move {
                            let (mut reader, writer) = stream.into_split();
                            let Ok(Ok(bytes)) = timeout(
                                AUTHENTICATION_TIMEOUT,
                                read_bounded_line(&mut reader, MAXIMUM_AUTHENTICATION_BYTES),
                            ).await else { return };
                            let Ok(authentication) = serde_json::from_slice::<SessionAuthentication>(&bytes)
                                else { return };
                            if tokens_match(&expected, &authentication.session_token) {
                                // One waiting UI is sufficient. Never accumulate authenticated peers.
                                let _ = authenticated.try_send(AuthenticatedConnection { reader, writer });
                            }
                        });
                    }
                    _ = pending.join_next(), if !pending.is_empty() => {}
                }
            }
        });
        Self { receiver, task }
    }

    pub async fn next(&mut self) -> Option<AuthenticatedConnection> {
        self.receiver.recv().await
    }
}

impl Drop for DesktopAcceptor {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(super) struct DesktopClientIo {
    commands: mpsc::Receiver<io::Result<Vec<u8>>>,
    responses: mpsc::Sender<MessageEnvelope>,
    writer_result: oneshot::Receiver<Result<(), EngineError>>,
    tasks: Vec<JoinHandle<()>>,
}

impl DesktopClientIo {
    pub fn start(connection: AuthenticatedConnection) -> Self {
        Self::with_streams(connection.reader, connection.writer)
    }

    fn with_streams<R, W>(reader: R, mut writer: W) -> Self
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let (input_sender, commands) = mpsc::channel(DESKTOP_QUEUE_CAPACITY);
        let (responses, mut output_receiver) = mpsc::channel(DESKTOP_QUEUE_CAPACITY);
        let (result_sender, writer_result) = oneshot::channel();
        let input_task = tokio::spawn(async move {
            let mut reader = BoundedLineReader::new(BufReader::new(reader));
            loop {
                let line = match reader.next_line(MAX_MESSAGE_BYTES).await {
                    Ok(Some(line)) => Ok(line),
                    Ok(None) => return,
                    Err(error) => Err(error),
                };
                let failed = line.is_err();
                if input_sender.send(line).await.is_err() || failed {
                    return;
                }
            }
        });
        let output_task = tokio::spawn(async move {
            let result = async {
                while let Some(envelope) = output_receiver.recv().await {
                    timeout(
                        DESKTOP_WRITE_TIMEOUT,
                        write_envelope(&mut writer, &envelope),
                    )
                    .await
                    .map_err(|_| {
                        EngineError::Io(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "desktop stopped reading responses",
                        ))
                    })??;
                }
                Ok(())
            }
            .await;
            let _ = result_sender.send(result);
        });
        Self {
            commands,
            responses,
            writer_result,
            tasks: vec![input_task, output_task],
        }
    }

    pub async fn next_command(&mut self) -> Result<Option<Vec<u8>>, EngineError> {
        tokio::select! {
            command = self.commands.recv() => command.transpose().map_err(EngineError::AuthenticatedClientIo),
            result = &mut self.writer_result => {
                match result {
                    Ok(Err(error)) => Err(authenticated_client_error(error)),
                    _ => Ok(None),
                }
            }
        }
    }

    pub fn respond(&self, response: MessageEnvelope) -> Result<(), EngineError> {
        self.responses.try_send(response).map_err(|_| {
            EngineError::AuthenticatedClientIo(io::Error::new(
                io::ErrorKind::WouldBlock,
                "desktop response queue is full or closed",
            ))
        })
    }
}

impl Drop for DesktopClientIo {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpStream;

    #[tokio::test]
    async fn incomplete_authentication_does_not_block_another_ui()
    -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let mut acceptor = DesktopAcceptor::start(listener, "secret".to_owned());
        let mut stalled = TcpStream::connect(address).await?;
        stalled.write_all(b"{\"sessionToken\":").await?;
        tokio::time::sleep(Duration::from_millis(20)).await;
        let mut valid = TcpStream::connect(address).await?;
        valid.write_all(b"{\"sessionToken\":\"secret\"}\n").await?;
        assert!(
            timeout(Duration::from_millis(250), acceptor.next())
                .await?
                .is_some()
        );
        Ok(())
    }

    #[tokio::test]
    async fn blocked_writer_does_not_block_command_input_or_the_calling_task()
    -> Result<(), Box<dyn std::error::Error>> {
        let (mut input, reader) = tokio::io::duplex(64);
        let (writer, _unread_output) = tokio::io::duplex(1);
        let mut client = DesktopClientIo::with_streams(reader, writer);
        client.respond(error_envelope(1, "a", "test", "test", "test", false, None)?)?;
        input.write_all(b"first\nsecond\n").await?;
        assert_eq!(
            timeout(Duration::from_millis(100), client.next_command()).await??,
            Some(b"first".to_vec())
        );
        assert_eq!(
            timeout(Duration::from_millis(100), client.next_command()).await??,
            Some(b"second".to_vec())
        );
        // Queue admission is bounded and synchronous even if the writer cannot advance.
        let mut rejected = false;
        for sequence in 2..20 {
            if client
                .respond(error_envelope(
                    sequence, "a", "test", "test", "test", false, None,
                )?)
                .is_err()
            {
                rejected = true;
                break;
            }
        }
        assert!(rejected);
        Ok(())
    }
}
