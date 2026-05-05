use once_cell::sync::Lazy;
use prometheus::{Encoder, TextEncoder, Registry, Counter, Gauge};

pub static REGISTRY: Lazy<Registry> = Lazy::new(|| Registry::new());

pub static JT1078_MISSING_COUNTER: Lazy<Counter> = Lazy::new(|| {
    let c = Counter::new("jt1078_missing_retransmit_total", "Total missing seq retransmit events").unwrap();
    REGISTRY.register(Box::new(c.clone())).ok();
    c
});

pub static JT1078_ACTIVE_SESSIONS: Lazy<Gauge> = Lazy::new(|| {
    let g = Gauge::new("jt1078_active_sessions", "Number of active JT1078 sessions").unwrap();
    REGISTRY.register(Box::new(g.clone())).ok();
    g
});

pub fn inc_missing(n: u64) {
    JT1078_MISSING_COUNTER.inc_by(n as f64);
}

pub fn set_active_sessions(n: usize) {
    JT1078_ACTIVE_SESSIONS.set(n as f64);
}

pub fn gather() -> String {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap_or_default()
}
