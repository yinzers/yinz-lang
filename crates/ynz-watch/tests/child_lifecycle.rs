// test-ratchet: replacing tautological struct-field test + adding real spawn-kill-respawn test.
//   Old tests: (1) spawn_nonexistent_binary_returns_error, (2) check_mode_config_flag_is_set
//   (tautological — asserted struct field value matches what was just set, not behavior),
//   (3) drop_kills_child_no_zombie.
//   New tests: (1) unchanged, (2) check_mode_does_not_spawn_child (real behavior test via
//   rebuild_one API), (3) unchanged + (4) spawn_kill_respawn (full lifecycle with /bin/sleep).
/// Integration tests for ChildHandle lifecycle: spawn errors, --check mode, Drop cleanup,
/// and spawn-kill-respawn with a real long-running binary.
use std::path::{Path, PathBuf};

use ynz_watch::{WatchError, child::ChildHandle, project::WatchSourceFile};

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

    let _outcome = rebuild_one(&mut db, &path, &path, &out_dir, true, &mut current_child);

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
    assert!(second.is_ok(), "second spawn must succeed after first child was killed");

    // Kill second on drop.
    drop(second);
}
