use dashmap::DashMap;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Clone)]
pub struct SsrcAllocation {
    pub ssrc: String,
    pub device_id: String,
    pub channel_id: String,
    pub stream_type: String,
}

pub struct SsrcManager {
    domain_part: String,
    counter: AtomicU32,
    allocations: DashMap<String, SsrcAllocation>,
}

impl SsrcManager {
    pub fn new(sip_device_id: &str) -> Self {
        Self {
            domain_part: crate::sip::server::ssrc_domain_part(sip_device_id),
            counter: AtomicU32::new(1),
            allocations: DashMap::new(),
        }
    }

    /// 分配一个未占用的 SSRC；对应类型的编号耗尽或超出 RTP u32 范围时返回 None。
    /// 调用方负责在会话结束后释放，不能把未释放的编号分配给其他会话。
    ///
    /// 国标 SSRC 为 **10 位十进制**：1 位类型位 + 5 位域标识 + 4 位流序号。
    /// 此前这里是 `format!("0{}{:04}0", prefix9, seq)` —— 前缀 9 位再加 4 位
    /// 序号加尾随 0，得到的是 **15 位**字符串，写进 INVITE 的 `y=` 即为非法值
    /// （单元测试还把 15 位当成期望值固化了下来）。
    ///
    /// 域标识取 SIP 域标识的第 4~8 位
    /// （`sipDomain.substring(3, 8)`），`3402000000` → `20000`。
    /// 早期实现取 SIP 设备号前 5 位（`34020`）：类型位为 `4`（对讲/广播）时
    /// 得到 `4340200001` = 4,340,200,001 > `u32::MAX`，RTP 头装不下，
    /// 平台只能回落到另一个 SSRC，与 `y=` 不一致。
    pub fn try_allocate(
        &self,
        device_id: &str,
        channel_id: &str,
        stream_type: &str,
    ) -> Option<String> {
        // 类型位：0 实时 / 1 回放 / 2 下载 / 4 广播(含对讲)
        let type_digit = match stream_type.to_ascii_lowercase().as_str() {
            "playback" | "history" | "play_back" => '1',
            "download" => '2',
            "broadcast" | "talk" | "audio" => '4',
            _ => '0',
        };
        let start = self.counter.fetch_add(1, Ordering::Relaxed) % 10000;
        for offset in 0..10000 {
            let seq = (start + offset) % 10000;
            let ssrc = format!("{}{}{:04}", type_digit, self.domain_part, seq);
            if ssrc.parse::<u32>().is_err() {
                return None;
            }
            if let dashmap::mapref::entry::Entry::Vacant(entry) =
                self.allocations.entry(ssrc.clone())
            {
                entry.insert(SsrcAllocation {
                    ssrc: ssrc.clone(),
                    device_id: device_id.to_string(),
                    channel_id: channel_id.to_string(),
                    stream_type: stream_type.to_string(),
                });
                return Some(ssrc);
            }
        }
        None
    }

    pub fn release(&self, ssrc: &str) -> Option<SsrcAllocation> {
        self.allocations.remove(ssrc).map(|(_, v)| v)
    }

    pub fn validate(
        &self,
        ssrc: &str,
        expected_device_id: &str,
        expected_channel_id: &str,
    ) -> bool {
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
        let to_remove: Vec<String> = self
            .allocations
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
    fn allocation_never_overwrites_active_ssrc_and_reuses_released_slot() {
        let manager = SsrcManager::new("34020000002000000001");
        let first = manager
            .try_allocate("first", "channel", "playback")
            .unwrap();
        for _ in 1..10000 {
            manager
                .try_allocate("other", "channel", "playback")
                .unwrap();
        }
        assert!(manager
            .try_allocate("overflow", "channel", "playback")
            .is_none());
        assert_eq!(manager.get(&first).unwrap().device_id, "first");
        manager.release(&first).unwrap();
        let results = std::thread::scope(|scope| {
            (0..8)
                .map(|_| scope.spawn(|| manager.try_allocate("new", "channel", "playback")))
                .collect::<Vec<_>>()
                .into_iter()
                .filter_map(|thread| thread.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(results, [first]);
    }

    #[test]
    fn allocation_rejects_ssrc_outside_rtp_u32_range() {
        let manager = SsrcManager::new("1239999900");
        assert!(manager.try_allocate("device", "channel", "talk").is_none());
    }

    #[test]
    fn test_allocate_and_release() {
        let manager = SsrcManager::new("34020000002000000001");
        let ssrc = manager.try_allocate("device1", "channel1", "Play").unwrap();
        // 国标 SSRC 必须是 10 位十进制
        assert_eq!(ssrc.len(), 10, "SSRC 应为 10 位, 实际 {}", ssrc);
        assert!(
            ssrc.chars().all(|c| c.is_ascii_digit()),
            "SSRC 必须全是数字: {}",
            ssrc
        );
        assert!(ssrc.starts_with('0'), "实时流类型位应为 0: {}", ssrc);

        let alloc = manager.get(&ssrc).unwrap();
        assert_eq!(alloc.device_id, "device1");

        let released = manager.release(&ssrc).unwrap();
        assert_eq!(released.ssrc, ssrc);
        assert!(manager.get(&ssrc).is_none());
    }

    /// 类型位必须随业务类型变化，长度恒为 10。
    #[test]
    fn test_allocate_type_digit_and_width() {
        let manager = SsrcManager::new("34020000002000000001");
        let cases = [
            ("Play", '0'),
            ("playback", '1'),
            ("download", '2'),
            ("broadcast", '4'),
            ("talk", '4'),
        ];
        for (kind, expected) in cases {
            let ssrc = manager.try_allocate("d", "c", kind).unwrap();
            assert_eq!(ssrc.len(), 10, "{} -> {}", kind, ssrc);
            assert!(
                ssrc.starts_with(expected),
                "{} 的类型位应为 {}，实际 {}",
                kind,
                expected,
                ssrc
            );
        }
    }

    #[test]
    fn test_validate() {
        let manager = SsrcManager::new("34020000002000000001");
        let ssrc = manager.try_allocate("device1", "channel1", "Play").unwrap();
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
                m.try_allocate("dev", &format!("ch{}", i), "Play").unwrap()
            }));
        }

        let ssrcs: Vec<String> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let unique: std::collections::HashSet<&str> = ssrcs.iter().map(|s| s.as_str()).collect();
        assert_eq!(unique.len(), 10);
    }

    #[test]
    fn test_release_by_device() {
        let manager = SsrcManager::new("34020000002000000001");
        manager.try_allocate("device1", "ch1", "Play").unwrap();
        manager.try_allocate("device1", "ch2", "Play").unwrap();
        manager.try_allocate("device2", "ch3", "Play").unwrap();

        let released = manager.release_by_device("device1");
        assert_eq!(released.len(), 2);
        assert_eq!(manager.active_count(), 1);
    }
}
