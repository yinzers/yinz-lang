/// Integration tests for ChildHandle lifecycle: spawn errors, --check mode, and the
/// process-group kill of a real long-running child.
use std::path::{Path, PathBuf};

use ynz_watch::{child::ChildHandle, project::WatchSourceFile, WatchError};

// WHY: ChildHandle::spawn must return WatchError::ChildSpawnFailed when the binary doesn't
//      exist. If this returns Ok (or panics), the watch event loop crashes with no diagnostic
//      instead of recovering gracefully and printing a WHAT/WHAT-INSTEAD/WHY message.
//      Do NOT change to expect()/unwrap().
// bug: ChildHandle::spawn panics or returns Ok for a missing binary, so the watch loop crashes without a diagnostic | tier: Medium
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
// bug: rebuild_one with check_only=true spawns the program, so CI pipelines gating on --check run it | tier: Medium
#[test]
fn check_mode_does_not_spawn_child() {
    use ynz_watch::db::init_db;
    use ynz_watch::rebuild::rebuild_one;

    let valid_src = "function entrypoint() -> Nothing { print(`ok`) }\n";
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

// F6: the SIGKILL escalation path in `kill_gracefully_impl` must hit the WHOLE process
// group (`killpg`), not just the direct child (`child.kill()`). Before the fix, a child
// that ignores SIGTERM AND has spawned a grandchild that also ignores SIGTERM would
// leak that grandchild once the grace period expired and escalation fired — the doc
// comment's group-kill invariant held for the SIGTERM half only, silently breaking for
// the SIGKILL half.
//
// This test spawns `/bin/sh -c '...'` (the direct child, `setsid`'d by ChildHandle) that
// itself traps SIGTERM AND backgrounds a subshell grandchild that ALSO traps SIGTERM —
// so SIGTERM alone (the graceful half) cannot kill either process; only a genuine
// process-group SIGKILL can. The grandchild writes its own PID to a file so the test
// can check liveness via `/proc/<pid>` after escalation.
// bug: the SIGKILL escalation of kill_gracefully kills only the direct child, leaking SIGTERM-ignoring grandchildren | tier: Medium
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
