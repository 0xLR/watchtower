use std::io::{self, Write};
/// Buffer complete tracing records so URLs split across formatter writes are redacted.
pub struct RedactingWriter<W: Write> {
    target: W,
    bytes: Vec<u8>,
}
impl<W: Write> RedactingWriter<W> {
    pub fn new(target: W) -> Self {
        Self {
            target,
            bytes: Vec::new(),
        }
    }
}
impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl<W: Write> Drop for RedactingWriter<W> {
    fn drop(&mut self) {
        let sanitized = watchtower_core::redact(&String::from_utf8_lossy(&self.bytes));
        let _ = writeln!(self.target, "{sanitized}");
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logs_never_contain_complete_or_split_credential_urls() {
        let mut logs = Vec::new();
        {
            let mut writer = RedactingWriter::new(&mut logs);
            writer.write_all(b"connection failed rt").unwrap();
            writer
                .write_all(b"sp://viewer:unique-camera-password@192.168.1.30/live?token=hidden")
                .unwrap();
        }
        let logs = String::from_utf8(logs).unwrap();
        for value in [
            "unique-camera-password",
            "rtsp://",
            "viewer",
            "hidden",
            "192.168.1.30",
        ] {
            assert!(!logs.contains(value));
        }
        assert!(logs.contains("[redacted-url]"));
    }
}
