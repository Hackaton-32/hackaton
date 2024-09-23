use anyhow::Result;
use clap::Parser;
use file_monitor_core::FileMonitor;
use log::error;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::select;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the file to monitor
    #[arg(short, long)]
    path: PathBuf,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(if cli.debug {
        "debug"
    } else {
        "info"
    }))
    .init();

    let monitor = Arc::new(FileMonitor::new(cli.path));
    let monitor_clone = Arc::clone(&monitor);
    let mut monitor_handle = tokio::spawn(async move { monitor_clone.monitor().await });

    println!("File monitor started. Type 'help' for available commands.");

    let mut reader = BufReader::new(tokio::io::stdin()).lines();

    loop {
        select! {
            result = &mut monitor_handle => {
                match result {
                    Ok(Ok(())) => println!("Monitor finished successfully"),
                    Ok(Err(e)) => error!("Monitor error: {}", e),
                    Err(e) => error!("Monitor task error: {}", e),
                }
                break;
            }
            result = reader.next_line() => {
                match result {
                    Ok(Some(line)) => {
                        if !handle_command(&monitor, line.trim()).await? {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        error!("Failed to read line: {}", e);
                        continue;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_command(monitor: &Arc<FileMonitor>, command: &str) -> Result<bool> {
    match command.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["help"] => {
            println!("Available commands:");
            println!("  update <new_path> - Update the monitored file path");
            println!("  substitute <old_path> <new_path> - Substitute displayed path");
            println!(
                "  add_substitution <original_path> <substitute_path> - Add a path substitution"
            );
            println!("  remove_substitution <original_path> - Remove a path substitution");
            println!("  pause - Pause monitoring");
            println!("  resume - Resume monitoring");
            println!("  stats - Show event statistics");
            println!("  history - Show recent event history");
            println!("  quit - Exit the program");
        }
        ["update", new_path] => {
            if let Err(e) = monitor.update_path(new_path).await {
                error!("Failed to update path: {}", e);
            }
        }
        ["substitute", old_path, new_path] => {
            if let Err(e) = monitor.substitute_path(old_path, new_path).await {
                error!("Failed to substitute path: {}", e);
            }
        }
        ["add_substitution", original_path, substitute_path] => {
            if let Err(e) = monitor
                .add_path_substitution(original_path, substitute_path)
                .await
            {
                error!("Failed to add path substitution: {}", e);
            }
        }
        ["remove_substitution", original_path] => {
            if let Err(e) = monitor.remove_path_substitution(original_path).await {
                error!("Failed to remove path substitution: {}", e);
            }
        }
        ["pause"] => {
            if let Err(e) = monitor.pause().await {
                error!("Failed to pause monitoring: {}", e);
            }
        }
        ["resume"] => {
            if let Err(e) = monitor.resume().await {
                error!("Failed to resume monitoring: {}", e);
            }
        }
        ["stats"] => {
            let stats = monitor.get_stats().await;
            println!("Event statistics:");
            for (event, count) in stats {
                println!("  {:?}: {}", event, count);
            }
        }
        ["history"] => {
            let history = monitor.get_history().await;
            println!("Recent event history:");
            for (time, event) in history.iter().rev().take(10) {
                println!("  {} - {:?}", time, event);
            }
        }
        ["quit"] => return Ok(false),
        _ => println!("Unknown command. Type 'help' for available commands."),
    }
    Ok(true)
}
