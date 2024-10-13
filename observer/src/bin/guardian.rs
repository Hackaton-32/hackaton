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
