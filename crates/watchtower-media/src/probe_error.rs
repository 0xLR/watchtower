use crate::MediaError;

fn contains_any(text: &str, values: &[&str]) -> bool {
    values.iter().any(|value| text.contains(value))
}

pub(crate) fn classify(body: &str) -> MediaError {
    let text = watchtower_core::redact(body).to_ascii_lowercase();
    if contains_any(
        &text,
        &[
            "wrong user/pass",
            "user/pass not provided",
            "401 unauthorized",
            "status=401",
            "403 forbidden",
        ],
    ) {
        MediaError::Authentication
    } else if contains_any(&text, &["timeout", "timed out", "deadline exceeded"]) {
        MediaError::Timeout
    } else if contains_any(&text, &["connection refused", "actively refused"]) {
        MediaError::Refused
    } else if contains_any(&text, &["404 not found", "status=404", "stream not found"]) {
        MediaError::Path
    } else if text.contains("wrong response on describe") {
        MediaError::DescribeRejected
    } else if text.contains("wrong response on setup") {
        MediaError::SetupRejected
    } else if text.contains("wrong response on options") {
        MediaError::OptionsRejected
    } else if text.contains("wrong response on ") {
        MediaError::Rejected
    } else {
        MediaError::Connection
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostics_are_specific_and_never_echo_upstream_material() {
        for (body, expected) in [
            ("streams: wrong user/pass", "authentication"),
            ("rtsp: user/pass not provided", "authentication"),
            ("RTSP/1.0 401 Unauthorized", "authentication"),
            ("dial tcp: i/o timeout", "respond in time"),
            ("dial tcp: connection refused", "refused"),
            ("RTSP/1.0 404 Not Found", "stream path"),
            ("wrong response on DESCRIBE", "rejected the RTSP"),
            ("unexpected camera failure", "could not connect"),
        ] {
            let result = classify(&format!(
                "{body} rtsp://secret-user:secret-password@private-host/live"
            ))
            .to_string();
            assert!(result.contains(expected));
            for secret in ["secret-user", "secret-password", "private-host", "/live"] {
                assert!(!result.contains(secret));
            }
        }
        assert!(matches!(
            classify("failure rtsp://user:timeout@host/live"),
            MediaError::Connection
        ));
    }

    #[test]
    fn rejected_requests_preserve_only_allowlisted_stage() {
        for (method, stage) in [
            ("DESCRIBE", "description"),
            ("SETUP", "transport setup"),
            ("OPTIONS", "capability check"),
        ] {
            let message = classify(&format!(
                "wrong response on {method} rtsp://private:secret@camera/live"
            ))
            .to_string();
            assert!(message.contains(stage));
            assert!(!message.contains("secret"));
        }
    }
}
