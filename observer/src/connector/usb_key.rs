use crate::connector::device_operator::{Device, DeviceInfo, DeviceType};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::any::Any;
use std::time::Duration;

pub struct UsbKey {
    device: Box<dyn Device>,
    key_id: String,
}

impl UsbKey {
    pub fn new(device: Box<dyn Device>, key_id: String) -> Self {
        Self { device, key_id }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        self.device.connect().await?;
        let info = self.device.get_info().await?;
        if info.device_type != DeviceType::USB {
            return Err(anyhow!("Not a USB device"));
        }
        if info.id != self.key_id {
            return Err(anyhow!("Unexpected USB key"));
        }
        Ok(())
    }

    pub async fn read_data(&self, size: usize) -> Result<Vec<u8>> {
        self.device.read(size).await
    }

    pub async fn write_data(&self, data: &[u8]) -> Result<()> {
        self.device.write(data).await
    }

    pub async fn wait_for_command(&self, timeout: Duration) -> Result<String> {
        self.device.wait_for_command(timeout).await
    }
}

#[async_trait]
impl Device for UsbKey {
    async fn connect(&mut self) -> Result<()> {
        self.device.connect().await
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.device.disconnect().await
    }

    async fn read(&self, size: usize) -> Result<Vec<u8>> {
        self.read_data(size).await
    }

    async fn write(&self, data: &[u8]) -> Result<()> {
        self.write_data(data).await
    }

    async fn get_info(&self) -> Result<DeviceInfo> {
        self.device.get_info().await
    }

    async fn wait_for_command(&self, timeout: Duration) -> Result<String> {
        self.wait_for_command(timeout).await
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
