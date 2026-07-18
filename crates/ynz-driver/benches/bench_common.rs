// Shared plumbing for the ynz-driver calibration bench binaries
// (soa_calibration.rs, opt_pipeline_calibration.rs). Cargo benches with
// `harness = false` are independent binaries — `benches/` is not itself a
// crate — so each bench pulls this file in via
// `#[path = "bench_common.rs"] mod bench_common;` (the standard cross-bench
// sharing pattern). Only the mechanically-identical scaffolding lives here;
// each bench keeps its own gates (IR gate, dual-mode oracle) and workload
// generators, supplying the per-bench bits as parameters.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Workspace-target scratch dir for generated sources, binaries, and .ll files.
pub fn scratch_dir(subdir: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(subdir);
    fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Compile one generated workload with `ynz build --emit-ir` under one forced
/// mode; returns the binary path (`dir/stem`, with the .ll sibling the callers'
/// IR gates read). `force_var` is the harness-only override env var
/// (YNZ_SOA_FORCE / YNZ_OPT_FORCE), set on the CHILD process only (never
/// set_var — the bench binary itself may share the process with other
/// criterion machinery).
pub fn compile_workload(
    dir: &Path,
    stem: &str,
    source: &str,
    force_var: &str,
    mode: &str,
) -> PathBuf {
    let src = dir.join(format!("{stem}.ynz"));
    fs::write(&src, source).expect("write workload source");
    let out = Command::new(env!("CARGO_BIN_EXE_ynz"))
        .arg("build")
        .arg(&src)
        .arg("--emit-ir")
        .env(force_var, mode)
        .output()
        .expect("spawn ynz build");
    assert!(
        out.status.success(),
        "ynz build failed for {stem}: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    dir.join(stem)
}

/// Checksum gate: one run, stdout must be exactly the closed-form checksum.
/// `crash_hint` is the per-bench tail of the non-zero-exit panic — each
/// harness names the knob NOT to reach for ("lower the cap" vs "shrink the
/// workload"), because each has a different budget axis.
pub fn run_once_checked(bin: &Path, expected: i64, crash_hint: &str) -> String {
    let out = Command::new(bin).output().expect("spawn workload binary");
    assert!(
        out.status.success(),
        "workload {} exited non-zero ({:?}) — crash-class regression (row-439 stack bug \
         is fixed and locked by hot_loop_stack_stress.rs; investigate, don't {crash_hint})",
        bin.display(),
        out.status
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");
    assert_eq!(
        stdout.trim(),
        expected.to_string(),
        "checksum gate failed for {}",
        bin.display()
    );
    stdout
}
