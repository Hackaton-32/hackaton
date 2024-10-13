use crate::connector::usb_key::UsbKey;
use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};

pub struct SecurityManager {
    expected_key_hash: String,
}

impl SecurityManager {
    pub fn new(expected_key_hash: String) -> Self {
        Self { expected_key_hash }
    }

    pub async fn verify_key(&self, usb_key: &UsbKey) -> Result<bool> {
        let key_data = usb_key.read_data(1024).await?;
        let key_hash = self.hash_data(&key_data);

        Ok(key_hash == self.expected_key_hash)
    }

    fn hash_data(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    pub async fn authenticate_key(&self, usb_key: &UsbKey) -> Result<()> {
        if self.verify_key(usb_key).await? {
            Ok(())
        } else {
            Err(anyhow!("Key authentication failed"))
        }
    }
}
