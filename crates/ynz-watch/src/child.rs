use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use crate::error::{Result, WatchError};

/// Handle to a running child process spawned by the watch daemon.
///
/// Drop kills the child unconditionally (via Drop impl), preventing zombie processes
/// on panics or early returns.
///
/// # Invariants
///
/// - The child runs in its own process group (Unix: setsid; Windows: CREATE_NEW_PROCESS_GROUP)
///   so SIGTERM/TerminateProcess hits the entire tree — catches double-forked grandchildren.
/// - stdin/stdout/stderr are INHERITED from the watch process's terminal.
///   Interactive programs (`terminal.readLine()` style) work transparently.
pub struct ChildHandle {
    inner: Option<Child>,
}

impl ChildHandle {
    /// Spawn the compiled binary as a child process in its own process group.
    ///
    /// # Flow
    ///
    /// 1. Build `Command` with inherited stdio.
    /// 2. Unix: install pre_exec hook calling `setsid()` to create new session + process group.
    ///    Windows: set `CREATE_NEW_PROCESS_GROUP` creation flag.
    /// 3. Spawn. Return a handle that owns the child.
    ///
    /// # Failure modes
    ///
    /// - Binary not found / no execute permission → `WatchError::ChildSpawnFailed`.
    /// - Pre-exec hook fails (setsid) → propagated as spawn error.
    ///
    /// # Side effects
    ///
    /// Spawns a child OS process. May write to the user's terminal via inherited stdout/stderr.
    ///
    /// Time: O(1) compute + OS process spawn overhead. Space: O(1).
    pub fn spawn(binary: &Path) -> Result<Self> {
        let mut cmd = Command::new(binary);
        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        setup_process_group(&mut cmd);

        let child = cmd.spawn().map_err(|e| WatchError::ChildSpawnFailed {
            binary: binary.to_path_buf(),
            reason: e.to_string(),
        })?;

        Ok(ChildHandle { inner: Some(child) })
    }

    /// Spawn a binary with explicit arguments (for tests and future interactive use).
    ///
    /// Identical to `spawn` but accepts a slice of string arguments to pass to the binary.
    pub fn spawn_with_args(binary: &Path, args: &[&str]) -> Result<Self> {
        let mut cmd = Command::new(binary);
        cmd.args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        setup_process_group(&mut cmd);

        let child = cmd.spawn().map_err(|e| WatchError::ChildSpawnFailed {
            binary: binary.to_path_buf(),
            reason: e.to_string(),
        })?;

        Ok(ChildHandle { inner: Some(child) })
    }

    /// Check whether the child has already exited (non-blocking).
    ///
    /// Returns `Some(exit_code)` if the child has exited; `None` if still running.
    ///
    /// Note: `kill_gracefully_impl` polls the inner `Child::try_wait` directly (for platform
    /// isolation reasons). This public method exposes the same check to external observers such
    /// as --json ChildExit event emitters that track child lifecycle outside the kill path.
    // CARVE-OUT: public method for external child-lifecycle observers (e.g. --json ChildExit emitter).
    //            kill_gracefully_impl polls the inner Child directly; this method is for callers
    //            that need to observe exit state without triggering a kill.
    #[allow(dead_code)]
    pub fn try_wait(&mut self) -> Option<i32> {
        let child = self.inner.as_mut()?;
        match child.try_wait() {
            Ok(Some(status)) => Some(status.code().unwrap_or(1)),
            Ok(None) => None,
            Err(_) => None,
        }
    }

    /// Gracefully terminate the child process and wait up to `grace_ms` for it to exit.
    ///
    /// # Flow (Unix)
    ///
    /// 1. `killpg(pgid, SIGTERM)` — hits the entire process group (catches double-forks).
    /// 2. Poll `child.try_wait()` every 50ms for `grace_ms` milliseconds.
    /// 3. If still alive: `child.kill()` (SIGKILL).
    ///
    /// # Flow (Windows)
    ///
    /// `child.kill()` sends TerminateProcess immediately — no grace period.
    /// Windows has no POSIX signal model; in-flight I/O in the child may be lost.
    /// This is a documented Windows limitation (see design/watch.md).
    ///
    /// # Failure modes
    ///
    /// - Signal delivery failure: logged to stderr; SIGKILL fallback still fires.
    /// - `try_wait` error: child assumed still alive; SIGKILL fires.
    ///
    /// Time: O(1) compute + up to `grace_ms` wall-clock wait. Space: O(1).
    pub fn kill_gracefully(&mut self, grace_ms: u64) {
        let child = match self.inner.as_mut() {
            Some(c) => c,
            None => return,
        };

        kill_gracefully_impl(child, grace_ms);

        // Clear the handle so Drop doesn't double-kill.
        self.inner = None;
    }
}

impl Drop for ChildHandle {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.inner {
            // Unconditional kill on drop to prevent zombie processes on panic/early-return.
            // Use 0ms grace — drop is a safety net, not a graceful shutdown path.
            kill_gracefully_impl(child, 0);
        }
    }
}

/// Platform-specific: configure the child command to run in its own process group.
#[cfg(unix)]
fn setup_process_group(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: setsid() has no invariants that unsafe code could violate; the only side
    // effect is creating a new session for the child, which is what we want.
    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid().map_err(|e| std::io::Error::other(e.to_string()))?;
            Ok(())
        });
    }
}

#[cfg(windows)]
fn setup_process_group(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    // CREATE_NEW_PROCESS_GROUP: child gets its own process group.
    // TerminateProcess on the parent (child.kill()) kills the entire group on Windows.
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
    cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(any(unix, windows)))]
fn setup_process_group(_cmd: &mut Command) {
    // No process-group isolation on unsupported platforms.
    // Child still runs; graceful-kill behavior degrades to best-effort.
}

/// Platform-specific graceful kill + wait.
#[cfg(unix)]
fn kill_gracefully_impl(child: &mut Child, grace_ms: u64) {
    use nix::{
        sys::signal::{killpg, Signal},
        unistd::Pid,
    };

    let pgid = Pid::from_raw(child.id() as i32);

    // SIGTERM the entire process group.
    if let Err(e) = killpg(pgid, Signal::SIGTERM) {
        // Child may have already exited; ESRCH is benign.
        if e != nix::errno::Errno::ESRCH {
            eprintln!("ynz watch: could not send SIGTERM to child group: {e}");
        }
    }

    if grace_ms > 0 {
        let deadline = Instant::now() + Duration::from_millis(grace_ms);
        while Instant::now() < deadline {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(_) => break,
            }
        }
    }

    // Grace period expired or not provided — SIGKILL.
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(not(unix))]
fn kill_gracefully_impl(child: &mut Child, _grace_ms: u64) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    // WHY: ChildHandle::spawn must return WatchError::ChildSpawnFailed when the binary
    //      doesn't exist. If this panics instead of returning an Err, the event loop
    //      crashes and leaves no useful diagnostic. Do NOT change to expect()/unwrap().
    #[test]
    fn spawn_nonexistent_binary_returns_error() {
        let result = ChildHandle::spawn(Path::new("/nonexistent/binary"));
        assert!(
            matches!(result, Err(WatchError::ChildSpawnFailed { .. })),
            "spawn should return WatchError::ChildSpawnFailed for missing binary"
        );
    }
}
