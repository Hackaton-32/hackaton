use anyhow::{anyhow, Result};
use tokio::process::Command as AsyncCommand;

pub struct CommandHandler {
    script_directory: String,
}

impl CommandHandler {
    pub fn new(script_directory: String) -> Self {
        Self { script_directory }
    }

    pub async fn handle_command(&self, command: &str) -> Result<String> {
        match command {
            "ALLOW_NETWORK" => self.run_script("an").await,
            "BLOCK_NETWORK" => self.run_script("bn").await,
            "LOCK_SCREEN" => self.run_script("sl").await,
            "LOCK_USB" => self.run_script("lu").await,
            "UNLOCK_USB" => self.run_script("uu").await,

            "CHECK_STATUS" => self.check_status().await,
            _ => Err(anyhow!("Unknown command: {}", command)),
        }
    }

    async fn run_script(&self, script_name: &str) -> Result<String> {
        let (script_path, shell_command) = if cfg!(target_os = "windows") {
            (
                format!("{}\\{}.bat", self.script_directory, script_name),
                "cmd",
            )
        } else {
            (
                format!("{}/{}.sh", self.script_directory, script_name),
                "bash",
            )
        };

        let mut command = AsyncCommand::new(shell_command);

        #[cfg(target_os = "windows")]
        {
            command.creation_flags(0x08000000);
            command.arg("/C");
        }

        #[cfg(not(target_os = "windows"))]
        {
            command.arg("-c");
        }

        command.arg(&script_path);

        let output = command.output().await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow!(
                "Script execution failed: {}\nError: {}",
                script_name,
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    async fn check_status(&self) -> Result<String> {
        let mut command = if cfg!(target_os = "windows") {
            AsyncCommand::new("tasklist")
        } else {
            AsyncCommand::new("ps")
        };

        if !cfg!(target_os = "windows") {
            command.arg("aux");
        }

        let output = command.output().await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow!(
                "Status check failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    pub fn is_script_exists(&self, script_name: &str) -> bool {
        let script_path = if cfg!(target_os = "windows") {
            format!("{}\\{}.bat", self.script_directory, script_name)
        } else {
            format!("{}/{}.sh", self.script_directory, script_name)
        };
        std::path::Path::new(&script_path).exists()
    }
}
