use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::time::sleep;

pub fn shared_tokio_runtime() -> Arc<Runtime> {
    static RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();

    RUNTIME
        .get_or_init(|| Arc::new(Runtime::new().expect("Failed to create shared Tokio runtime")))
        .clone()
}

struct DebouncedJsonWriterState<T> {
    pending: Option<T>,
    task_running: bool,
}

#[derive(Clone)]
pub struct DebouncedJsonWriter<T> {
    label: &'static str,
    path: PathBuf,
    delay: Duration,
    runtime: Arc<Runtime>,
    state: Arc<Mutex<DebouncedJsonWriterState<T>>>,
}

impl<T> DebouncedJsonWriter<T>
where
    T: Serialize + Send + 'static,
{
    pub fn new(label: &'static str, path: PathBuf, delay: Duration) -> Self {
        Self {
            label,
            path,
            delay,
            runtime: shared_tokio_runtime(),
            state: Arc::new(Mutex::new(DebouncedJsonWriterState {
                pending: None,
                task_running: false,
            })),
        }
    }

    pub fn schedule_save(&self, value: T) {
        let should_spawn = {
            let mut state = self.state.lock().expect("persistence state poisoned");
            state.pending = Some(value);

            if state.task_running {
                false
            } else {
                state.task_running = true;
                true
            }
        };

        if !should_spawn {
            return;
        }

        let label = self.label;
        let path = self.path.clone();
        let delay = self.delay;
        let state = self.state.clone();

        self.runtime.spawn(async move {
            loop {
                sleep(delay).await;

                let snapshot = {
                    let mut state = state.lock().expect("persistence state poisoned");
                    state.pending.take()
                };

                let Some(snapshot) = snapshot else {
                    let mut state = state.lock().expect("persistence state poisoned");
                    state.task_running = false;
                    return;
                };

                if let Some(parent) = path.parent() {
                    if let Err(err) = tokio::fs::create_dir_all(parent).await {
                        log::error!("Failed to create {} storage directory: {}", label, err);
                    }
                }

                match serde_json::to_string(&snapshot) {
                    Ok(contents) => {
                        if let Err(err) = tokio::fs::write(&path, contents).await {
                            log::error!("Failed to save {}: {}", label, err);
                        }
                    }
                    Err(err) => {
                        log::error!("Failed to serialize {}: {}", label, err);
                    }
                }

                let should_continue = {
                    let mut state = state.lock().expect("persistence state poisoned");
                    if state.pending.is_some() {
                        true
                    } else {
                        state.task_running = false;
                        false
                    }
                };

                if !should_continue {
                    return;
                }
            }
        });
    }
}
