use anyhow::Result;
use async_trait::async_trait;
use std::any::Any;
use std::time::Duration;

#[async_trait]
pub trait Device: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn read(&self, size: usize) -> Result<Vec<u8>>;
    async fn write(&self, data: &[u8]) -> Result<()>;
    async fn get_info(&self) -> Result<DeviceInfo>;
    async fn wait_for_command(&self, timeout: Duration) -> Result<String>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub id: String,
    pub device_type: DeviceType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceType {
    USB,
    Disk,
    Other,
}

#[async_trait]
pub trait DeviceManager: Send + Sync {
    async fn list_devices(&self) -> Result<Vec<DeviceInfo>>;
    async fn get_device(&self, id: &str) -> Result<Box<dyn Device>>;
    async fn wait_for_device(&self, timeout: Duration) -> Result<Box<dyn Device>>;
}
