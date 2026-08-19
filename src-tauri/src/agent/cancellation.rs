use std::collections::HashMap;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub struct CancellationTree {
    application: CancellationToken,
    runs: Mutex<HashMap<String, CancellationToken>>,
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
    pub async fn begin_run(&self, run_id: String) -> CancellationToken {
        let token = self.application.child_token();
        if let Some(previous) = self.runs.lock().await.insert(run_id, token.clone()) {
            previous.cancel();
        }
        token
    }

    pub async fn finish_run(&self, run_id: &str) {
        self.runs.lock().await.remove(run_id);
    }

    pub async fn cancel_run(&self, run_id: &str) -> bool {
        if let Some(token) = self.runs.lock().await.remove(run_id) {
            token.cancel();
            true
        } else {
            false
        }
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
    async fn application_cancellation_reaches_every_run() {
        let tree = CancellationTree::default();
        let token = tree.begin_run("run".to_owned()).await;
        tree.cancel_application();
        assert!(token.is_cancelled());
    }
}
