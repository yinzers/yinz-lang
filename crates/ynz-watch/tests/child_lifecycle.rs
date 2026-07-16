// test-ratchet: replacing tautological struct-field test + adding real spawn-kill-respawn test.
//   Old tests: (1) spawn_nonexistent_binary_returns_error, (2) check_mode_config_flag_is_set
//   (tautological — asserted struct field value matches what was just set, not behavior),
//   (3) drop_kills_child_no_zombie.
//   New tests: (1) unchanged, (2) check_mode_does_not_spawn_child (real behavior test via
//   rebuild_one API), (3) unchanged + (4) spawn_kill_respawn (full lifecycle with /bin/sleep).
/// Integration tests for ChildHandle lifecycle: spawn errors, --check mode, Drop cleanup,
/// and spawn-kill-respawn with a real long-running binary.
use std::path::{Path, PathBuf};

use ynz_watch::{child::ChildHandle, project::WatchSourceFile, WatchError};

// WHY: ChildHandle::spawn must return WatchError::ChildSpawnFailed when the binary doesn't
//      exist. If this returns Ok (or panics), the watch event loop crashes with no diagnostic
//      instead of recovering gracefully and printing a WHAT/WHAT-INSTEAD/WHY message.
//      Do NOT change to expect()/unwrap().
#[test]
fn spawn_nonexistent_binary_returns_error() {
    let result = ChildHandle::spawn(Path::new("/tmp/ynz-watch-test-nonexistent-binary-xyz"));
    assert!(
        matches!(result, Err(WatchError::ChildSpawnFailed { .. })),
        "spawn must return WatchError::ChildSpawnFailed for missing binary"
    );
}

// WHY: rebuild_one with check_only=true must never populate current_child. If it does,
//      CI pipelines using --check to gate builds may accidentally spawn the program,
//      breaking non-interactive environments and producing unpredictable output.
//      Asserts the rebuild_one check_only=true path leaves Option<ChildHandle> as None.
#[test]
fn check_mode_does_not_spawn_child() {
    use ynz_watch::db::init_db;
    use ynz_watch::rebuild::rebuild_one;

    let valid_src = "function entrypoint() -> nothing { print(`ok`) }\n";
    let path = PathBuf::from("/tmp/ynz_watch_check_mode_test.ynz");
    let mut db = init_db(&[WatchSourceFile {
        path: path.clone(),
        text: valid_src.to_string(),
    }]);

    let out_dir = std::env::temp_dir().join("ynz-watch-check-mode-test");
    let _ = std::fs::create_dir_all(&out_dir);

    let mut current_child: Option<ChildHandle> = None;

    let _outcome = rebuild_one(
        &mut db,
        &path,
        &path,
        &out_dir,
        true,
        &mut current_child,
        false,
    );

    assert!(
        current_child.is_none(),
        "check_only=true must NOT spawn a child process; current_child must remain None"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

// WHY: ChildHandle's Drop impl must kill the child unconditionally to prevent zombie
//      processes when the watch daemon exits unexpectedly (panic, signal, early return).
//      Without this, every watch restart leaves a dead child on the process table.
#[test]
fn drop_kills_child_no_zombie() {
    let true_path = Path::new("/bin/true");
    if !true_path.exists() {
        eprintln!("Skipping drop_kills_child_no_zombie: /bin/true not found");
        return;
    }

    let handle = ChildHandle::spawn(true_path);
    assert!(handle.is_ok(), "spawning /bin/true should succeed");
    drop(handle); // Must not panic; child must be reaped by Drop.
}

// WHY: the spawn-kill-respawn cycle is the critical path for every rebuild — old child must
//      die (whole process group, via SIGTERM) before the new one starts. If kill_gracefully
//      doesn't work, old children accumulate as zombies and new spawns run concurrently with
//      old ones, causing terminal interleaving and resource exhaustion.
//      Uses /bin/sleep 10 as the long-running test binary.
#[test]
fn spawn_kill_respawn_via_child_handle() {
    let sleep_path = Path::new("/bin/sleep");
    if !sleep_path.exists() {
        eprintln!("Skipping spawn_kill_respawn: /bin/sleep not found");
        return;
    }

    // Build a Command that spawns `sleep 10` via ChildHandle.
    let mut first = match ChildHandle::spawn_with_args(sleep_path, &["10"]) {
        Ok(c) => c,
        Err(e) => panic!("Failed to spawn first child: {e}"),
    };

    // Child should be alive.
    assert!(
        first.try_wait().is_none(),
        "child should still be running after spawn"
    );

    // Kill it gracefully (50ms grace so the test is fast).
    first.kill_gracefully(50);

    // Now spawn a second one — must succeed (no zombie from first).
    let second = ChildHandle::spawn_with_args(sleep_path, &["10"]);
    assert!(
        second.is_ok(),
        "second spawn must succeed after first child was killed"
    );

    // Kill second on drop.
    drop(second);
}

// F6 (SCRATCH-audit-2026-07-11-non-concurrency.md): the SIGKILL escalation path in
// `kill_gracefully_impl` must hit the WHOLE process group (`killpg`), not just the
// direct child (`child.kill()`). Before the fix, a child that ignores SIGTERM AND has
// spawned a grandchild that also ignores SIGTERM would leak that grandchild once the
// grace period expired and escalation fired — the doc comment's group-kill invariant
// held for the SIGTERM half only, silently breaking for the SIGKILL half.
//
// This test spawns `/bin/sh -c '...'` (the direct child, `setsid`'d by ChildHandle) that
// itself traps SIGTERM AND backgrounds a subshell grandchild that ALSO traps SIGTERM —
// so SIGTERM alone (the graceful half) cannot kill either process; only a genuine
// process-group SIGKILL can. The grandchild writes its own PID to a file so the test
// can check liveness via `/proc/<pid>` after escalation.
#[test]
fn kill_gracefully_sigkill_escalation_reaches_grandchildren() {
    let sh_path = Path::new("/bin/sh");
    if !sh_path.exists() {
        eprintln!(
            "Skipping kill_gracefully_sigkill_escalation_reaches_grandchildren: /bin/sh not found"
        );
        return;
    }

    let pidfile = std::env::temp_dir().join(format!(
        "ynz-watch-f6-grandchild-pid-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&pidfile);

    // Outer shell (the direct child) traps SIGTERM so it survives the graceful half.
    // It backgrounds a subshell (the grandchild) that ALSO traps SIGTERM, shares the
    // SAME process group (no setsid call inside the script — exactly the
    // double-forked-but-same-group shape the ChildHandle doc comment describes). The
    // OUTER shell records `$!` (the PID of the job it just backgrounded) right after
    // backgrounding — `$!` is reliable across shells, unlike `$$` inside a subshell,
    // which some shells keep pinned to the ORIGINATING shell's PID rather than the
    // subshell's own PID.
    let script = format!(
        "trap '' TERM; (trap '' TERM; sleep 30) & echo $! > {pidfile}; wait",
        pidfile = pidfile.display()
    );

    let mut handle = match ChildHandle::spawn_with_args(sh_path, &["-c", &script]) {
        Ok(c) => c,
        Err(e) => panic!("Failed to spawn sh: {e}"),
    };

    // Wait for the grandchild to actually start and record its PID (bounded poll —
    // avoids a fixed sleep that's either flaky-fast or needlessly slow).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut grandchild_pid: Option<String> = None;
    while std::time::Instant::now() < deadline {
        if let Ok(contents) = std::fs::read_to_string(&pidfile) {
            let trimmed = contents.trim();
            if !trimmed.is_empty() {
                grandchild_pid = Some(trimmed.to_string());
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let grandchild_pid =
        grandchild_pid.expect("grandchild never wrote its PID — test setup broken");
    assert!(
        process_is_running(&grandchild_pid),
        "grandchild should be alive before kill_gracefully"
    );

    // Short grace period — both processes trap SIGTERM, so the grace period always
    // expires and forces the SIGKILL escalation path (the one F6 fixes).
    handle.kill_gracefully(100);

    // SIGKILL cannot be trapped — both the direct child (shell) and the grandchild
    // (which shares its process group) must be gone once killpg(SIGKILL) lands. This
    // container's PID 1 is a plain `bash`, not a reaping init, so a killed-but-orphaned
    // grandchild sits as a zombie ("defunct") rather than disappearing from /proc
    // entirely — `process_is_running` treats zombie as dead (it received the kill; it
    // is simply unreaped), which is what this test actually needs to assert.
    let gone_deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut grandchild_gone = false;
    while std::time::Instant::now() < gone_deadline {
        if !process_is_running(&grandchild_pid) {
            grandchild_gone = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let _ = std::fs::remove_file(&pidfile);

    assert!(
        grandchild_gone,
        "grandchild (pid {grandchild_pid}) survived the SIGKILL escalation — \
         kill_gracefully_impl's escalation must killpg(pgid, SIGKILL), not child.kill()"
    );
}

/// True when `pid` is a live, non-zombie process per `/proc/<pid>/status`. A zombie
/// ("defunct") process has already been killed and is only awaiting reap by its
/// parent — this container's PID 1 is a plain `bash`, not a reaping init, so an
/// orphaned killed grandchild lingers as a zombie rather than vanishing from `/proc`.
/// Treating a zombie as "dead" (not "running") is what the test actually needs.
fn process_is_running(pid: &str) -> bool {
    let status = match std::fs::read_to_string(format!("/proc/{pid}/status")) {
        Ok(s) => s,
        Err(_) => return false, // no /proc entry at all — definitely gone
    };
    for line in status.lines() {
        if let Some(state) = line.strip_prefix("State:") {
            return !state.trim_start().starts_with('Z');
        }
    }
    // No "State:" line found — treat conservatively as not confirmed-running.
    false
}
