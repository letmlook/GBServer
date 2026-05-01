use dashmap::DashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SsrcAllocation {
    pub ssrc: String,
    pub device_id: String,
    pub channel_id: String,
    pub stream_type: String,
}

pub struct SsrcManager {
    device_prefix: String,
    counter: AtomicU32,
    allocations: DashMap<String, SsrcAllocation>,
}

impl SsrcManager {
    pub fn new(sip_device_id: &str) -> Self {
        let prefix = if sip_device_id.len() >= 9 {
            &sip_device_id[..9]
        } else {
            sip_device_id
        };
        Self {
            device_prefix: prefix.to_string(),
            counter: AtomicU32::new(1),
            allocations: DashMap::new(),
        }
    }

    pub fn allocate(&self, device_id: &str, channel_id: &str, stream_type: &str) -> String {
        let seq = self.counter.fetch_add(1, Ordering::Relaxed);
        let ssrc = format!("0{}{:04}0", self.device_prefix, seq % 10000);

        self.allocations.insert(ssrc.clone(), SsrcAllocation {
            ssrc: ssrc.clone(),
            device_id: device_id.to_string(),
            channel_id: channel_id.to_string(),
            stream_type: stream_type.to_string(),
        });

        ssrc
    }

    pub fn release(&self, ssrc: &str) -> Option<SsrcAllocation> {
        self.allocations.remove(ssrc).map(|(_, v)| v)
    }

    pub fn validate(&self, ssrc: &str, expected_device_id: &str, expected_channel_id: &str) -> bool {
        if let Some(entry) = self.allocations.get(ssrc) {
            entry.device_id == expected_device_id && entry.channel_id == expected_channel_id
        } else {
            false
        }
    }

    pub fn get(&self, ssrc: &str) -> Option<SsrcAllocation> {
        self.allocations.get(ssrc).map(|v| v.clone())
    }

    pub fn active_count(&self) -> usize {
        self.allocations.len()
    }

    pub fn list_by_device(&self, device_id: &str) -> Vec<SsrcAllocation> {
        self.allocations
            .iter()
            .filter(|v| v.device_id == device_id)
            .map(|v| v.clone())
            .collect()
    }

    pub fn release_by_device(&self, device_id: &str) -> Vec<SsrcAllocation> {
        let to_remove: Vec<String> = self.allocations
            .iter()
            .filter(|v| v.device_id == device_id)
            .map(|v| v.ssrc.clone())
            .collect();

        let mut released = Vec::new();
        for ssrc in to_remove {
            if let Some((_, alloc)) = self.allocations.remove(&ssrc) {
                released.push(alloc);
            }
        }
        released
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_and_release() {
        let manager = SsrcManager::new("34020000002000000001");
        let ssrc = manager.allocate("device1", "channel1", "Play");
        assert!(ssrc.starts_with('0'));
        assert!(ssrc.ends_with('0'));
        assert_eq!(ssrc.len(), 15);

        let alloc = manager.get(&ssrc).unwrap();
        assert_eq!(alloc.device_id, "device1");

        let released = manager.release(&ssrc).unwrap();
        assert_eq!(released.ssrc, ssrc);
        assert!(manager.get(&ssrc).is_none());
    }

    #[test]
    fn test_validate() {
        let manager = SsrcManager::new("34020000002000000001");
        let ssrc = manager.allocate("device1", "channel1", "Play");
        assert!(manager.validate(&ssrc, "device1", "channel1"));
        assert!(!manager.validate(&ssrc, "device2", "channel1"));
    }

    #[test]
    fn test_concurrent_allocate_no_conflict() {
        use std::sync::Arc;
        let manager = Arc::new(SsrcManager::new("34020000002000000001"));
        let mut handles = vec![];

        for i in 0..10 {
            let m = manager.clone();
            handles.push(std::thread::spawn(move || {
                m.allocate("dev", &format!("ch{}", i), "Play")
            }));
        }

        let ssrcs: Vec<String> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let unique: std::collections::HashSet<&str> = ssrcs.iter().map(|s| s.as_str()).collect();
        assert_eq!(unique.len(), 10);
    }

    #[test]
    fn test_release_by_device() {
        let manager = SsrcManager::new("34020000002000000001");
        manager.allocate("device1", "ch1", "Play");
        manager.allocate("device1", "ch2", "Play");
        manager.allocate("device2", "ch3", "Play");

        let released = manager.release_by_device("device1");
        assert_eq!(released.len(), 2);
        assert_eq!(manager.active_count(), 1);
    }
}
