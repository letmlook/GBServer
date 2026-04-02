//! SIP Dialog 管理

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct Dialog {
    pub call_id: String,
    pub local_tag: String,
    pub remote_tag: String,
    pub local_uri: String,
    pub remote_uri: String,
    pub contact: String,
    pub route_set: Vec<String>,
    pub local_seq: u32,
    pub remote_seq: u32,
    pub established: bool,
}

pub struct DialogManager {
    dialogs: Arc<RwLock<HashMap<String, Dialog>>>,
}

impl DialogManager {
    pub fn new() -> Self {
        Self {
            dialogs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create(&self, call_id: &str, local_tag: &str, remote_tag: &str) -> Dialog {
        let dialog = Dialog {
            call_id: call_id.to_string(),
            local_tag: local_tag.to_string(),
            remote_tag: remote_tag.to_string(),
            local_uri: String::new(),
            remote_uri: String::new(),
            contact: String::new(),
            route_set: Vec::new(),
            local_seq: 0,
            remote_seq: 0,
            established: false,
        };
        let key = format!("{}-{}", call_id, local_tag);
        self.dialogs.write().await.insert(key, dialog.clone());
        dialog
    }

    pub async fn get(&self, call_id: &str, local_tag: &str) -> Option<Dialog> {
        let key = format!("{}-{}", call_id, local_tag);
        self.dialogs.read().await.get(&key).cloned()
    }

    pub async fn update(&self, dialog: &Dialog) {
        let key = format!("{}-{}", dialog.call_id, dialog.local_tag);
        self.dialogs.write().await.insert(key, dialog.clone());
    }

    pub async fn remove(&self, call_id: &str, local_tag: &str) {
        let key = format!("{}-{}", call_id, local_tag);
        self.dialogs.write().await.remove(&key);
    }
}

impl Default for DialogManager {
    fn default() -> Self {
        Self::new()
    }
}
