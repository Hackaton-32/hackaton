use anyhow::Result;
use chrono::{DateTime, Local};
use log::{debug, error, info};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct FileMonitor {
    current_path: Arc<Mutex<PathBuf>>,
    substitute_path: Arc<Mutex<Option<PathBuf>>>,
    watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
    event_history: Arc<Mutex<Vec<(DateTime<Local>, FileEvent)>>>,
    stats: Arc<Mutex<HashMap<FileEvent, usize>>>,
    is_paused: Arc<Mutex<bool>>,
    path_substitutions: Arc<Mutex<HashMap<PathBuf, PathBuf>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FileEvent {
    Opened,
    Modified,
    Deleted,
    Renamed(PathBuf),
    Created,
    Closed,
}

impl FileMonitor {
    pub fn new<P: AsRef<Path>>(initial_path: P) -> Self {
        FileMonitor {
            current_path: Arc::new(Mutex::new(initial_path.as_ref().to_path_buf())),
            substitute_path: Arc::new(Mutex::new(None)),
            watcher: Arc::new(Mutex::new(None)),
            event_history: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(Mutex::new(HashMap::new())),
            is_paused: Arc::new(Mutex::new(false)),
            path_substitutions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn monitor(&self) -> Result<()> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);

        let path = self.current_path.lock().await.clone();
        let watcher = self.create_watcher(tx.clone())?;

        {
            let mut watcher_lock = self.watcher.lock().await;
            *watcher_lock = Some(watcher);
        }

        self.watch_path(&path).await?;

        while let Some(event) = rx.recv().await {
            if !*self.is_paused.lock().await {
                if let Some(file_event) = self.map_event(event) {
                    self.handle_event(file_event).await?;
                }
            }
        }

        Ok(())
    }

    fn create_watcher(
        &self,
        tx: tokio::sync::mpsc::Sender<Event>,
    ) -> Result<notify::RecommendedWatcher> {
        let watcher =
            notify::recommended_watcher(move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    let _ = tx.blocking_send(event);
                }
                Err(e) => error!("Watch error: {:?}", e),
            })?;
        Ok(watcher)
    }

    async fn watch_path(&self, path: &Path) -> Result<()> {
        let mut watcher_lock = self.watcher.lock().await;
        if let Some(watcher) = watcher_lock.as_mut() {
            watcher.watch(path, RecursiveMode::NonRecursive)?;
            info!("Now watching path: {}", path.display());
        }
        Ok(())
    }

    fn map_event(&self, event: Event) -> Option<FileEvent> {
        match event.kind {
            EventKind::Access(notify::event::AccessKind::Close(_)) => Some(FileEvent::Closed),
            EventKind::Access(_) => Some(FileEvent::Opened),
            EventKind::Modify(_) => Some(FileEvent::Modified),
            EventKind::Remove(_) => Some(FileEvent::Deleted),
            EventKind::Create(_) => Some(FileEvent::Created),
            EventKind::Any => {
                if event.paths.len() == 2 {
                    Some(FileEvent::Renamed(event.paths[1].clone()))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    async fn handle_event(&self, event: FileEvent) -> Result<()> {
        let path = self.current_path.lock().await;
        let substitute = self.substitute_path.lock().await;
        let now = Local::now();

        let display_path = substitute.as_ref().unwrap_or(&path);
        let substituted_path = self.get_substituted_path(display_path).await;

        let event_message = match &event {
            FileEvent::Opened => format!(
                "File opened: {} (actual: {})",
                display_path.display(),
                substituted_path.display()
            ),
            FileEvent::Modified => format!(
                "File modified: {} (actual: {})",
                display_path.display(),
                substituted_path.display()
            ),
            FileEvent::Deleted => format!(
                "File deleted: {} (actual: {})",
                display_path.display(),
                substituted_path.display()
            ),
            FileEvent::Created => format!(
                "File created: {} (actual: {})",
                display_path.display(),
                substituted_path.display()
            ),
            FileEvent::Renamed(new_path) => {
                let substituted_new_path = self.get_substituted_path(new_path).await;
                format!(
                    "File renamed from {} (actual: {}) to {} (actual: {})",
                    display_path.display(),
                    substituted_path.display(),
                    new_path.display(),
                    substituted_new_path.display()
                )
            }
            FileEvent::Closed => format!(
                "File closed: {} (actual: {})",
                display_path.display(),
                substituted_path.display()
            ),
        };

        info!("{} at {}", event_message, now);

        self.update_history(now, event.clone()).await;
        self.update_stats(event).await;

        Ok(())
    }

    async fn update_history(&self, time: DateTime<Local>, event: FileEvent) {
        let mut history = self.event_history.lock().await;
        history.push((time, event));
        if history.len() > 100 {
            history.remove(0);
        }
    }

    async fn update_stats(&self, event: FileEvent) {
        let mut stats = self.stats.lock().await;
        *stats.entry(event).or_insert(0) += 1;
    }

    pub async fn update_path<P: AsRef<Path>>(&self, new_path: P) -> Result<()> {
        let new_path = new_path.as_ref();
        let absolute_path = if new_path.is_relative() {
            std::env::current_dir()?.join(new_path)
        } else {
            new_path.to_path_buf()
        };

        let mut current_path = self.current_path.lock().await;
        debug!(
            "Updating path from {} to {}",
            current_path.display(),
            absolute_path.display()
        );

        if let Some(watcher) = self.watcher.lock().await.as_mut() {
            watcher.unwatch(&*current_path)?;
            watcher.watch(&absolute_path, RecursiveMode::NonRecursive)?;
        }

        *current_path = absolute_path.clone();
        info!("Path updated to: {}", absolute_path.display());
        Ok(())
    }

    pub async fn substitute_path<P: AsRef<Path>>(&self, old_path: P, new_path: P) -> Result<()> {
        let current_path = self.current_path.lock().await;
        let mut substitute = self.substitute_path.lock().await;

        if current_path.as_path() == old_path.as_ref() {
            *substitute = Some(new_path.as_ref().to_path_buf());
            info!(
                "Path substituted: {} -> {}",
                old_path.as_ref().display(),
                new_path.as_ref().display()
            );
        } else {
            error!("Cannot substitute path: current path does not match the old path");
        }

        Ok(())
    }

    pub async fn pause(&self) -> Result<()> {
        let mut is_paused = self.is_paused.lock().await;
        *is_paused = true;
        info!("Monitoring paused");
        Ok(())
    }

    pub async fn resume(&self) -> Result<()> {
        let mut is_paused = self.is_paused.lock().await;
        *is_paused = false;
        info!("Monitoring resumed");
        Ok(())
    }

    pub async fn get_stats(&self) -> HashMap<FileEvent, usize> {
        self.stats.lock().await.clone()
    }

    pub async fn get_history(&self) -> Vec<(DateTime<Local>, FileEvent)> {
        self.event_history.lock().await.clone()
    }

    pub async fn add_path_substitution<P: AsRef<Path>>(
        &self,
        original_path: P,
        substitute_path: P,
    ) -> Result<()> {
        let mut substitutions = self.path_substitutions.lock().await;
        substitutions.insert(
            original_path.as_ref().to_path_buf(),
            substitute_path.as_ref().to_path_buf(),
        );
        info!(
            "Path substitution added: {} -> {}",
            original_path.as_ref().display(),
            substitute_path.as_ref().display()
        );
        Ok(())
    }

    pub async fn remove_path_substitution<P: AsRef<Path>>(&self, original_path: P) -> Result<()> {
        let mut substitutions = self.path_substitutions.lock().await;
        if substitutions.remove(original_path.as_ref()).is_some() {
            info!(
                "Path substitution removed for: {}",
                original_path.as_ref().display()
            );
        } else {
            info!(
                "No path substitution found for: {}",
                original_path.as_ref().display()
            );
        }
        Ok(())
    }

    pub async fn get_substituted_path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        let substitutions = self.path_substitutions.lock().await;
        substitutions
            .get(path.as_ref())
            .cloned()
            .unwrap_or_else(|| path.as_ref().to_path_buf())
    }
}
