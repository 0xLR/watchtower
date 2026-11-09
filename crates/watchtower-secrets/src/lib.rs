//! Credentials are keyed by camera UUID and never returned across IPC.
#[derive(Debug, thiserror::Error)]
#[error("The OS credential store is unavailable or locked. Unlock it and try again.")]
pub struct SecretError;
pub trait SecretStore: Send + Sync {
    fn get(&self, id: &str) -> Result<Option<String>, SecretError>;
    fn set(&self, id: &str, password: &str) -> Result<(), SecretError>;
    fn delete(&self, id: &str) -> Result<(), SecretError>;
}
pub struct Keychain;
fn entry(id: &str) -> Result<keyring::Entry, SecretError> {
    keyring::Entry::new("dev.watchtower.camera", id).map_err(|_| SecretError)
}
impl SecretStore for Keychain {
    fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
        match entry(id)?.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(SecretError),
        }
    }
    fn set(&self, id: &str, password: &str) -> Result<(), SecretError> {
        entry(id)?.set_password(password).map_err(|_| SecretError)
    }
    fn delete(&self, id: &str) -> Result<(), SecretError> {
        match entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(SecretError),
        }
    }
}
/// In-memory store for offline tests; never selected by application wiring.
#[derive(Default)]
pub struct MemorySecrets(std::sync::Mutex<std::collections::HashMap<String, String>>);
impl SecretStore for MemorySecrets {
    fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
        Ok(self.0.lock().map_err(|_| SecretError)?.get(id).cloned())
    }
    fn set(&self, id: &str, password: &str) -> Result<(), SecretError> {
        self.0
            .lock()
            .map_err(|_| SecretError)?
            .insert(id.into(), password.into());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<(), SecretError> {
        self.0.lock().map_err(|_| SecretError)?.remove(id);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn in_memory_round_trip_and_delete() {
        let secrets = MemorySecrets::default();
        secrets.set("id", "secret").unwrap();
        assert_eq!(secrets.get("id").unwrap().as_deref(), Some("secret"));
        secrets.delete("id").unwrap();
        assert_eq!(secrets.get("id").unwrap(), None);
    }
    #[test]
    #[ignore = "writes a temporary credential to the OS keychain"]
    fn os_keychain_round_trip() {
        let secrets = Keychain;
        let id = "watchtower-integration-test";
        secrets.set(id, "test-only-value").unwrap();
        assert_eq!(secrets.get(id).unwrap().as_deref(), Some("test-only-value"));
        secrets.delete(id).unwrap();
    }
}
