use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

// Runtime library embedded at compile time — no external file dependency when distributing ynz.
static RUNTIME_LIB_BYTES: &[u8] = include_bytes!(env!("YNZ_RT_LIB_PATH"));

use ynz_codegen::codegen_query;
use ynz_diagnostics::{render, DiagnosticBucket, SourceSpan};
use ynz_parser::{parse_query, CompilerDb, SourceFile};

use crate::load::{find_project_root, load_project, load_project_config, load_source};

/// Filter `diags` to non-error diagnostics and render them as a string.
///
/// Returns an empty string when there are no warnings or suggestions.
fn render_warnings(
    diags: &DiagnosticBucket,
    sources: &std::collections::HashMap<String, String>,
) -> String {
    let has_warnings = diags
        .iter()
        .any(|d| d.severity != ynz_diagnostics::Severity::Error);
    if !has_warnings {
        return String::new();
    }
    let mut bucket = DiagnosticBucket::new();
    for d in diags.iter() {
        if d.severity != ynz_diagnostics::Severity::Error {
            bucket.push(d.clone());
        }
    }
    render(&bucket, sources, false)
}

/// Whether a failed build was caused by user source errors or an infra problem.
pub enum FailureKind {
    /// The user's source code is wrong (parse/typeck/codegen errors).
    CompileError,
    /// Infrastructure failure: missing linker, can't write output, can't read source, etc.
    InfraError,
}

/// Outcome of a `build` invocation.
pub struct BuildResult {
    /// Path to the produced binary, if compilation succeeded.
    pub binary: Option<PathBuf>,
    /// The rendered diagnostic output (may be empty if everything succeeded).
    pub stderr_output: String,
    /// `true` if there were no errors.
    pub success: bool,
    /// Populated on failure — classifies the failure for exit-code purposes.
    pub failure_kind: Option<FailureKind>,
    /// LLVM IR text from the last compiled source file, if available.
    /// `None` on multi-file projects (IR is per-file; the driver surfaces the
    /// first file's IR for single-file builds only).
    pub ir_text: Option<String>,
}

/// Drop guard that removes `path` on drop, unless `.persist()` is called first.
///
/// Used to ensure temporary object files are cleaned up even on early return.
struct CleanupGuard {
    path: Option<PathBuf>,
}

impl CleanupGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    /// Consume the guard without running cleanup.
    fn persist(mut self) -> PathBuf {
        self.path.take().expect("already persisted")
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if let Some(p) = self.path.take() {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Compile `source_path` (file or project root) to a native binary.
///
/// If `source_path` is a directory or a file inside a project with `yinz.toml`,
/// loads all `.ynz` files under the project root and links them together.
/// Otherwise treats `source_path` as a single-file project (M1-compatible path).
///
/// When `output_dir` is `Some(dir)`, the binary is written into that directory
/// instead of next to the source. Used by `ynz run` to write into a tempdir.
pub fn build(source_path: &Path) -> BuildResult {
    build_with_output_dir(source_path, None)
}

/// Same as `build`, but the produced binary is placed in `output_dir` rather
/// than next to the source file. Used by `ynz run` to isolate the binary in a
/// per-invocation temp directory.
pub fn build_into(source_path: &Path, output_dir: &Path) -> BuildResult {
    build_with_output_dir(source_path, Some(output_dir))
}

fn build_with_output_dir(source_path: &Path, output_dir: Option<&Path>) -> BuildResult {
    // Canonicalize to absolute path so find_project_root never returns ""
    // when the user runs `ynz run relative/path.ynz` from the project root.
    let source_abs = std::fs::canonicalize(source_path)
        .unwrap_or_else(|_| source_path.to_path_buf());

    let project_root = if source_abs.is_dir() {
        Some(source_abs.clone())
    } else {
        find_project_root(&source_abs)
    };

    if let Some(root) = project_root {
        return build_project(&root, output_dir);
    }

    build_single_file(&source_abs, output_dir)
}

/// Build a multi-file project rooted at `root`.
fn build_project(root: &Path, output_dir: Option<&Path>) -> BuildResult {
    let mut diags = DiagnosticBucket::new();
    let _config = load_project_config(root, &mut diags);
    let sources = load_project(root, &mut diags);

    if diags.has_errors() {
        return build_failed_diags(
            diags,
            &std::collections::HashMap::new(),
            FailureKind::InfraError,
        );
    }

    if sources.is_empty() {
        diags.push(ynz_diagnostics::Diagnostic::error(
            SourceSpan::new(root.display().to_string(), 0, 0),
            "No `.ynz` source files found in the project root.",
            "Add at least one `.ynz` file to the project root, starting with `entrypoint.ynz`.",
            "Every Yinz project needs at least one source file.",
        ));
        return build_failed_diags(
            diags,
            &std::collections::HashMap::new(),
            FailureKind::InfraError,
        );
    }

    // Pass 1: create ALL SourceFile inputs and register them before any query runs.
    // This ensures cross-file import resolution can look up any project file by path
    // using CompilerDb::source_by_path — without creating duplicate salsa inputs.
    let mut db = CompilerDb::default();
    let mut all_source_texts: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut source_files: Vec<(SourceFile, &crate::load::SourceEntry)> = Vec::new();

    for entry in &sources {
        let path_str = entry.path.display().to_string();
        all_source_texts.insert(path_str.clone(), entry.text.clone());
        let sf = SourceFile::new(&db, path_str, entry.text.clone());
        db.register_source(sf);
        source_files.push((sf, entry));
    }

    // Pass 1.5: reject projects with more than one `entrypoint` function declaration.
    //
    // Each file's `entrypoint` would be renamed to the C `main` symbol at link time;
    // two files both declaring it produces a linker collision with a confusing C-level
    // error message.  Catching it here gives a teaching diagnostic instead.
    {
        let mut entrypoint_files: Vec<String> = Vec::new();
        for (sf, entry) in &source_files {
            let parse = parse_query(&db, *sf);
            let has_entrypoint = parse.module.items.iter().any(|item| {
                if let ynz_ast::nodes::Item::Function(f) = item {
                    f.name == "entrypoint"
                } else {
                    false
                }
            });
            if has_entrypoint {
                entrypoint_files.push(entry.path.display().to_string());
            }
        }
        if entrypoint_files.len() > 1 {
            let file_a = &entrypoint_files[0];
            let file_b = &entrypoint_files[1];
            diags.push(ynz_diagnostics::Diagnostic::error(
                SourceSpan::new(file_b.clone(), 0, 0),
                format!(
                    "Project has more than one `entrypoint` function — defined in `{file_a}` and `{file_b}`."
                ),
                format!(
                    "Yinz programs have exactly one entrypoint, declared in the file named by \
                     `entry` in `yinz.toml`. Remove or rename `entrypoint` in `{file_b}`."
                ),
                "The `entrypoint` function is the program's starting point. Allowing multiple \
                 would create ambiguity at link time (each gets emitted as the C `main` symbol) \
                 and at semantics (which one runs?). Yinz refuses both.",
            ));
            return build_failed_diags(diags, &all_source_texts, FailureKind::CompileError);
        }
    }

    // Pass 2: run codegen for each file now that all SourceFiles are registered.
    // Intermediates are written into a per-invocation temp dir to avoid races when
    // two `ynz build` invocations run on the same project concurrently.
    // Perf: TempDir creates one directory; all .o writes land there rather than next to sources.
    let obj_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            diags.push(ynz_diagnostics::Diagnostic::error(
                SourceSpan::new(root.display().to_string(), 0, 0),
                format!("Could not create a temporary directory for object files: {e}"),
                "Check that your system temp directory is writable.",
                "The compiler uses a temporary directory to hold intermediate files during linking.",
            ));
            return build_failed_diags(diags, &all_source_texts, FailureKind::InfraError);
        }
    };

    let mut object_files: Vec<PathBuf> = Vec::new();
    let mut had_error = false;

    for (sf, entry) in &source_files {
        let sf = *sf;
        let codegen_out = codegen_query(&db, sf);

        let has_error = codegen_out.diagnostics.has_errors();
        for d in codegen_out.diagnostics.iter() {
            diags.push(d.clone());
        }
        if has_error {
            had_error = true;
            continue;
        }

        let object_bytes = &codegen_out.artifact.object_bytes;
        if object_bytes.is_empty() {
            continue;
        }

        let stem = entry
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "module".to_string());
        let obj_path = obj_dir.path().join(format!("{stem}.o"));
        let guard = CleanupGuard::new(obj_path.clone());

        if let Err(e) = std::fs::write(&obj_path, object_bytes) {
            diags.push(ynz_diagnostics::Diagnostic::error(
                SourceSpan::new(entry.path.display().to_string(), 0, 0),
                format!("Could not write object file `{}`: {e}", obj_path.display()),
                "Check that your temp directory is writable.",
                "The compiler writes a temporary object file while linking.",
            ));
            had_error = true;
            // guard drops here — cleans up the partial .o
        } else {
            object_files.push(guard.persist());
        }
    }

    if had_error {
        return build_failed_diags(diags, &all_source_texts, FailureKind::CompileError);
    }

    // Determine binary output path.
    let binary_path = match output_dir {
        Some(dir) => dir.join("bin"),
        None => root.join("bin").with_extension(""),
    };

    let result = link_objects(
        &object_files,
        &binary_path,
        &mut diags,
        root.display().to_string(),
    );

    match result {
        Ok(()) => {
            let stderr_output = render_warnings(&diags, &all_source_texts);
            BuildResult {
                binary: Some(binary_path),
                stderr_output,
                success: true,
                failure_kind: None,
                ir_text: None,
            }
        }
        Err(()) => {
            // Clean up a partial binary if the linker wrote one before failing.
            let _ = std::fs::remove_file(&binary_path);
            build_failed_diags(diags, &all_source_texts, FailureKind::InfraError)
        }
    }
}

/// Link object files together with the runtime library.
fn link_objects(
    objects: &[PathBuf],
    binary_path: &Path,
    diags: &mut DiagnosticBucket,
    file_name: String,
) -> Result<(), ()> {
    // Extract the runtime library to a NamedTempFile with a random name.
    // NamedTempFile uses O_CREAT|O_EXCL internally — no predictable-PID race.
    // The variable must stay alive until the linker finishes (tempfile is deleted on drop).
    let rt_lib_tmp = match tempfile::NamedTempFile::new() {
        Ok(f) => f,
        Err(e) => {
            diags.push(ynz_diagnostics::Diagnostic::error(
                SourceSpan::new(file_name, 0, 0),
                format!("Failed to create temp file for runtime library: {e}"),
                "Check that your temp directory is writable.",
                "ynz extracts its bundled runtime library to a temp file during linking.",
            ));
            return Err(());
        }
    };

    if let Err(e) = std::fs::write(rt_lib_tmp.path(), RUNTIME_LIB_BYTES) {
        diags.push(ynz_diagnostics::Diagnostic::error(
            SourceSpan::new(file_name, 0, 0),
            format!("Failed to write runtime library to temp file: {e}"),
            "Check that your temp directory is writable.",
            "ynz extracts its bundled runtime library to a temp file during linking.",
        ));
        return Err(());
    }

    let Some(linker) = find_linker() else {
        diags.push(ynz_diagnostics::Diagnostic::error(
            SourceSpan::new(file_name, 0, 0),
            "No linker found (tried: clang-18, clang, cc, gcc, g++).",
            "Install clang: on Ubuntu: `sudo apt-get install clang-18`.",
            "`ynz build` links your program against the C runtime.",
        ));
        return Err(());
    };

    let mut cmd = Command::new(linker);
    for obj in objects {
        cmd.arg(obj);
    }
    cmd.arg(rt_lib_tmp.path())
        .arg("-no-pie")
        .arg("-o")
        .arg(binary_path);
    let cc_result = cmd.output();
    // rt_lib_tmp dropped here — linker has already finished reading it.

    match cc_result {
        Err(e) => {
            diags.push(ynz_diagnostics::Diagnostic::error(
                SourceSpan::new(file_name, 0, 0),
                format!("The linker (`{linker}`) failed to start: {e}"),
                "Check your PATH and C toolchain installation.",
                "The compiler invokes the system linker to produce the final binary.",
            ));
            Err(())
        }
        Ok(output) if !output.status.success() => {
            let linker_stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            diags.push(ynz_diagnostics::Diagnostic::error(
                SourceSpan::new(file_name, 0, 0),
                "The linker failed.",
                "This is a compiler bug. Please report it with the output below.",
                format!("Linker stderr:\n{linker_stderr}"),
            ));
            Err(())
        }
        Ok(_) => Ok(()),
    }
}

fn build_failed_diags(
    diags: DiagnosticBucket,
    sources: &std::collections::HashMap<String, String>,
    kind: FailureKind,
) -> BuildResult {
    let stderr_output = render(&diags, sources, false);
    BuildResult {
        binary: None,
        stderr_output,
        success: false,
        failure_kind: Some(kind),
        ir_text: None,
    }
}

/// Compile a single `.ynz` file to a native binary (M1-compatible path).
fn build_single_file(source_path: &Path, output_dir: Option<&Path>) -> BuildResult {
    let mut diags = DiagnosticBucket::new();

    // 1. Load source.
    let source_text = match load_source(source_path, &mut diags) {
        Some(t) => t,
        None => {
            return build_failed(diags, source_path, FailureKind::InfraError);
        }
    };

    let file_name = source_path.display().to_string();

    // 2. Run the salsa pipeline.
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, file_name.clone(), source_text);
    let codegen_out = codegen_query(&db, sf);

    if codegen_out.diagnostics.has_errors() {
        return build_failed(
            codegen_out.diagnostics.clone(),
            source_path,
            FailureKind::CompileError,
        );
    }

    let object_bytes = &codegen_out.artifact.object_bytes;
    if object_bytes.is_empty() {
        diags.push(ynz_diagnostics::Diagnostic::error(
            SourceSpan::new(&file_name, 0, 0),
            "Codegen produced no output.",
            "This is a compiler bug. Please report it.",
            "The compiler should always emit object bytes for a valid program.",
        ));
        return build_failed(diags, source_path, FailureKind::InfraError);
    }

    // 3. Write object file to a temp directory.
    // Perf: TempDir isolates the .o from the source tree, preventing concurrent-build races.
    let obj_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            diags.push(ynz_diagnostics::Diagnostic::error(
                SourceSpan::new(&file_name, 0, 0),
                format!("Could not create a temporary directory: {e}"),
                "Check that your system temp directory is writable.",
                "The compiler uses a temporary directory to hold the object file during linking.",
            ));
            return build_failed(diags, source_path, FailureKind::InfraError);
        }
    };

    let obj_path = obj_dir.path().join("source.o");
    let _obj_guard = CleanupGuard::new(obj_path.clone());

    if let Err(e) = std::fs::write(&obj_path, object_bytes) {
        diags.push(ynz_diagnostics::Diagnostic::error(
            SourceSpan::new(&file_name, 0, 0),
            format!("Could not write object file `{}`: {e}", obj_path.display()),
            "Check that your temp directory is writable.",
            "The compiler writes a temporary object file while linking.",
        ));
        return build_failed(diags, source_path, FailureKind::InfraError);
    }

    // 4. Link with system C compiler, including the Yinz runtime library.
    //
    // The runtime library is embedded in the ynz binary at compile time (RUNTIME_LIB_BYTES).
    // We extract it to a NamedTempFile for the linker invocation.
    // This means ynz is fully self-contained — no libynz_runtime.a on the target machine needed.
    let binary_path = match output_dir {
        Some(dir) => dir.join(
            source_path
                .file_stem()
                .unwrap_or(std::ffi::OsStr::new("out")),
        ),
        None => source_path.with_extension(""),
    };

    let rt_lib_tmp = match tempfile::NamedTempFile::new() {
        Ok(f) => f,
        Err(e) => {
            diags.push(ynz_diagnostics::Diagnostic::error(
                SourceSpan::new(&file_name, 0, 0),
                format!("Failed to create temp file for runtime library: {e}"),
                "Check that your temp directory is writable.",
                "ynz extracts its bundled runtime library to a temp file during linking.",
            ));
            return build_failed(diags, source_path, FailureKind::InfraError);
        }
    };

    if let Err(e) = std::fs::write(rt_lib_tmp.path(), RUNTIME_LIB_BYTES) {
        diags.push(ynz_diagnostics::Diagnostic::error(
            SourceSpan::new(&file_name, 0, 0),
            format!("Failed to write runtime library to temp file: {e}"),
            "Check that your temp directory is writable.",
            "ynz extracts its bundled runtime library to a temp file during linking.",
        ));
        return build_failed(diags, source_path, FailureKind::InfraError);
    }

    let Some(linker) = find_linker() else {
        diags.push(ynz_diagnostics::Diagnostic::error(
            SourceSpan::new(&file_name, 0, 0),
            "No linker found (tried: clang-18, clang, cc, gcc, g++).",
            "Install clang: on Ubuntu: `sudo apt-get install clang-18`; \
             on macOS: `xcode-select --install`.",
            "`ynz build` links your program against the C runtime. \
             `clang-18` is the lightest option — it ships with LLVM 18, which ynz already requires.",
        ));
        return build_failed(diags, source_path, FailureKind::InfraError);
    };

    let cc_result = Command::new(linker)
        .arg(&obj_path)
        .arg(rt_lib_tmp.path())
        // Modern Linux distros default `cc` to producing PIE executables, but
        // LLVM emits object files with absolute (non-PIC) relocations for
        // string-literal references. Linking those against a PIE template
        // fails with "R_X86_64_32 against .rodata.str1.1 can not be used when
        // making a PIE object." `-no-pie` tells cc to produce a position-
        // dependent executable, which matches what the codegen emits. Until
        // codegen is updated to emit PIC relocations (deferred to v0.2),
        // this is the load-bearing fix. The non-PIE binary is fully functional
        // — only loses ASLR, which Yinz programs don't currently need.
        .arg("-no-pie")
        .arg("-o")
        .arg(&binary_path)
        .output();
    // rt_lib_tmp dropped here — linker has already read it.

    match cc_result {
        Err(e) => {
            diags.push(ynz_diagnostics::Diagnostic::error(
                SourceSpan::new(&file_name, 0, 0),
                format!("The linker (`{linker}`) failed to start: {e}"),
                "This is unexpected. Check your PATH and C toolchain installation.",
                "The compiler invokes the system linker to produce the final binary.",
            ));
            build_failed(diags, source_path, FailureKind::InfraError)
        }
        Ok(output) if !output.status.success() => {
            let linker_stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            diags.push(ynz_diagnostics::Diagnostic::error(
                SourceSpan::new(&file_name, 0, 0),
                "The linker failed.",
                "This is a compiler bug. Please report it with the output below.",
                format!("Linker stderr:\n{linker_stderr}"),
            ));
            // Clean up partial binary if linker wrote one before failing.
            let _ = std::fs::remove_file(&binary_path);
            build_failed(diags, source_path, FailureKind::InfraError)
        }
        Ok(_) => {
            let sources = std::fs::read_to_string(source_path)
                .map(|text| std::collections::HashMap::from([(file_name.clone(), text)]))
                .unwrap_or_default();
            let stderr_output = render_warnings(&codegen_out.diagnostics, &sources);
            let ir_text = {
                let ir = &codegen_out.artifact.ir_text;
                if ir.is_empty() { None } else { Some(ir.clone()) }
            };
            BuildResult {
                binary: Some(binary_path),
                stderr_output,
                success: true,
                failure_kind: None,
                ir_text,
            }
        }
    }
}

fn find_linker() -> Option<&'static str> {
    // clang-18 first: ships with LLVM 18 (already required to run ynz),
    // so it's the lightest install path — a few MB vs build-essential's ~200 MB GCC stack.
    for candidate in ["clang-18", "clang", "cc", "gcc", "g++"] {
        let found = Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if found {
            return Some(candidate);
        }
    }
    None
}

fn build_failed(diags: DiagnosticBucket, source_path: &Path, kind: FailureKind) -> BuildResult {
    let file_name = source_path.display().to_string();
    // Build the source map for rendering (empty if the file couldn't be read).
    let sources = std::fs::read_to_string(source_path)
        .map(|text| std::collections::HashMap::from([(file_name.clone(), text)]))
        .unwrap_or_default();

    let stderr_output = render(&diags, &sources, false);
    BuildResult {
        binary: None,
        stderr_output,
        success: false,
        failure_kind: Some(kind),
        ir_text: None,
    }
}
