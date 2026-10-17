use std::collections::BTreeMap;
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HookKind {
    Play,
    Publish,
}

impl HookKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Play => "on_play",
            Self::Publish => "on_publish",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookSummary {
    pub hook_type: &'static str,
    pub received_at: DateTime<Utc>,
    pub media_server_id: Option<String>,
    pub app: Option<String>,
    pub stream: Option<String>,
    pub schema: Option<String>,
    pub success: bool,
}

fn safe_label(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty()
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return None;
    }
    Some(value.chars().take(128).collect())
}

impl HookSummary {
    pub fn new(
        kind: HookKind,
        media_server_id: Option<&str>,
        app: Option<&str>,
        stream: Option<&str>,
        schema: Option<&str>,
    ) -> Self {
        Self {
            hook_type: kind.as_str(),
            received_at: Utc::now(),
            media_server_id: safe_label(media_server_id),
            app: safe_label(app),
            stream: safe_label(stream),
            schema: safe_label(schema),
            success: true,
        }
    }
}

#[derive(Default)]
pub struct HookDiagnostics {
    latest: RwLock<BTreeMap<HookKind, HookSummary>>,
}

impl HookDiagnostics {
    pub fn record(&self, kind: HookKind, summary: HookSummary) {
        self.latest
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(kind, summary);
    }

    pub fn list(&self) -> Vec<HookSummary> {
        self.latest
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_diagnostics_keep_one_sanitized_entry_per_type() {
        let diagnostics = HookDiagnostics::default();
        diagnostics.record(
            HookKind::Play,
            HookSummary::new(
                HookKind::Play,
                Some("node-a"),
                Some("rtp"),
                Some("old"),
                Some("rtsp"),
            ),
        );
        diagnostics.record(
            HookKind::Play,
            HookSummary::new(
                HookKind::Play,
                Some("node-a"),
                Some("rtp"),
                Some("new"),
                Some("rtsp"),
            ),
        );
        diagnostics.record(
            HookKind::Publish,
            HookSummary::new(
                HookKind::Publish,
                Some("node-a"),
                Some("rtp"),
                Some("https://secret.example/?token=abc"),
                Some("rtsp"),
            ),
        );
        let entries = diagnostics.list();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].stream.as_deref(), Some("new"));
        assert_eq!(entries[1].stream, None);
        assert!(entries.iter().all(|entry| entry.hook_type != "unknown"));
    }

    #[test]
    fn hook_diagnostics_truncate_fields_to_128_characters() {
        let summary = HookSummary::new(HookKind::Play, Some(&"a".repeat(200)), None, None, None);
        assert_eq!(summary.media_server_id.unwrap().len(), 128);
    }
}
