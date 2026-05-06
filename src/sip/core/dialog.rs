use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use super::message::SipRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogState {
    Early,
    Confirmed,
    Terminated,
}

#[derive(Debug, Clone)]
pub struct Dialog {
    pub id: String,
    pub call_id: String,
    pub local_tag: String,
    pub remote_tag: String,
    pub state: DialogState,
    pub local_uri: String,
    pub remote_uri: String,
    pub remote_target: String,
    pub route_set: Vec<String>,
    pub local_seq: u32,
    pub remote_seq: u32,
    pub local_contact: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub secure: bool,
    pub sdp_answer: Option<String>,
    pub sdp_offer: Option<String>,
}

impl Dialog {
    pub fn new_uac(
        call_id: &str,
        local_tag: &str,
        remote_tag: &str,
        local_uri: &str,
        remote_uri: &str,
        remote_target: &str,
        contact: &str,
    ) -> Self {
        let id = Self::generate_id(call_id, local_tag, remote_tag);
        Self {
            id,
            call_id: call_id.to_string(),
            local_tag: local_tag.to_string(),
            remote_tag: remote_tag.to_string(),
            state: DialogState::Early,
            local_uri: local_uri.to_string(),
            remote_uri: remote_uri.to_string(),
            remote_target: remote_target.to_string(),
            route_set: Vec::new(),
            local_seq: 0,
            remote_seq: 0,
            local_contact: contact.to_string(),
            created_at: chrono::Utc::now(),
            secure: false,
            sdp_answer: None,
            sdp_offer: None,
        }
    }
    
    pub fn new_uas(
        call_id: &str,
        local_tag: &str,
        remote_tag: &str,
        local_uri: &str,
        remote_uri: &str,
        remote_target: &str,
        contact: &str,
        local_seq: u32,
    ) -> Self {
        let id = Self::generate_id(call_id, local_tag, remote_tag);
        Self {
            id,
            call_id: call_id.to_string(),
            local_tag: local_tag.to_string(),
            remote_tag: remote_tag.to_string(),
            state: DialogState::Early,
            local_uri: local_uri.to_string(),
            remote_uri: remote_uri.to_string(),
            remote_target: remote_target.to_string(),
            route_set: Vec::new(),
            local_seq,
            remote_seq: 0,
            local_contact: contact.to_string(),
            created_at: chrono::Utc::now(),
            secure: false,
            sdp_answer: None,
            sdp_offer: None,
        }
    }
    
    fn generate_id(call_id: &str, local_tag: &str, remote_tag: &str) -> String {
        format!("{}-{}-{}", call_id, local_tag, remote_tag)
    }
    
    pub fn is_early(&self) -> bool {
        self.state == DialogState::Early
    }
    
    pub fn is_confirmed(&self) -> bool {
        self.state == DialogState::Confirmed
    }
    
    pub fn is_terminated(&self) -> bool {
        self.state == DialogState::Terminated
    }
    
    pub fn is_local(&self, tag: &str) -> bool {
        self.local_tag == tag
    }
    
    pub fn is_remote(&self, tag: &str) -> bool {
        self.remote_tag == tag
    }
    
    pub fn update_remote_tag(&mut self, remote_tag: &str) {
        self.remote_tag = remote_tag.to_string();
        self.id = Self::generate_id(&self.call_id, &self.local_tag, &self.remote_tag);
    }
    
    pub fn update_remote_seq(&mut self, seq: u32) {
        self.remote_seq = seq;
    }
    
    pub fn update_local_seq(&mut self, seq: u32) {
        self.local_seq = seq;
    }
    
    pub fn set_route_set(&mut self, route_set: Vec<String>) {
        self.route_set = route_set;
    }
    
    pub fn add_route(&mut self, route: &str) {
        self.route_set.insert(0, route.to_string());
    }
    
    pub fn remove_route(&mut self, route: &str) {
        self.route_set.retain(|r| r != route);
    }
    
    pub fn confirm(&mut self) {
        self.state = DialogState::Confirmed;
    }
    
    pub fn terminate(&mut self) {
        self.state = DialogState::Terminated;
    }
    
    pub fn set_sdp_offer(&mut self, sdp: &str) {
        self.sdp_offer = Some(sdp.to_string());
    }
    
    pub fn set_sdp_answer(&mut self, sdp: &str) {
        self.sdp_answer = Some(sdp.to_string());
    }
    
    pub fn has_sdp_exchange(&self) -> bool {
        self.sdp_offer.is_some() && self.sdp_answer.is_some()
    }
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
    
    pub async fn create_uac(
        &self,
        call_id: &str,
        local_tag: &str,
        remote_tag: &str,
        local_uri: &str,
        remote_uri: &str,
        remote_target: &str,
        contact: &str,
    ) -> Dialog {
        let dialog = Dialog::new_uac(
            call_id,
            local_tag,
            remote_tag,
            local_uri,
            remote_uri,
            remote_target,
            contact,
        );
        let key = dialog.id.clone();
        self.dialogs.write().await.insert(key, dialog.clone());
        dialog
    }
    
    pub async fn create_uas(
        &self,
        call_id: &str,
        local_tag: &str,
        remote_tag: &str,
        local_uri: &str,
        remote_uri: &str,
        remote_target: &str,
        contact: &str,
        local_seq: u32,
    ) -> Dialog {
        let dialog = Dialog::new_uas(
            call_id,
            local_tag,
            remote_tag,
            local_uri,
            remote_uri,
            remote_target,
            contact,
            local_seq,
        );
        let key = dialog.id.clone();
        self.dialogs.write().await.insert(key, dialog.clone());
        dialog
    }
    
    pub async fn get(&self, call_id: &str, local_tag: &str) -> Option<Dialog> {
        let _key = format!("{}-{}-", call_id, local_tag);
        self.dialogs.read().await
            .values()
            .find(|d| d.call_id == call_id && d.local_tag == local_tag)
            .cloned()
    }
    
    pub async fn get_by_call_id(&self, call_id: &str) -> Vec<Dialog> {
        self.dialogs.read().await
            .values()
            .filter(|d| d.call_id == call_id)
            .cloned()
            .collect()
    }
    
    pub async fn get_dialog_for_request(&self, req: &SipRequest) -> Option<Dialog> {
        let call_id = req.call_id()?.to_string();
        let from_tag = req.from()
            .and_then(|s| s.tag.clone())
            .unwrap_or_default();
        
        self.get(&call_id, &from_tag).await
    }
    
    pub async fn update(&self, dialog: &Dialog) {
        self.dialogs.write().await.insert(dialog.id.clone(), dialog.clone());
    }
    
    pub async fn remove(&self, call_id: &str, local_tag: &str) {
        let _key = format!("{}-{}-", call_id, local_tag);
        if let Some(dialog) = self.dialogs.read().await.values()
            .find(|d| d.call_id == call_id && d.local_tag == local_tag)
            .cloned() {
            self.dialogs.write().await.remove(&dialog.id);
        }
    }
    
    pub async fn remove_by_call_id(&self, call_id: &str) {
        self.dialogs.write().await.retain(|_, d| d.call_id != call_id);
    }
    
    pub async fn confirm_dialog(&self, call_id: &str, local_tag: &str) -> Option<Dialog> {
        let mut guard = self.dialogs.write().await;
        let dialog = guard.values_mut()
            .find(|d| d.call_id == call_id && d.local_tag == local_tag)?;
        
        dialog.confirm();
        Some(dialog.clone())
    }
    
    pub async fn terminate_dialog(&self, call_id: &str, local_tag: &str) -> Option<Dialog> {
        let mut guard = self.dialogs.write().await;
        let dialog = guard.values_mut()
            .find(|d| d.call_id == call_id && d.local_tag == local_tag)?;
        
        dialog.terminate();
        Some(dialog.clone())
    }
    
    pub async fn terminate_all_for_call_id(&self, call_id: &str) {
        let mut guard = self.dialogs.write().await;
        for dialog in guard.values_mut() {
            if dialog.call_id == call_id {
                dialog.terminate();
            }
        }
    }
    
    pub async fn get_active_dialogs(&self) -> Vec<Dialog> {
        self.dialogs.read().await
            .values()
            .filter(|d| !d.is_terminated())
            .cloned()
            .collect()
    }
    
    pub async fn get_early_dialogs(&self) -> Vec<Dialog> {
        self.dialogs.read().await
            .values()
            .filter(|d| d.is_early())
            .cloned()
            .collect()
    }
    
    pub async fn get_confirmed_dialogs(&self) -> Vec<Dialog> {
        self.dialogs.read().await
            .values()
            .filter(|d| d.is_confirmed())
            .cloned()
            .collect()
    }
    
    pub async fn match_dialog(&self, req: &SipRequest) -> Option<Dialog> {
        let call_id = req.call_id()?;
        
        for dialog in self.dialogs.read().await.values() {
            if dialog.call_id == *call_id {
                let local_tag = dialog.local_tag.clone();
                let remote_tag = dialog.remote_tag.clone();
                
                if let Some(from) = req.from() {
                    if let Some(ref tag) = from.tag {
                        if tag == &remote_tag {
                            return Some(dialog.clone());
                        }
                    }
                }
                
                if let Some(to) = req.to() {
                    if let Some(ref tag) = to.tag {
                        if tag == &local_tag {
                            return Some(dialog.clone());
                        }
                    }
                }
            }
        }
        
        None
    }
    
    pub async fn has_dialog(&self, call_id: &str) -> bool {
        self.dialogs.read().await
            .values()
            .any(|d| d.call_id == call_id && !d.is_terminated())
    }
}

impl Default for DialogManager {
    fn default() -> Self {
        Self::new()
    }
}
