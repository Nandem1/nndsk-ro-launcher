use ro_session_protocol::{SessionEvent, MAX_MESSAGE_BYTES};
#[cfg(test)]
use std::fs;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

pub struct EventWriter {
    inner: Mutex<Box<dyn Write + Send>>,
}

impl EventWriter {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Box::new(io::stdout())),
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn null_sink() -> Self {
        let sink = fs::OpenOptions::new()
            .write(true)
            .open("/dev/null")
            .unwrap_or_else(|_| fs::File::create("/dev/null").expect("/dev/null"));
        Self {
            inner: Mutex::new(Box::new(sink)),
        }
    }

    #[cfg(test)]
    pub fn collecting() -> (SharedWriter, Arc<Mutex<Vec<String>>>) {
        let lines = Arc::new(Mutex::new(Vec::new()));
        let capture = lines.clone();
        struct CollectingWriter {
            lines: Arc<Mutex<Vec<String>>>,
        }
        impl Write for CollectingWriter {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                if let Ok(mut s) = String::from_utf8(buf.to_vec()) {
                    if s.ends_with('\n') {
                        s.pop();
                    }
                    self.lines.lock().unwrap().push(s);
                }
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let writer = Arc::new(Self {
            inner: Mutex::new(Box::new(CollectingWriter { lines: capture })),
        });
        (writer, lines)
    }

    pub fn emit(&self, event: &SessionEvent) -> io::Result<()> {
        let line = serde_json::to_string(event).map_err(io::Error::other)?;
        let mut out = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("stdout writer poisoned"))?;
        writeln!(out, "{line}")?;
        out.flush()
    }
}

pub struct BoundedLineReader<R: Read> {
    reader: R,
    buf: Vec<u8>,
}

impl<R: Read> BoundedLineReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buf: Vec::with_capacity(MAX_MESSAGE_BYTES + 2),
        }
    }

    pub fn has_buffered_line(&self) -> bool {
        self.buf.contains(&b'\n')
    }

    /// Returns Ok(None) on EOF, Ok(Some(line)) on a line, Err on protocol violation.
    pub fn read_line(&mut self) -> io::Result<Option<String>> {
        loop {
            if let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = self.buf.drain(..=pos).collect();
                let line = String::from_utf8(line_bytes[..line_bytes.len() - 1].to_vec())
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid utf-8"))?;
                if line.is_empty() {
                    continue;
                }
                if line.len() > MAX_MESSAGE_BYTES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "message exceeds MAX_MESSAGE_BYTES",
                    ));
                }
                return Ok(Some(line));
            }

            let max_buf = MAX_MESSAGE_BYTES + 1;
            if self.buf.len() > max_buf {
                self.discard_oversized_line()?;
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "message exceeds MAX_MESSAGE_BYTES",
                ));
            }

            let room = max_buf.saturating_sub(self.buf.len());
            if room == 0 {
                self.discard_oversized_line()?;
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "message exceeds MAX_MESSAGE_BYTES",
                ));
            }
            let read_len = 4096.min(room);
            let mut chunk = vec![0u8; read_len];
            let n = match self.reader.read(&mut chunk) {
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "incomplete line"));
                }
                Err(e) => return Err(e),
            };
            if n == 0 {
                return Ok(None);
            }
            self.buf.extend_from_slice(&chunk[..n]);
            if self.buf.len() > max_buf && !self.buf.contains(&b'\n') {
                self.discard_oversized_line()?;
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "message exceeds MAX_MESSAGE_BYTES",
                ));
            }
        }
    }

    fn discard_oversized_line(&mut self) -> io::Result<()> {
        let mut scratch = [0u8; 4096];
        loop {
            if let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
                self.buf.drain(..=pos);
                self.buf.shrink_to(MAX_MESSAGE_BYTES + 2);
                return Ok(());
            }
            self.buf.clear();
            let n = match self.reader.read(&mut scratch) {
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "incomplete line"));
                }
                Err(e) => return Err(e),
            };
            if n == 0 {
                return Ok(());
            }
            if let Some(pos) = scratch[..n].iter().position(|&b| b == b'\n') {
                self.buf.extend_from_slice(&scratch[pos + 1..n]);
                return Ok(());
            }
        }
    }
}

pub type SharedWriter = Arc<EventWriter>;

#[cfg(test)]
mod tests {
    use super::*;
    use ro_session_protocol::PROTOCOL_VERSION;
    use std::io::Cursor;

    #[test]
    fn oversized_line_preserves_following_message() {
        let hello = format!(
            r#"{{"type":"hello","protocolVersion":{}}}"#,
            PROTOCOL_VERSION
        );
        let mut payload = vec![b'A'; MAX_MESSAGE_BYTES + 8];
        payload.push(b'\n');
        payload.extend_from_slice(hello.as_bytes());
        payload.push(b'\n');
        let mut reader = BoundedLineReader::new(Cursor::new(payload));
        let err = reader.read_line().unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        let next = reader.read_line().unwrap().expect("hello line");
        assert_eq!(next, hello);
    }

    #[test]
    fn complete_buffered_line_is_visible_without_another_read() {
        let mut reader = BoundedLineReader::new(Cursor::new(b"first\nsecond\n"));
        assert_eq!(reader.read_line().unwrap().as_deref(), Some("first"));
        assert!(reader.has_buffered_line());
        assert_eq!(reader.read_line().unwrap().as_deref(), Some("second"));
        assert!(!reader.has_buffered_line());
    }
}
