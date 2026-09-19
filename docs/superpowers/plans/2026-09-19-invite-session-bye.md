# Invite Session BYE Precision Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure stopping a GB28181 live stream, playback, or download only terminates the intended SIP INVITE session.

**Architecture:** Keep `InviteSessionManager` as the single source of session identity. Handlers choose a `StreamType`; ZLM hooks choose the exact `zlm_stream_id`. The SIP server performs the existing dialog-correct BYE send only after the manager returns that precise session.

**Tech Stack:** Rust, Tokio, Axum, SIP/GB28181, ZLMediaKit hooks, cargo test.

**Spec:** `docs/OPEN_ISSUES.md` B6.

## Global Constraints

- Preserve `ApiResult<T>` response envelopes and existing public HTTP paths.
- Do not send a BYE for an unrelated stream type or already terminated session.
- Keep all SQL-dialect and deployment configuration behavior unchanged.
- Add tests before production code and observe each new test fail first.

## Review Focus

- A live-stop request must not select a same-channel playback or download session.
- A ZLM hook for one stream must not terminate a different stream on that channel.
- A terminated old session must not win over a current session.
- A newly discovered ZLM stream ID must be attached to the INVITE identified by its Call-ID, not an arbitrary same-channel session.
- Repeated sessions of one type must select the most recently created live session.

---

### Task 1: Add precise InviteSessionManager lookups

**Files:**
- Modify: `src/sip/gb28181/invite_session.rs:425-465`
- Test: `src/sip/server.rs:8250-8310`

**Interfaces:**
- Produces `set_zlm_stream_by_call_id(call_id, stream_id, app) -> bool`.
- Produces `get_latest_by_device_channel_type(device_id, channel_id, stream_type) -> Option<InviteSession>`.
- Produces `get_by_zlm_stream_id(stream_id) -> Option<InviteSession>`.

- [ ] **Step 1: Write the failing tests**

Add two Tokio tests using real `InviteSessionManager` instances. One creates `Play`, `Playback`, and `Download` sessions for literal `dev/ch` values, terminates the older live session, and asserts a typed live lookup returns the newer `play_new` Call-ID while a playback lookup returns `playback_1`. The other assigns literal stream IDs `live_1` and `playback_1` by Call-ID and asserts stream lookup returns the matching Call-ID only.

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test precise_invite_session --lib`

Expected: FAIL because the precise lookup methods do not exist.

- [ ] **Step 3: Implement the minimal manager methods**

Replace the ambiguous device/channel ZLM backfill with a Call-ID lookup. Filter lookups to non-terminated sessions and select the maximum `created_at` only after filtering by the requested `StreamType` or exact ZLM stream ID.

- [ ] **Step 4: Verify the tests pass**

Run: `cargo test precise_invite_session --lib`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/sip/gb28181/invite_session.rs src/sip/server.rs
git commit -m "fix(sip): identify invite sessions precisely"
```

### Task 2: Route all stop paths through precise identity

**Files:**
- Modify: `src/sip/server.rs:5790-5890,6850-6870`
- Modify: `src/handlers/play.rs`
- Modify: `src/handlers/playback.rs`
- Modify: `src/handlers/common_channel.rs`
- Modify: `src/zlm/hook.rs`
- Test: `src/sip/server.rs`

**Interfaces:**
- Consumes the three manager methods from Task 1.
- Produces `send_session_bye_by_type(device_id, channel_id, stream_type) -> Result<String>`.
- Produces `send_session_bye_by_stream_id(stream_id) -> Result<String>`.

- [ ] **Step 1: Write the failing test**

Add a test that creates same-channel live and playback sessions and asserts the live-stop selection resolves Call-ID `play_1`, never `playback_1`. Name it `precise_invite_session_live_stop_excludes_playback`.

- [ ] **Step 2: Verify the test fails**

Run: `cargo test precise_invite_session_live_stop_excludes_playback --lib`

Expected: FAIL because the typed stop selection has not been implemented.

- [ ] **Step 3: Implement the minimal routing change**

Extract the existing dialog-correct BYE sender into a private helper accepting an `InviteSession`. Make live handlers request `StreamType::Play`, playback handlers request `StreamType::Playback`, download handlers request `StreamType::Download`, and ZLM hooks use their supplied stream ID. Backfill ZLM stream IDs using the Call-ID returned by the INVITE operation.

- [ ] **Step 4: Verify focused behavior**

Run: `cargo test precise_invite_session --lib`

Expected: PASS.

- [ ] **Step 5: Format and run affected tests**

Run: `cargo fmt --check && cargo test --lib`

Expected: both commands succeed.

- [ ] **Step 6: Commit**

```bash
git add src/sip/server.rs src/handlers/play.rs src/handlers/playback.rs src/handlers/common_channel.rs src/zlm/hook.rs
git commit -m "fix(sip): stop the intended media session"
```

### Task 3: Update the issue record

**Files:**
- Modify: `docs/OPEN_ISSUES.md`

**Interfaces:**
- Consumes the verified B6 behavior from Tasks 1-2.

- [ ] **Step 1: Record the closure**

Remove B6 from the open-issues summary and replace its detailed section with a concise closure note naming typed and stream-ID lookup coverage.

- [ ] **Step 2: Verify documentation integrity**

Run: `git diff --check`

Expected: command succeeds with no whitespace errors.

- [ ] **Step 3: Commit**

```bash
git add docs/OPEN_ISSUES.md
git commit -m "docs: close precise session BYE issue"
```
