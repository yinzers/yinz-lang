use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

// Runtime library embedded at compile time — no external file dependency when distributing ynz.
static RUNTIME_LIB_BYTES: &[u8] = include_bytes!(env!("YNZ_RT_LIB_PATH"));

use ynz_codegen::codegen_query;
use ynz_diagnostics::{render, DiagnosticBucket, SourceSpan};
use ynz_parser::{CompilerDb, SourceFile};

use crate::load::{find_project_root, load_project, load_project_config, load_source};

/// Outcome of a `build` invocation.
pub struct BuildResult {
    /// Path to the produced binary, if compilation succeeded.
    pub binary: Option<PathBuf>,
    /// The rendered diagnostic output (may be empty if everything succeeded).
    pub stderr_output: String,
    /// `true` if there were no errors.
    pub success: bool,
}

/// Compile `source_path` (file or project root) to a native binary.
///
/// If `source_path` is a directory or a file inside a project with `yinz.toml`,
/// loads all `.ynz` files under `src/` and links them together. Otherwise
/// treats `source_path` as a single-file project (M1-compatible path).
pub fn build(source_path: &Path) -> BuildResult {
    // Detect project root.
    let project_root = if source_path.is_dir() {
        Some(source_path.to_path_buf())
    } else {
        find_project_root(source_path)
    };

    if let Some(root) = project_root {
        return build_project(&root);
    }

    build_single_file(source_path)
}

/// Build a multi-file project rooted at `root`.
fn build_project(root: &Path) -> BuildResult {
    let mut diags = DiagnosticBucket::new();
    let _config = load_project_config(root, &mut diags);
    let sources = load_project(root, &mut diags);

    if !diags
        .iter()
        .filter(|d| d.severity == ynz_diagnostics::Severity::Error)
        .collect::<Vec<_>>()
        .is_empty()
    {
        return build_failed_diags(diags, &std::collections::HashMap::new());
    }

    if sources.is_empty() {
        diags.push(ynz_diagnostics::Diagnostic::error(
            SourceSpan::new(root.display().to_string(), 0, 0),
            "No `.ynz` source files found under `src/`.",
            "Add at least one `.ynz` file to `src/`, starting with `src/entrypoint.ynz`.",
            "Every Yinz project needs at least one source file.",
        ));
        return build_failed_diags(diags, &std::collections::HashMap::new());
    }

    // Parse all files and collect object bytes.
    let db = CompilerDb::default();
    let mut object_files: Vec<PathBuf> = Vec::new();
    let mut all_source_texts: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut had_error = false;

    for entry in &sources {
        all_source_texts.insert(entry.path.display().to_string(), entry.text.clone());
        let sf = SourceFile::new(&db, entry.path.display().to_string(), entry.text.clone());
        let codegen_out = codegen_query(&db, sf);

        let has_error = codegen_out
            .diagnostics
            .iter()
            .any(|d| d.severity == ynz_diagnostics::Severity::Error);
        for d in &codegen_out.diagnostics {
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

        let obj_path = entry.path.with_extension("o");
        if let Err(e) = std::fs::write(&obj_path, object_bytes) {
            diags.push(ynz_diagnostics::Diagnostic::error(
                SourceSpan::new(entry.path.display().to_string(), 0, 0),
                format!("Could not write object file `{}`: {e}", obj_path.display()),
                "Check that you have write permission in the directory.",
                "The compiler writes a temporary object file while linking.",
            ));
            had_error = true;
        } else {
            object_files.push(obj_path);
        }
    }

    if had_error {
        for obj in &object_files {
            let _ = std::fs::remove_file(obj);
        }
        return build_failed_diags(diags, &all_source_texts);
    }

    // Determine binary output path (next to yinz.toml or first source).
    let binary_path = root.join("bin").with_extension("");

    let result = link_objects(
        &object_files,
        &binary_path,
        &mut diags,
        root.display().to_string(),
    );
    for obj in &object_files {
        let _ = std::fs::remove_file(obj);
    }

    match result {
        Ok(()) => {
            let warnings: Vec<_> = diags
                .iter()
                .filter(|d| d.severity != ynz_diagnostics::Severity::Error)
                .cloned()
                .collect();
            let stderr_output = if warnings.is_empty() {
                String::new()
            } else {
                let mut bucket = DiagnosticBucket::new();
                for w in warnings {
                    bucket.push(w);
                }
                render(&bucket, &all_source_texts, false)
            };
            BuildResult {
                binary: Some(binary_path),
                stderr_output,
                success: true,
            }
        }
        Err(()) => build_failed_diags(diags, &all_source_texts),
    }
}

/// Link object files together with the runtime library.
fn link_objects(
    objects: &[PathBuf],
    binary_path: &Path,
    diags: &mut DiagnosticBucket,
    file_name: String,
) -> Result<(), ()> {
    let rt_lib_tmp = std::env::temp_dir().join(format!("libynz_runtime_{}.a", std::process::id()));
    if let Err(e) = std::fs::write(&rt_lib_tmp, RUNTIME_LIB_BYTES) {
        diags.push(ynz_diagnostics::Diagnostic::error(
            SourceSpan::new(file_name, 0, 0),
            format!("Failed to extract runtime library to temp dir: {e}"),
            "Check that your temp directory is writable.",
            "ynz extracts its bundled runtime library to a temp file during linking.",
        ));
        return Err(());
    }

    let Some(linker) = find_linker() else {
        let _ = std::fs::remove_file(&rt_lib_tmp);
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
    cmd.arg(&rt_lib_tmp)
        .arg("-no-pie")
        .arg("-o")
        .arg(binary_path);
    let cc_result = cmd.output();
    let _ = std::fs::remove_file(&rt_lib_tmp);

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
) -> BuildResult {
    let stderr_output = render(&diags, sources, false);
    BuildResult {
        binary: None,
        stderr_output,
        success: false,
    }
}

/// Compile a single `.ynz` file to a native binary (M1-compatible path).
fn build_single_file(source_path: &Path) -> BuildResult {
    let mut diags = DiagnosticBucket::new();

    // 1. Load source.
    let source_text = match load_source(source_path, &mut diags) {
        Some(t) => t,
        None => {
            return build_failed(diags, source_path);
        }
    };

    let file_name = source_path.display().to_string();

    // 2. Run the salsa pipeline.
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, file_name.clone(), source_text);
    let codegen_out = codegen_query(&db, sf);

    if codegen_out
        .diagnostics
        .iter()
        .any(|d| d.severity == ynz_diagnostics::Severity::Error)
    {
        let mut bucket = DiagnosticBucket::new();
        for d in &codegen_out.diagnostics {
            bucket.push(d.clone());
        }
        return build_failed(bucket, source_path);
    }

    let object_bytes = &codegen_out.artifact.object_bytes;
    if object_bytes.is_empty() {
        diags.push(ynz_diagnostics::Diagnostic::error(
            SourceSpan::new(&file_name, 0, 0),
            "Codegen produced no output.",
            "This is a compiler bug. Please report it.",
            "The compiler should always emit object bytes for a valid program.",
        ));
        return build_failed(diags, source_path);
    }

    // 3. Write object file to a temp path.
    let obj_path = source_path.with_extension("o");
    if let Err(e) = std::fs::write(&obj_path, object_bytes) {
        diags.push(ynz_diagnostics::Diagnostic::error(
            SourceSpan::new(&file_name, 0, 0),
            format!("Could not write object file `{}`: {e}", obj_path.display()),
            "Check that you have write permission in the directory.",
            "The compiler writes a temporary object file while linking.",
        ));
        return build_failed(diags, source_path);
    }

    // 4. Link with system C compiler, including the Yinz runtime library.
    //
    // The runtime library is embedded in the ynz binary at compile time (RUNTIME_LIB_BYTES).
    // We extract it to a temp file for the linker invocation, then delete it immediately after.
    // This means ynz is fully self-contained — no libynz_runtime.a on the target machine needed.
    let rt_lib_tmp = std::env::temp_dir().join(format!("libynz_runtime_{}.a", std::process::id()));
    if let Err(e) = std::fs::write(&rt_lib_tmp, RUNTIME_LIB_BYTES) {
        diags.push(ynz_diagnostics::Diagnostic::error(
            SourceSpan::new(&file_name, 0, 0),
            format!("Failed to extract runtime library to temp dir: {e}"),
            "Check that your temp directory is writable.",
            "ynz extracts its bundled runtime library to a temp file during linking.",
        ));
        return build_failed(diags, source_path);
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
        return build_failed(diags, source_path);
    };

    let binary_path = source_path.with_extension("");
    let cc_result = Command::new(linker)
        .arg(&obj_path)
        .arg(&rt_lib_tmp)
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

    // Clean up temp files regardless of link outcome.
    let _ = std::fs::remove_file(&obj_path);
    let _ = std::fs::remove_file(&rt_lib_tmp);

    match cc_result {
        Err(e) => {
            diags.push(ynz_diagnostics::Diagnostic::error(
                SourceSpan::new(&file_name, 0, 0),
                format!("The linker (`{linker}`) failed to start: {e}"),
                "This is unexpected. Check your PATH and C toolchain installation.",
                "The compiler invokes the system linker to produce the final binary.",
            ));
            build_failed(diags, source_path)
        }
        Ok(output) if !output.status.success() => {
            let linker_stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            diags.push(ynz_diagnostics::Diagnostic::error(
                SourceSpan::new(&file_name, 0, 0),
                "The linker failed.",
                "This is a compiler bug. Please report it with the output below.",
                format!("Linker stderr:\n{linker_stderr}"),
            ));
            build_failed(diags, source_path)
        }
        Ok(_) => {
            // Success — render any warnings so the user sees them even though
            // the build succeeded. Warnings are informational; they do not
            // change the exit code or prevent the binary from running.
            let warnings: Vec<_> = codegen_out
                .diagnostics
                .iter()
                .filter(|d| d.severity != ynz_diagnostics::Severity::Error)
                .cloned()
                .collect();
            let stderr_output = if warnings.is_empty() {
                String::new()
            } else {
                let mut bucket = DiagnosticBucket::new();
                for w in warnings {
                    bucket.push(w);
                }
                let sources = std::fs::read_to_string(source_path)
                    .map(|text| std::collections::HashMap::from([(file_name.clone(), text)]))
                    .unwrap_or_default();
                render(&bucket, &sources, false)
            };
            BuildResult {
                binary: Some(binary_path),
                stderr_output,
                success: true,
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

fn build_failed(diags: DiagnosticBucket, source_path: &Path) -> BuildResult {
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
    }
}
