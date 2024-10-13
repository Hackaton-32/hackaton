use anyhow::{anyhow, Result};
use async_trait::async_trait;
use observer::connector::{Device, DeviceInfo, DeviceManager, DeviceType, SecurityManager, UsbKey};
use observer::handler::CommandHandler;
use std::any::Any;
use std::path::Path;
use std::time::Duration;

const USB_TIMEOUT: Duration = Duration::from_secs(60);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_DIR: &str = "./response";
const EXPECTED_KEY_HASH: &str = "your_expected_key_hash_here";

#[cfg(target_os = "windows")]
const OS_SPECIFIC_DIR: &str = "win";
#[cfg(not(target_os = "windows"))]
const OS_SPECIFIC_DIR: &str = "nix";

struct PlaceholderDeviceManager;

#[async_trait]
impl DeviceManager for PlaceholderDeviceManager {
    async fn list_devices(&self) -> Result<Vec<DeviceInfo>> {
        Ok(vec![])
    }

    async fn get_device(&self, _id: &str) -> Result<Box<dyn Device>> {
        Err(anyhow!("Not implemented"))
    }

    async fn wait_for_device(&self, _timeout: Duration) -> Result<Box<dyn Device>> {
        Ok(Box::new(UsbKey::new(
            Box::new(PlaceholderDevice),
            "placeholder_key_id".to_string(),
        )))
    }
}

struct PlaceholderDevice;

#[async_trait]
impl Device for PlaceholderDevice {
    async fn connect(&mut self) -> Result<()> {
        Ok(())
    }
    async fn disconnect(&mut self) -> Result<()> {
        Ok(())
    }
    async fn read(&self, _size: usize) -> Result<Vec<u8>> {
        Ok(vec![])
    }
    async fn write(&self, _data: &[u8]) -> Result<()> {
        Ok(())
    }
    async fn get_info(&self) -> Result<DeviceInfo> {
        Ok(DeviceInfo {
            name: "Placeholder".to_string(),
            id: "placeholder_id".to_string(),
            device_type: DeviceType::USB,
        })
    }
    async fn wait_for_command(&self, _timeout: Duration) -> Result<String> {
        Ok("PLACEHOLDER_COMMAND".to_string())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Guardian starting...");

    let device_manager: Box<dyn DeviceManager> = Box::new(PlaceholderDeviceManager);
    let security_manager = SecurityManager::new(EXPECTED_KEY_HASH.to_string());
    let script_directory = Path::new(RESPONSE_DIR).join(OS_SPECIFIC_DIR);
    let command_handler = CommandHandler::new(script_directory.to_string_lossy().to_string());

    loop {
        println!("Waiting for USB key...");
        let mut device = device_manager.wait_for_device(USB_TIMEOUT).await?;

        let device_any = device.as_any_mut();
        if let Some(usb_key) = device_any.downcast_mut::<UsbKey>() {
            println!("USB key detected. Initializing...");
            if let Err(e) = usb_key.initialize().await {
                println!("Failed to initialize USB key: {}", e);
                continue;
            }

            println!("Authenticating USB key...");
            if let Err(e) = security_manager.authenticate_key(usb_key).await {
                println!("Authentication failed: {}", e);
                continue;
            }

            println!("USB key authenticated. Waiting for commands...");
            loop {
                match usb_key.wait_for_command(COMMAND_TIMEOUT).await {
                    Ok(command) => {
                        println!("Received command: {}", command);
                        match command_handler.handle_command(&command).await {
                            Ok(result) => println!("Command executed successfully: {}", result),
                            Err(e) => println!("Error executing command: {}", e),
                        }
                    }
                    Err(e) => {
                        println!("Error waiting for command: {}", e);
                        break;
                    }
                }
            }

            println!("Disconnecting USB key...");
            if let Err(e) = usb_key.disconnect().await {
                println!("Error disconnecting USB key: {}", e);
            }
        } else {
            println!("Connected device is not a USB key. Ignoring.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use sha2::{Digest, Sha256};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct MockDevice {
        command_queue: Arc<Mutex<Vec<String>>>,
        key_data: Vec<u8>,
    }

    impl MockDevice {
        fn new(key_data: Vec<u8>) -> Self {
            Self {
                command_queue: Arc::new(Mutex::new(vec![])),
                key_data,
            }
        }

        async fn add_command(&self, command: String) {
            self.command_queue.lock().await.push(command);
        }
    }

    #[async_trait::async_trait]
    impl Device for MockDevice {
        async fn connect(&mut self) -> Result<()> {
            Ok(())
        }
        async fn disconnect(&mut self) -> Result<()> {
            Ok(())
        }
        async fn read(&self, _size: usize) -> Result<Vec<u8>> {
            Ok(self.key_data.clone())
        }
        async fn write(&self, _data: &[u8]) -> Result<()> {
            Ok(())
        }
        async fn get_info(&self) -> Result<DeviceInfo> {
            Ok(DeviceInfo {
                name: "MockDevice".to_string(),
                id: "test_key_id".to_string(),
                device_type: DeviceType::USB,
            })
        }
        async fn wait_for_command(&self, timeout: Duration) -> Result<String> {
            tokio::time::timeout(timeout, async {
                loop {
                    if let Some(cmd) = self.command_queue.lock().await.pop() {
                        return Ok(cmd);
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            })
            .await?
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[async_trait::async_trait]
    impl Device for MockDeviceWrapper {
        async fn connect(&mut self) -> Result<()> {
            self.inner.lock().await.connect().await
        }

        async fn disconnect(&mut self) -> Result<()> {
            self.inner.lock().await.disconnect().await
        }

        async fn read(&self, size: usize) -> Result<Vec<u8>> {
            self.inner.lock().await.read(size).await
        }

        async fn write(&self, data: &[u8]) -> Result<()> {
            self.inner.lock().await.write(data).await
        }

        async fn get_info(&self) -> Result<DeviceInfo> {
            self.inner.lock().await.get_info().await
        }

        async fn wait_for_command(&self, timeout: Duration) -> Result<String> {
            self.inner.lock().await.wait_for_command(timeout).await
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    struct MockDeviceWrapper {
        inner: Arc<Mutex<MockDevice>>,
    }

    fn calculate_hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    #[tokio::test]
    async fn test_usb_key_initialize() -> Result<()> {
        let key_data = b"test_key_data".to_vec();
        let mock_device = Box::new(MockDevice::new(key_data));
        let mut usb_key = UsbKey::new(mock_device, "test_key_id".to_string());

        usb_key.initialize().await?;
        Ok(())
    }

    fn bytes_to_hex_string(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[tokio::test]
    async fn test_security_manager_authentication() -> Result<()> {
        let key_data = b"test_key_data".to_vec();
        let mock_device = Box::new(MockDevice::new(key_data.clone()));
        let usb_key = UsbKey::new(mock_device, "test_key_id".to_string());
        let expected_hash = calculate_hash(&key_data);
        let security_manager = SecurityManager::new(expected_hash);

        security_manager.authenticate_key(&usb_key).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_command_handler() -> Result<()> {
        let temp_dir = std::env::current_dir()?.join("test_scripts");
        let command_handler = CommandHandler::new(temp_dir.to_string_lossy().to_string());

        let result = command_handler.handle_command("ALLOW_NETWORK").await?;
        assert!(
            result.contains("ALLOW_NETWORK script executed"),
            "Unexpected result: {}",
            result
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_integration() -> Result<()> {
        println!("Starting test_integration");
        let key_data = b"test_key_data".to_vec();
        let mock_device = Arc::new(Mutex::new(MockDevice::new(key_data.clone())));
        let mock_device_wrapper = Box::new(MockDeviceWrapper {
            inner: mock_device.clone(),
        });
        let mut usb_key = UsbKey::new(mock_device_wrapper, "test_key_id".to_string());
        let expected_hash = calculate_hash(&key_data);
        let security_manager = SecurityManager::new(expected_hash);

        println!("Initializing USB key");
        usb_key.initialize().await?;
        println!("Authenticating USB key");
        security_manager.authenticate_key(&usb_key).await?;

        println!("Adding command to the queue");
        mock_device
            .lock()
            .await
            .add_command("ALLOW_NETWORK".to_string())
            .await;

        println!("Waiting for command");
        let command = tokio::time::timeout(
            Duration::from_secs(6),
            usb_key.wait_for_command(Duration::from_secs(5)),
        )
        .await??;
        println!("Received command: {}", command);
        assert_eq!(command, "ALLOW_NETWORK");

        println!("Handling command");
        let temp_dir = std::env::current_dir()?.join("test_scripts");
        let command_handler = CommandHandler::new(temp_dir.to_string_lossy().to_string());
        let result = command_handler.handle_command(&command).await?;
        println!("Command result: {}", result);
        assert!(
            result.contains("ALLOW_NETWORK script executed"),
            "Unexpected result: {}",
            result
        );

        println!("Test completed successfully");
        Ok(())
    }
}
