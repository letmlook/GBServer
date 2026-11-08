# Playback Recovery Durability Implementation Plan

> Execute incrementally with `superpowers:executing-plans`; tests must fail before implementation and each database change requires real PostgreSQL and MySQL verification.

**Goal:** Preserve enough information across process failure to safely expose and retry cleanup of interrupted GB28181 playback sessions.

**Architecture:** Write a durable recovery journal before side effects that may survive the process, advance it as the SIP dialog becomes known, and restore unresolved entries before accepting new playback allocations. Reserve every playback INFO/BYE CSeq durably before sending; never infer remote termination from a local UDP send. Keep records with incomplete or unsafe dialog information visible to administrators without sending an invalid request.

**Tech stack:** Rust, Axum, SQLx native SQLite/PostgreSQL/MySQL pools, Tokio, ZLM, SIP.

**Spec:** [2.0 roadmap](../../ROADMAP_2.0.md), [B46](../../OPEN_ISSUES.md).

## Constraints and review focus

- Startup schema upgrade must work for existing databases; initialization errors must not silently enable unsafe playback.
- A crash can occur before or after every database write, SIP send, ZLM request, and HTTP response.
- The dialog identity must include Call-ID, local/remote tags, the original addressing and transport; never generate replacement tags when retrying a restored dialog.
- Concurrent INFO/BYE and concurrent retry operations must never reuse CSeq or claim an unconfirmed device outcome as success.
- Restore or quarantine the original SSRC before allocating new playback sessions; prevent stale leases from releasing replacement allocations.
- TCP connections cannot be persisted as sockets; require a verified current route or retain an actionable pending record.
- No ZLM secret, credentials, or full SDP in the administrator response or logs.

## Task 1: Restore SSRC ownership

**Files:** `src/sip/gb28181/ssrc.rs`.

- [x] Add tests for restoring an exact, valid playback SSRC, collision under eight concurrent callers, invalid/domain/type mismatches and stale lease release.
- [x] Implement `SsrcManager::try_restore_lease(self: &Arc<Self>, ssrc: &str, device_id: &str, channel_id: &str, stream_type: &str) -> Option<SsrcLease>` using the same atomic map entry and allocation ownership token as normal leases.
- [x] Run `cargo test --locked --lib restore_lease_` and `cargo test --locked --lib sip::gb28181::ssrc::tests`.

Task 1 only supplies the allocator primitive; startup does not yet call it and restart recovery remains unimplemented.

## Task 2: Durable journal and schema

**Files:** new `src/db/playback_recovery.rs`; `src/db/mod.rs`, `src/lib.rs`, all three `database/init-*-2.7.4.sql`; new database integration tests.

- [ ] Define the journal transitions and unique stream/Call-ID constraints; store IDs, original network/transport and dialog fields, allocated SSRC, last reserved CSeq, ZLM node/app/stream, state and timestamps. Distinguish unknown dialog, active dialog, cleanup needed and locally cleaned but device outcome unconfirmed. Never store secrets or SDP.
- [ ] Test fresh schema and existing-schema upgrade, save/reload, atomic CSeq reservation, conflict, transaction rollback and invalid state transitions using each actual database. Run SQLite tests and the project's PostgreSQL/MySQL smoke commands.
- [ ] Add startup migration that propagates failure; implement runtime-pool dialect dispatch and parameterized SQL. Reservation must commit before the corresponding network send and never wrap.

**2026-11-08 partial implementation:** startup now creates the journal table and propagates failures; insert/read/delete and compare-and-set CSeq reservation are present. SQLite connection reopen and concurrent reservations against SQLite, PostgreSQL 16 and MySQL 8 passed. CSeq reservation now rejects incomplete and terminal dialogs and rechecks eligibility in the atomic update; the same tests passed on all three databases. This is storage infrastructure only: state transitions, timestamps, schema scripts, SIP write-ahead integration and restart restoration remain unfinished. The isolated three-dialect application smoke executed but reported CSV and unavailable RTSP source failures on all three dialects, plus PostgreSQL seed sequence collisions for role/user creation; its zero exit status does not mean all checks passed. The PostgreSQL seed-sequence conflict was subsequently traced to missing startup wiring and fixed; isolated real PostgreSQL tests covered the initial insert and synchronization concurrent with an INSERT. After the fix, all three dialect smoke runs still reported the CSV and RTSP source failures but no seed primary-key conflicts. Synchronization protects ordinary table writers; direct external nextval calls during synchronization remain unverified.

## Task 3: Write ahead and restore

**Files:** `src/handlers/playback.rs`, `src/sip/server.rs`, `src/sip/gb28181/invite_session.rs`, `src/lib.rs`, journal module and integration tests.

- [ ] Test cancellation and forced process termination at journal creation, INVITE send, dialog establishment, media ready and HTTP response; restart and inspect unresolved entries. Test write failure before each outgoing operation: no unsafe unjournaled operation proceeds.
- [ ] Journal ownership before allocating externally visible resources or sending INVITE; update the dialog before allowing controls. On startup reserve all unresolved SSRCs before SIP accepts new allocations; conflicts remain visible and non-retryable, not silently overwritten.
- [ ] Restore only validated complete dialogs; persist every CSeq reservation before INFO/BYE send. Incomplete dialogs and unavailable TCP routes remain actionable with an explicit reason; no invented tags, endpoint or success.
- [ ] Test simultaneous controls and retries, crash after reserving CSeq but before send, changed device address, SSRC collision, missing tags and database outage. Confirm the next sent CSeq exceeds every previously reserved value.

## Task 4: Administrator recovery and end-to-end acceptance

**Files:** `src/handlers/playback.rs`, system information page, OpenAPI, relevant e2e tests, `docs/OPEN_ISSUES.md`, `docs/STATUS.md`.

- [ ] Test admin/ordinary/anonymous access, pagination across a restart, pending and unconfirmed status, partial cleanup, retries, redaction, and error reporting without leaking credentials.
- [ ] Integrate durable entries into existing recovery list/retry; retain history when local send succeeds but remote acknowledgement is unknown, and make local RTP cleanup independent of the SIP outcome.
- [ ] Run `cargo fmt --check`, `cargo test --no-fail-fast --locked`, frontend build and targeted Playwright. Run real SQLite, PostgreSQL and MySQL migration and recovery tests; use a real ZLM and SIP simulator to kill/restart the backend and verify the exact dialog and increasing CSeq on the wire. Record the hardware and target-deployment gaps separately; do not claim real-device acceptance from a simulator.
