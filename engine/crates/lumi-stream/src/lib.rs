//! Protocol-neutral, bounded newline framing for local and LAN transports.

#![forbid(unsafe_code)]

use std::io;
use tokio::io::{AsyncBufRead, AsyncBufReadExt as _};

/// One owner per connection. A cancelled `next_line` retains every consumed
/// byte. The only await is before copying/consuming input, so cancellation can
/// never occur between those two operations. Limits exclude the newline.
pub struct BoundedLineReader<R> {
    reader: R,
    pending: Vec<u8>,
    failed: bool,
}

impl<R: AsyncBufRead + Unpin> BoundedLineReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            pending: Vec::new(),
            failed: false,
        }
    }

    pub async fn next_line(&mut self, maximum: usize) -> io::Result<Option<Vec<u8>>> {
        if self.failed {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "frame reader is closed",
            ));
        }
        loop {
            let available = self.reader.fill_buf().await?;
            if available.is_empty() {
                if self.pending.is_empty() {
                    return Ok(None);
                }
                self.failed = true;
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated frame",
                ));
            }
            let newline = available.iter().position(|byte| *byte == b'\n');
            let length = newline.unwrap_or(available.len());
            if self.pending.len().saturating_add(length) > maximum {
                self.failed = true;
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "frame exceeds byte limit",
                ));
            }
            self.pending.extend_from_slice(&available[..length]);
            self.reader.consume(length + usize::from(newline.is_some()));
            if newline.is_some() {
                return Ok(Some(std::mem::take(&mut self.pending)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncWriteExt as _, BufReader, duplex};
    use tokio::time::{Duration, timeout};

    #[tokio::test]
    async fn cancellation_preserves_consumed_bytes_at_every_split() -> io::Result<()> {
        let frame = b"{\"kind\":\"command\",\"id\":1234}";
        for split in 1..frame.len() {
            let (mut peer, input) = duplex(256);
            let mut reader = BoundedLineReader::new(BufReader::with_capacity(3, input));
            peer.write_all(&frame[..split]).await?;
            assert!(
                timeout(Duration::from_millis(2), reader.next_line(128))
                    .await
                    .is_err()
            );
            peer.write_all(&frame[split..]).await?;
            peer.write_all(b"\nnext\n").await?;
            assert_eq!(reader.next_line(128).await?, Some(frame.to_vec()));
            assert_eq!(reader.next_line(128).await?, Some(b"next".to_vec()));
        }
        Ok(())
    }

    #[tokio::test]
    async fn oversized_unterminated_frame_is_rejected_without_waiting_for_eof() -> io::Result<()> {
        let (mut peer, input) = duplex(256);
        let mut reader = BoundedLineReader::new(BufReader::with_capacity(3, input));
        peer.write_all(b"123456789").await?;
        let result = timeout(Duration::from_millis(100), reader.next_line(8))
            .await
            .map_err(io::Error::other)?;
        assert!(result.is_err());
        assert!(reader.pending.len() <= 8);
        Ok(())
    }

    #[tokio::test]
    async fn exact_limit_and_clean_eof_are_valid_but_truncated_frame_is_not() -> io::Result<()> {
        let mut reader = BoundedLineReader::new(&b"1234\n\n"[..]);
        assert_eq!(reader.next_line(4).await?, Some(b"1234".to_vec()));
        assert_eq!(reader.next_line(0).await?, Some(vec![]));
        assert_eq!(reader.next_line(4).await?, None);
        let mut truncated = BoundedLineReader::new(&b"1234"[..]);
        assert_eq!(
            truncated.next_line(4).await.err().map(|error| error.kind()),
            Some(io::ErrorKind::UnexpectedEof)
        );
        Ok(())
    }
}
