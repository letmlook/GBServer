use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

static JT1078_MISSING: AtomicU64 = AtomicU64::new(0);
static JT1078_ACTIVE: AtomicUsize = AtomicUsize::new(0);

pub fn inc_missing(n: u64) {
    JT1078_MISSING.fetch_add(n, Ordering::Relaxed);
}

pub fn set_active_sessions(n: usize) {
    JT1078_ACTIVE.store(n, Ordering::Relaxed);
}

pub fn gather() -> String {
    let mut s = String::new();
    let missing = JT1078_MISSING.load(Ordering::Relaxed);
    s.push_str("# HELP jt1078_missing_retransmit_total Total missing seq retransmit events\n");
    s.push_str("# TYPE jt1078_missing_retransmit_total counter\n");
    s.push_str(&format!("jt1078_missing_retransmit_total {}\n", missing));
    let active = JT1078_ACTIVE.load(Ordering::Relaxed);
    s.push_str("# HELP jt1078_active_sessions Number of active JT1078 sessions\n");
    s.push_str("# TYPE jt1078_active_sessions gauge\n");
    s.push_str(&format!("jt1078_active_sessions {}\n", active));
    s
}
