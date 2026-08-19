use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub struct CancellationTree {
    application: CancellationToken,
    runs: Mutex<HashMap<String, RunCancellation>>,
}

const STOP_NONE: u8 = 0;
const STOP_CANCEL: u8 = 1;
const STOP_PAUSE: u8 = 2;

#[derive(Clone)]
pub struct RunCancellation {
    token: CancellationToken,
    stop_request: Arc<AtomicU8>,
}

impl RunCancellation {
    #[cfg(test)]
    pub fn detached() -> Self {
        Self {
            token: CancellationToken::new(),
            stop_request: Arc::new(AtomicU8::new(STOP_NONE)),
        }
    }

    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    pub fn is_pause_requested(&self) -> bool {
        self.stop_request.load(Ordering::SeqCst) == STOP_PAUSE
    }

    fn cancel(&self) {
        self.stop_request.store(STOP_CANCEL, Ordering::SeqCst);
        self.token.cancel();
    }

    fn pause(&self) {
        self.stop_request.store(STOP_PAUSE, Ordering::SeqCst);
        self.token.cancel();
    }
}

impl Default for CancellationTree {
    fn default() -> Self {
        Self {
            application: CancellationToken::new(),
            runs: Mutex::new(HashMap::new()),
        }
    }
}

impl CancellationTree {
    pub async fn begin_run(&self, run_id: String) -> RunCancellation {
        let cancellation = RunCancellation {
            token: self.application.child_token(),
            stop_request: Arc::new(AtomicU8::new(STOP_NONE)),
        };
        if let Some(previous) = self.runs.lock().await.insert(run_id, cancellation.clone()) {
            previous.cancel();
        }
        cancellation
    }

    pub async fn finish_run(&self, run_id: &str) {
        self.runs.lock().await.remove(run_id);
    }

    pub async fn cancel_run(&self, run_id: &str) -> bool {
        if let Some(cancellation) = self.runs.lock().await.remove(run_id) {
            cancellation.cancel();
            true
        } else {
            false
        }
    }

    pub async fn pause_run(&self, run_id: &str) -> bool {
        if let Some(cancellation) = self.runs.lock().await.remove(run_id) {
            cancellation.pause();
            true
        } else {
            false
        }
    }

    pub async fn active_run_ids(&self) -> Vec<String> {
        self.runs.lock().await.keys().cloned().collect()
    }

    pub fn cancel_application(&self) {
        self.application.cancel();
    }
}

impl Drop for CancellationTree {
    fn drop(&mut self) {
        self.cancel_application();
    }
}

#[cfg(test)]
mod tests {
    use super::CancellationTree;

    #[tokio::test]
    async fn cancelling_a_run_does_not_cancel_siblings() {
        let tree = CancellationTree::default();
        let first = tree.begin_run("first".to_owned()).await;
        let second = tree.begin_run("second".to_owned()).await;
        assert!(tree.cancel_run("first").await);
        assert!(first.is_cancelled());
        assert!(!second.is_cancelled());
    }

    #[tokio::test]
    async fn pause_is_distinct_from_cancel() {
        let tree = CancellationTree::default();
        let paused = tree.begin_run("paused".to_owned()).await;
        let cancelled = tree.begin_run("cancelled".to_owned()).await;
        assert!(tree.pause_run("paused").await);
        assert!(tree.cancel_run("cancelled").await);
        assert!(paused.is_pause_requested());
        assert!(!cancelled.is_pause_requested());
    }

    #[tokio::test]
    async fn application_cancellation_reaches_every_run() {
        let tree = CancellationTree::default();
        let token = tree.begin_run("run".to_owned()).await;
        tree.cancel_application();
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn active_run_ids_support_lifecycle_checkpoints() {
        let tree = CancellationTree::default();
        tree.begin_run("first".to_owned()).await;
        tree.begin_run("second".to_owned()).await;
        let mut ids = tree.active_run_ids().await;
        ids.sort();
        assert_eq!(ids, vec!["first", "second"]);
    }
}
