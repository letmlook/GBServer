use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};

static JT1078_MISSING: AtomicU64 = AtomicU64::new(0);
static JT1078_ACTIVE: AtomicUsize = AtomicUsize::new(0);
static SIP_DEVICES_ONLINE: AtomicUsize = AtomicUsize::new(0);
static SIP_INVITES_ACTIVE: AtomicUsize = AtomicUsize::new(0);
static STREAMS_ACTIVE: AtomicUsize = AtomicUsize::new(0);
// Phase 7.5: extended metric set
static CLUSTER_NODES_ACTIVE: AtomicUsize = AtomicUsize::new(0);
static RPC_MESSAGES_TOTAL: AtomicU64 = AtomicU64::new(0);
static WS_CLIENTS_CONNECTED: AtomicUsize = AtomicUsize::new(0);
static AUDIT_LOG_WRITES_TOTAL: AtomicU64 = AtomicU64::new(0);
static AUDIT_LOG_WRITES_FAILED: AtomicU64 = AtomicU64::new(0);
static REDIS_STATE_KEYS: AtomicI64 = AtomicI64::new(-1);

pub fn inc_missing(n: u64) {
    JT1078_MISSING.fetch_add(n, Ordering::Relaxed);
}

// Phase 7.5 setters
pub fn set_cluster_nodes_active(n: usize) { CLUSTER_NODES_ACTIVE.store(n, Ordering::Relaxed); }
pub fn inc_rpc_messages_total() { RPC_MESSAGES_TOTAL.fetch_add(1, Ordering::Relaxed); }
pub fn inc_ws_clients(delta: i64) {
    if delta >= 0 {
        WS_CLIENTS_CONNECTED.fetch_add(delta as usize, Ordering::Relaxed);
    } else {
        WS_CLIENTS_CONNECTED.fetch_sub((-delta) as usize, Ordering::Relaxed);
    }
}
pub fn inc_audit_log_writes_total() { AUDIT_LOG_WRITES_TOTAL.fetch_add(1, Ordering::Relaxed); }
pub fn inc_audit_log_writes_failed() { AUDIT_LOG_WRITES_FAILED.fetch_add(1, Ordering::Relaxed); }
pub fn set_redis_state_keys(n: i64) { REDIS_STATE_KEYS.store(n, Ordering::Relaxed); }

pub fn set_active_sessions(n: usize) {
    JT1078_ACTIVE.store(n, Ordering::Relaxed);
}

pub fn set_sip_devices_online(n: usize) {
    SIP_DEVICES_ONLINE.store(n, Ordering::Relaxed);
}

pub fn inc_sip_devices_online() {
    SIP_DEVICES_ONLINE.fetch_add(1, Ordering::Relaxed);
}

pub fn dec_sip_devices_online() {
    SIP_DEVICES_ONLINE.fetch_sub(1, Ordering::Relaxed);
}

pub fn set_sip_invites_active(n: usize) {
    SIP_INVITES_ACTIVE.store(n, Ordering::Relaxed);
}

pub fn set_active_streams(n: usize) {
    STREAMS_ACTIVE.store(n, Ordering::Relaxed);
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
    let devices = SIP_DEVICES_ONLINE.load(Ordering::Relaxed);
    s.push_str("# HELP sip_devices_online Number of online SIP devices\n");
    s.push_str("# TYPE sip_devices_online gauge\n");
    s.push_str(&format!("sip_devices_online {}\n", devices));
    let invites = SIP_INVITES_ACTIVE.load(Ordering::Relaxed);
    s.push_str("# HELP sip_invites_active Number of active SIP invite sessions\n");
    s.push_str("# TYPE sip_invites_active gauge\n");
    s.push_str(&format!("sip_invites_active {}\n", invites));
    let streams = STREAMS_ACTIVE.load(Ordering::Relaxed);
    s.push_str("# HELP streams_active Number of active media streams\n");
    s.push_str("# TYPE streams_active gauge\n");
    s.push_str(&format!("streams_active {}\n", streams));
    // Phase 7.5: extended metric set with HELP/TYPE annotations
    let cluster_nodes = CLUSTER_NODES_ACTIVE.load(Ordering::Relaxed);
    s.push_str("# HELP gb_cluster_nodes_active Number of active cluster nodes\n");
    s.push_str("# TYPE gb_cluster_nodes_active gauge\n");
    s.push_str(&format!("gb_cluster_nodes_active {}\n", cluster_nodes));
    let rpc_total = RPC_MESSAGES_TOTAL.load(Ordering::Relaxed);
    s.push_str("# HELP gb_rpc_messages_total Total RPC messages dispatched\n");
    s.push_str("# TYPE gb_rpc_messages_total counter\n");
    s.push_str(&format!("gb_rpc_messages_total {}\n", rpc_total));
    let ws_clients = WS_CLIENTS_CONNECTED.load(Ordering::Relaxed);
    s.push_str("# HELP gb_ws_clients_connected Number of connected WebSocket clients\n");
    s.push_str("# TYPE gb_ws_clients_connected gauge\n");
    s.push_str(&format!("gb_ws_clients_connected {}\n", ws_clients));
    let audit_total = AUDIT_LOG_WRITES_TOTAL.load(Ordering::Relaxed);
    s.push_str("# HELP gb_audit_log_writes_total Total audit log writes\n");
    s.push_str("# TYPE gb_audit_log_writes_total counter\n");
    s.push_str(&format!("gb_audit_log_writes_total {}\n", audit_total));
    let audit_failed = AUDIT_LOG_WRITES_FAILED.load(Ordering::Relaxed);
    s.push_str("# HELP gb_audit_log_writes_failed Total failed audit log writes\n");
    s.push_str("# TYPE gb_audit_log_writes_failed counter\n");
    s.push_str(&format!("gb_audit_log_writes_failed {}\n", audit_failed));
    let redis_keys = REDIS_STATE_KEYS.load(Ordering::Relaxed);
    s.push_str("# HELP gb_redis_state_keys Number of keys in Redis (or -1 if disabled)\n");
    s.push_str("# TYPE gb_redis_state_keys gauge\n");
    s.push_str(&format!("gb_redis_state_keys {}\n", redis_keys));
    // Build info
    s.push_str("# HELP gb_build_info Build information\n");
    s.push_str("# TYPE gb_build_info gauge\n");
    s.push_str(&format!("gb_build_info{{version=\"{}\"}} 1\n", env!("CARGO_PKG_VERSION")));
    s
}
