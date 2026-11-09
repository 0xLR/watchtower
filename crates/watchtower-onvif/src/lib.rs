//! Minimal ONVIF device information. PTZ remains out of scope.
use base64::{engine::general_purpose::STANDARD, Engine};
use sha1::{Digest, Sha1};
use std::time::Duration;
#[derive(Debug, thiserror::Error)]
#[error("ONVIF device information is unavailable.")]
pub struct OnvifError;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub serial: Option<String>,
    pub model: Option<String>,
}
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
pub fn password_digest(nonce: &[u8], created: &str, password: &str) -> String {
    let mut hash = Sha1::new();
    hash.update(nonce);
    hash.update(created.as_bytes());
    hash.update(password.as_bytes());
    STANDARD.encode(hash.finalize())
}
pub fn parse_device_info(text: &str) -> Result<DeviceInfo, OnvifError> {
    if text.len() > 65536 {
        return Err(OnvifError);
    }
    let document = roxmltree::Document::parse(text).map_err(|_| OnvifError)?;
    if document
        .descendants()
        .any(|n| n.is_element() && n.tag_name().name() == "Fault")
    {
        return Err(OnvifError);
    }
    if !document
        .descendants()
        .any(|n| n.is_element() && n.tag_name().name() == "GetDeviceInformationResponse")
    {
        return Err(OnvifError);
    }
    let value = |tag: &str| {
        document
            .descendants()
            .find(|n| n.is_element() && n.tag_name().name() == tag)
            .and_then(|n| n.text())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.chars().filter(|c| !c.is_control()).take(120).collect())
    };
    Ok(DeviceInfo {
        serial: value("SerialNumber"),
        model: value("Model"),
    })
}
pub async fn device_info(
    host: &str,
    port: u16,
    username: &str,
    password: Option<&str>,
) -> Result<DeviceInfo, OnvifError> {
    if !watchtower_core::is_private(host) {
        return Err(OnvifError);
    }
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.into()
    };
    let created = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| OnvifError)?;
    let nonce = uuid::Uuid::new_v4();
    let digest = password_digest(nonce.as_bytes(), &created, password.unwrap_or(""));
    let security = if username.is_empty() {
        String::new()
    } else {
        format!(
            r#"<s:Header><wsse:Security s:mustUnderstand="1" xmlns:wsse="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd" xmlns:wsu="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd"><wsse:UsernameToken><wsse:Username>{}</wsse:Username><wsse:Password Type="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordDigest">{digest}</wsse:Password><wsse:Nonce EncodingType="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary">{}</wsse:Nonce><wsu:Created>{created}</wsu:Created></wsse:UsernameToken></wsse:Security></s:Header>"#,
            xml(username),
            STANDARD.encode(nonce.as_bytes())
        )
    };
    let body = format!(
        r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:tds="http://www.onvif.org/ver10/device/wsdl">{security}<s:Body><tds:GetDeviceInformation/></s:Body></s:Envelope>"#
    );
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|_| OnvifError)?;
    let mut response=client.post(format!("http://{host}:{port}/onvif/device_service")).header("content-type","application/soap+xml; charset=utf-8; action=\"http://www.onvif.org/ver10/device/wsdl/GetDeviceInformation\"").body(body).send().await.map_err(|_|OnvifError)?.error_for_status().map_err(|_|OnvifError)?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| OnvifError)? {
        if bytes.len() + chunk.len() > 65536 {
            return Err(OnvifError);
        }
        bytes.extend_from_slice(&chunk);
    }
    parse_device_info(std::str::from_utf8(&bytes).map_err(|_| OnvifError)?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_namespaced_identity_and_rejects_faults() {
        let info=parse_device_info(r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:d="http://www.onvif.org/ver10/device/wsdl"><s:Body><d:GetDeviceInformationResponse><d:Model>Tapo C200</d:Model><d:SerialNumber>123</d:SerialNumber></d:GetDeviceInformationResponse></s:Body></s:Envelope>"#).unwrap();
        assert_eq!(info.serial.as_deref(), Some("123"));
        assert!(parse_device_info("<Fault>denied</Fault>").is_err());
    }
    #[test]
    fn digest_is_not_plaintext_and_xml_is_escaped() {
        assert_eq!(
            password_digest(b"nonce", "2026-11-09T12:00:00Z", "secret"),
            "1Me+OW1mq3I8hP9Eg/qBTDZgZus="
        );
        assert_eq!(xml("a<&"), "a&lt;&amp;");
    }
}
