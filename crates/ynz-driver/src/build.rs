use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use ynz_codegen::codegen_query;
use ynz_diagnostics::{render, DiagnosticBucket, SourceSpan};
use ynz_parser::{CompilerDb, SourceFile};

use crate::load::load_source;

/// Outcome of a `build` invocation.
pub struct BuildResult {
    /// Path to the produced binary, if compilation succeeded.
    pub binary: Option<PathBuf>,
    /// The rendered diagnostic output (may be empty if everything succeeded).
    pub stderr_output: String,
    /// `true` if there were no errors.
    pub success: bool,
}

/// Compile `source_path` to a native binary next to the source file.
///
/// Returns a `BuildResult` describing the outcome. The caller decides how to
/// render it — the driver's `main` calls `eprintln!(stderr_output)` and
/// exits with the appropriate code.
pub fn build(source_path: &Path) -> BuildResult {
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
    // YNZ_RT_LIB_DIR and YNZ_RT_LIB_NAME are emitted by crates/ynz-driver/build.rs
    // at compile time and resolve to the target/{profile}/ directory where cargo
    // places the ynz-runtime staticlib.
    let rt_lib_dir = env!("YNZ_RT_LIB_DIR");
    let rt_lib_name = env!("YNZ_RT_LIB_NAME");

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
        .arg(format!("-L{rt_lib_dir}"))
        .arg(format!("-l{rt_lib_name}"))
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

    // Clean up object file regardless of link outcome.
    let _ = std::fs::remove_file(&obj_path);

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
            let warnings: Vec<_> = codegen_out.diagnostics.iter()
                .filter(|d| d.severity != ynz_diagnostics::Severity::Error)
                .cloned()
                .collect();
            let stderr_output = if warnings.is_empty() {
                String::new()
            } else {
                let mut bucket = DiagnosticBucket::new();
                for w in warnings { bucket.push(w); }
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
