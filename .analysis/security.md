# Security Analysis Report — Yinz Compiler

**Analyzed**: 2026-05-19
**Scope**: Entire `crates/**/*.rs` (compiler, driver, codegen, parser, typeck, runtime, diagnostics)
**Threat model**: A malicious `.ynz` source file (or project) compiled by `ynz build` / `ynz run` should not gain code execution beyond what normal compilation+linking permits, should not read arbitrary files outside the project, should not crash the compiler with attacker-friendly DoS, and should not corrupt object/binary outputs in surprising locations.
**Vulnerabilities Found**: 9 (Critical: 0, High: 3, Medium: 4, Low: 2)

---

## HIGH — Path traversal in import resolution (cross-project file disclosure via parse)

**Category**: Path traversal
**OWASP**: A01:2021 — Broken Access Control
**Location**: `crates/ynz-typeck/src/resolve_import.rs:59,69` (also reachable from `crates/ynz-parser/src/parser.rs:538` import-string parse)

**Issue**: `resolve_module_path` calls `base.join(format!("{module_str}.ynz"))` where `module_str` is the attacker-controlled import string from a `.ynz` file. `Path::join` does NOT canonicalize `..` segments. The parser only rejects strings that START with `./` or `../` (`parser.rs:547`, `parser.rs:566`); mid-string traversal like `foo/../../../some/other/dir/file` passes through unchecked.

After `candidate.exists()` is true, `std::fs::canonicalize(&candidate)` is called (`resolve_import.rs:63`), which dereferences `..` segments and produces a path that may be entirely outside the project root. The resolved file is then read (`resolve_import.rs:278`) and parsed as `.ynz` source for type-information extraction.

**Attack scenario**: A user runs `ynz build /tmp/evil-project/` containing `entrypoint.ynz`:
```yinz
import { x } from `foo/../../../home/victim/secret-project/internal`
```
The compiler reads, parses, and type-checks `/home/victim/secret-project/internal.ynz` (assuming it exists). Any parse errors or signature mismatches are surfaced in diagnostics that quote source content from the victim file (`resolve_import.rs:286-289, 302-310`). Effectively this is a cross-project information disclosure: source bytes from a victim's `.ynz` file can be exfiltrated to stderr/stdout, where the attacker may have arranged for them to be visible (e.g., CI log scraping, dev shell history capture).

**Impact**: Disclosure of `.ynz` file content outside the project root. Not a generic arbitrary-file read (the path must end with `.ynz` and contain parseable Yinz code OR the error path leaks `e: io::Error` text containing the canonicalized path), but enough to leak proprietary Yinz code on shared dev machines / CI runners.

**Vulnerable code** (`resolve_import.rs:51-72`):
```rust
pub fn resolve_module_path(importer_path: &str, module_str: &str) -> Option<PathBuf> {
    let importer = Path::new(importer_path);
    let importer_dir = importer.parent()?;
    let project_root = find_project_root(importer_dir);
    let base = project_root.as_deref().unwrap_or(importer_dir);
    let candidate = base.join(format!("{module_str}.ynz"));   // <-- no normalization
    if candidate.exists() {
        std::fs::canonicalize(&candidate).ok()                 // <-- escapes project root
    } ...
}
```

**Secure fix**: After canonicalization, verify the resolved path is a descendant of the project root before returning it. Reject any module_str containing `..` as a path component (use `Path::components()` to check, NOT a substring match — `..` can appear inside legitimate filenames). Pseudocode:
```rust
if module_str.split('/').any(|seg| seg == ".." || seg == ".") { return None; }
let canon = std::fs::canonicalize(&candidate).ok()?;
let root_canon = std::fs::canonicalize(base).ok()?;
if !canon.starts_with(&root_canon) { return None; }
Some(canon)
```

**Environment note**: This issue exists in dev too — a multi-tenant CI runner is the most realistic exposure path. Single-developer machines are lower-risk but not zero (e.g., `ynz` invoked on a downloaded project containing a malicious import).

---

## HIGH — Predictable temp filename for runtime library extraction (TOCTOU + symlink swap)

**Category**: Insecure temp-file handling
**OWASP**: A01:2021 — Broken Access Control
**Location**: `crates/ynz-driver/src/build.rs:182,310`

**Issue**: The runtime library is extracted to `${TMPDIR}/libynz_runtime_<pid>.a` — a name that is fully predictable from the parent PID. On a shared multi-user system:

1. Attacker creates `${TMPDIR}/libynz_runtime_12345.a` as a symlink pointing to `/home/victim/.ssh/authorized_keys` (or any file the running user can overwrite) **before** the ynz process with PID 12345 reaches the extraction step.
2. `std::fs::write(&rt_lib_tmp, RUNTIME_LIB_BYTES)` (build.rs:183, 311) follows the symlink and overwrites the target file with the runtime library bytes.
3. After link, `std::fs::remove_file(&rt_lib_tmp)` removes the symlink.

The result: arbitrary file overwrite (limited to the bytes of `libynz_runtime.a` — but those bytes contain valid ELF/Mach-O object content, which is enough to brick configuration files or, in the right context, plant attacker-staged data).

**Attack scenario**: On a shared CI runner where the attacker can `ls /tmp` and observe a build kicking off, they race to create the symlink before extraction. PIDs are easy to enumerate (`/proc/*/cmdline` shows `ynz build`). Even without `/proc`, PIDs cycle predictably.

**Impact**: Arbitrary file overwrite on shared systems. On single-user dev machines, low risk; on multi-tenant CI/build runners, this is the standard insecure-tempfile foot-gun.

**Vulnerable code** (`build.rs:182, 310`):
```rust
let rt_lib_tmp = std::env::temp_dir()
    .join(format!("libynz_runtime_{}.a", std::process::id()));
if let Err(e) = std::fs::write(&rt_lib_tmp, RUNTIME_LIB_BYTES) { ... }
```

**Secure fix**: Use `tempfile::NamedTempFile` (the `tempfile` crate is already in the dependency graph for testing — promote it to a runtime dep). It creates files with `O_CREAT | O_EXCL` and a random suffix, defeating both prediction and TOCTOU. Alternatively use `tempfile::tempdir()` and put the .a file inside.

**Environment note**: Linux + macOS `/tmp` is world-writable; the symlink-overwrite primitive is the standard CWE-377 / CWE-378 pattern. Windows behaves slightly differently but the predictable-PID pattern is still exploitable.

---

## HIGH — Output binary written next to source (cross-user binary squat / shared-dir race)

**Category**: Insecure file write path
**OWASP**: A01:2021 — Broken Access Control
**Location**: `crates/ynz-driver/src/build.rs:115,294,333` and `crates/ynz-driver/src/run.rs:21`

**Issue**: `ynz build foo.ynz` writes the object file to `foo.o` next to the source, and the final binary to `foo` (no extension, same directory). `ynz run foo.ynz` does the same, then executes `foo` and removes it.

Two compounding problems:

1. **Pre-existing file silent overwrite.** `std::fs::write` clobbers any existing file at `foo` or `foo.o` — including a regular file, a symlink (which follows), or a directory/binary the user actually wanted to keep. On `/shared/data/scripts/` style paths this can clobber arbitrary same-named files the user owns.

2. **TOCTOU between write and execute** (`run.rs:21`). The binary is written (build.rs:348), then executed (run.rs:21), then removed (run.rs:27). Between write and execute, an attacker with write access to the directory can swap `foo` for a different binary. `process::Command::new(&binary).status()` will execute whatever now sits at that path.

**Attack scenario**: User runs `ynz run /opt/shared-project/main.ynz` on a system where another user has write access to `/opt/shared-project/`. Attacker watches for the file `main` to appear (or pre-creates a directory `main` to fail the write), then swaps it. User executes the attacker's binary as themselves.

**Impact**: On shared/world-writable directories, arbitrary code execution as the invoking user. On personal-only directories, file clobber.

**Vulnerable code** (`build.rs:333`, `run.rs:21`):
```rust
let binary_path = source_path.with_extension("");
// ... write object, link with cc, output to binary_path ...
// Then in run.rs:
let status = process::Command::new(&binary).status().unwrap_or_else(|e| { ... });
```

**Secure fix**: For `ynz run`, write the binary to a per-invocation tempdir (`tempfile::tempdir()` returns a freshly-created, mode 0o700, randomly-named directory) and execute from there. For `ynz build`, accept that the user wants the binary in `cwd`, but emit a warning if the target file already exists and is not owned/writable by the current user. At minimum, use `OpenOptions::new().write(true).create_new(false).truncate(true).open()` so the file content is overwritten without following dangling symlinks (NB: `fs::write` already truncates but follows symlinks — `O_NOFOLLOW` on the open call is the standard hardening).

**Environment note**: Single-user dev machines using `/home/user/projects/` are not vulnerable. Shared CI runners, classroom labs, multi-developer build hosts are vulnerable.

---

## MEDIUM — Object/binary files written into project tree with no symlink check

**Category**: Path traversal via symlink (compile-time write primitive)
**Location**: `crates/ynz-driver/src/build.rs:115,294`

**Issue**: When walking the project, `collect_ynz_files` (`load.rs:174-222`) accepts any `.ynz` file regardless of whether the containing path crosses a symlink boundary. After codegen, the corresponding `.o` file is written to `entry.path.with_extension("o")`. A malicious project layout can place a symlink-directory in `src/` pointing at a victim directory; the `.ynz` files inside get parsed normally, and the `.o` file write goes back to the same canonicalized location (writing into the victim's directory).

The `.o` files are deleted on the success or failure path (`build.rs:130-132, 146-147, 352`), but a write that succeeds then immediately gets cleaned up still creates the file briefly, AND the cleanup uses the same canonicalized path, so a swap during the window between write and cleanup gives the attacker an inode of their choice.

**Attack scenario**: Malicious project at `/tmp/attack/` contains `src/` → symlink to `/home/victim/sensitive-dir/`. Running `ynz build /tmp/attack/` writes `*.o` files into `/home/victim/sensitive-dir/`. The files are then removed — but the act of writing files into the victim's directory (with names matching the victim's existing source files, since they share names) can clobber existing `.o` artifacts.

**Impact**: Limited file clobber via symlinked project directories. Lower severity than the temp-file issue (#2) because the user has to deliberately invoke `ynz` on the attacker's project, but worth fixing for defense-in-depth.

**Vulnerable code** (`load.rs:193-220`):
```rust
for entry in read_dir.flatten() {
    let path = entry.path();
    if path.is_dir() {                       // follows symlinks
        collect_ynz_files(src_root, &path, entries, diags);
    } else if path.extension().is_some_and(|e| e == "ynz") {
        // ... read file, ultimately write .o next to it
    }
}
```

**Secure fix**: Use `std::fs::symlink_metadata()` instead of `is_dir()` so symlinks are not followed during the walk. Optionally allow an opt-in flag for users who legitimately use symlinked source layouts.

---

## MEDIUM — Stack overflow via deeply-nested expressions or statements

**Category**: Resource exhaustion (compiler DoS)
**Location**: `crates/ynz-parser/src/parser.rs:2132` (`parse_expr`), `crates/ynz-typeck/src/check.rs` (`check_expr`/`infer_expr` family), `crates/ynz-codegen/src/emit.rs:2137` (`lower_expr`)

**Issue**: The parser limits *type* recursion to depth 16 (`parser.rs:868`), but expression and statement recursion are unbounded. A `.ynz` file containing `((((((((((((((((1))))))))))))))))` repeated millions of times — or right-associative `1+1+1+1+1+...` for the same depth — will:
1. Cause `parse_expr` to recurse to that depth.
2. Cause `check_expr` to recurse over the resulting AST.
3. Cause `lower_expr` to recurse during codegen.

Default Rust thread stack is ~8 MB. With ~100-200 bytes per stack frame across the parser, an attacker can blow the stack with a file of ~50k-100k unmatched-open-paren tokens. The compiler panics with a stack overflow signal (SIGABRT on Linux) — not a graceful diagnostic.

**Attack scenario**: User feeds a malicious `.ynz` (or imports one) that crashes `ynz build` mid-parse. On a CI runner this aborts the build with no usable error; on an editor LSP it crashes the language server.

**Impact**: Compiler DoS. Not a memory-safety bug (Rust's overflow protection aborts cleanly) but a UX/availability issue. Could also bypass `cargo fuzz` harnesses that consider abort-on-stack-overflow non-fatal.

**Vulnerable code**: All three recursive expression handlers have no depth tracking.

**Secure fix**: Add a depth parameter to `parse_expr` mirroring the existing `parse_type_with_depth` pattern. Reasonable expression-nesting depth: 256 (well above any handwritten code, well below stack limit). Same fix for `check_expr` / `lower_expr` — or run the entire compile inside a `stacker::grow` boundary so deeply-recursive ASTs allocate on a separate stack rather than aborting.

---

## MEDIUM — Unbounded interpolation depth stack in lexer

**Category**: Resource exhaustion
**Location**: `crates/ynz-parser/src/lexer.rs:27,746`

**Issue**: `interp_depth_stack: Vec<u32>` tracks open interpolations across `${...}`. Each `${` push (`lexer.rs:746`) adds an entry; there's no cap. A pathological input with millions of unclosed interpolations consumes O(N) memory in the stack vector. The bytes vector for the BacktickString segment is also rebuilt on each push (`lexer.rs:742`), but each is bounded.

**Attack scenario**: Source file containing `\`${\`${\`${\`${...` repeated millions of times. Compiler memory grows linearly with input size — not exponential, but a single file can pin a large vector without bound.

**Impact**: Memory exhaustion on very large inputs. Limited because file size already bounds memory (the source itself must be in RAM). Worth bounding to a sensible value to fail fast with a teaching diagnostic.

**Secure fix**: Cap `interp_depth_stack.len()` at e.g. 64 — far more than any real code uses. Emit a teaching diagnostic on overflow: "String interpolation nested too deeply (limit: 64). Break the string into smaller pieces."

---

## MEDIUM — Diagnostic includes raw OS error (limited information disclosure)

**Category**: Information disclosure
**Location**: `crates/ynz-typeck/src/resolve_import.rs:283`, `crates/ynz-driver/src/load.rs:13,213`

**Issue**: When file read fails, the error path renders the OS error to the diagnostic, e.g.:
```rust
format!("Cannot read module \"{module_str}\": {e}.")
```
On Linux, `e: io::Error` from `read_to_string` can include the absolute canonicalized path (in newer Rust) or only the error class (older). The canonicalized path resolved through the path-traversal issue (#1) gets surfaced verbatim. Combined with #1, this is the leak channel: even when the victim's `.ynz` file is not parseable, the IO error message confirms its existence and discloses the canonical path.

**Attack scenario**: Combined with #1 — attacker imports `foo/../../../home/victim/secret/file` with various extensions. The `Not Found` vs `Permission denied` vs `Is a directory` error text discloses the filesystem layout of the victim's home directory.

**Impact**: Filesystem enumeration primitive. Not a direct file-content leak (that's #1) but an oracle for "does file X exist."

**Secure fix**: When path traversal is fixed (issue #1), this disclosure scope narrows to project-internal paths. Additionally, scrub absolute paths from error messages — show only the module string the user typed, not the resolved location.

---

## LOW — `yinz.toml` parser accepts unknown keys silently (after a warning)

**Category**: Input validation
**Location**: `crates/ynz-driver/src/load.rs:79-108`

**Issue**: The hand-rolled TOML parser accepts known keys (`entry`, `name`, `version`) and warns on unknown keys. The parser uses simple `line.strip_prefix("entry")` matching — so a line like `entry_evil = "x"` would match `entry` prefix and be quietly accepted as the entry, just with weird parse semantics. Worse, the `parse_toml_string` helper trims whitespace and `=`, so the matching is loose.

Walk through `entryx = "evil.ynz"`:
- `line.strip_prefix("entry")` returns `Some("x = \"evil.ynz\"")`
- `parse_toml_string("x = \"evil.ynz\"")` strips leading non-quote chars by `trim_start_matches(|c: char| c.is_whitespace() || c == '=')` which only trims whitespace and `=`, NOT the `x`. So `inner` ends up as `x = "evil.ynz"` and after the quote-strip check fails (no surrounding quotes on the whole thing), it returns `Some("x = \"evil.ynz\"")` as the entry value.

The result: `entry = "x = \"evil.ynz\""` — broken value but not a security issue per se. The `entry` value is never used as a path for I/O in v0.1 (the loader walks the project root for all `.ynz` files — `entry` is currently unused, see `#[allow(dead_code)]` on `ProjectConfig.entry` at load.rs:42).

**Impact**: Latent bug — when the `entry` field becomes load-bearing in a future version, this loose matching becomes a path-injection vector.

**Secure fix**: Use a real TOML parser (the `toml` crate). Strict key matching with `==` not `strip_prefix`.

---

## LOW — `ynz-runtime` heap functions don't check malloc/realloc return values (compiled-binary stability, not compiler)

**Category**: Null-pointer-dereference / DoS
**Location**: `crates/ynz-runtime/src/lib.rs:405-411` (`map_alloc`), `:773-786` (`ynz_array_push`), `:464+` (`map_grow_int`/`map_grow_str`)

**Issue**: `malloc` and `realloc` return value is cast to `*mut T` without null-checking. The next operation dereferences. If the system is out of memory, the user's compiled program crashes with SIGSEGV rather than a clean abort.

This is a runtime (compiled-binary) issue, NOT a compiler issue. The threat model says "compiler input shouldn't allow code execution beyond what compilation+linking permits." Crashes in compiled-program runtime are out of scope for the compiler's threat model, but worth noting because the runtime ships embedded inside `ynz` and is therefore in the compiler's blast radius for review.

**Impact**: User-compiled programs crash unsafely under OOM. No code-execution risk (it's a null deref, not a write).

**Secure fix**: Add null checks after `malloc`/`realloc` and call `std::process::abort()` on null, matching the pattern already used in `ynz_alloc` (lib.rs:252) and `ynz_error_new` (lib.rs:1591). Existing `ynz_alloc` IS the right model — the other allocation paths should call it instead of raw `malloc`.

---

## Summary

- Path traversal: 2 (import resolution + symlink walk)
- Insecure file handling: 2 (temp file, output binary)
- Resource exhaustion: 2 (expression recursion, interpolation depth)
- Information disclosure: 1
- Input validation: 1
- Runtime null-deref: 1

## Priority

### Emergency (Deploy Fix Today)
None. The compiler is pre-v1.0 and is not exposed to untrusted input in production deployments yet.

### High (This Sprint)
- **#1 Path traversal in `resolve_module_path`** — landmark fix for the project's first real attacker-controlled input vector. Compiler's `module_str` validation is a v0.1 obligation.
- **#2 Insecure temp-file extraction** — straightforward `tempfile::NamedTempFile` swap. Low effort, high payoff for shared-CI users.
- **#3 Output binary write path** — at minimum, `ynz run` should drop the binary into a per-invocation tempdir.

### Medium (Next Sprint)
- **#4 Symlink-following project walk** — `symlink_metadata` swap.
- **#5 Stack overflow on deeply-nested expressions** — add depth limit mirroring the existing type-depth pattern (`parser.rs:868`).
- **#6 Bounded interpolation stack** — small cap with teaching diagnostic.
- **#7 Diagnostic path scrubbing** — narrows the leak channel from #1.

### Backlog
- **#8 TOML strict parsing** — pre-emptive before `entry` becomes load-bearing.
- **#9 Runtime malloc null-check** — runtime correctness, not compiler-input security.

## Security Posture

**Rating**: Needs Work

**Strengths**:
- Linker invocation is safe — no shell, no string interpolation of source values. `Command::new` + `arg(&path)` style throughout.
- Banned-jargon list (`crates/ynz-diagnostics/src/banned_jargon.rs`) is a static `&[&str]` literal compared with substring match — no regex, no ReDoS surface.
- Number literals are bounds-checked at lex time (`lexer.rs:1079-1089, 881-889`).
- Type-recursion depth is capped (`parser.rs:868`).
- Frame-stack in runtime is capped at 1024 (`runtime/lib.rs:1544`).
- SipHash for map keys uses a per-process key seeded from `/dev/urandom` (`runtime/lib.rs:286-295`) — solid hash-DoS mitigation in the compiled program.
- Source UTF-8 validation happens at file load (`driver/load.rs:22`) — the lexer can assume valid UTF-8.
- Salsa db model is consistent — no cross-db panic risk from imports.

**Gaps**:
- No path-traversal hardening on import resolution. This is the most exploitable gap.
- Tempfile usage is predictable-PID-based, not random-named. Standard CWE-377 footgun.
- No expression-depth limits in parser/typeck/codegen. Compiler DoS via crafted source.
- Symlink-following file walk allows cross-tree write primitives in the build pipeline.
- Output binary placement next to source is a UX choice with security side-effects on shared dirs.
- Runtime heap functions skip null-checks on a path that's already known to be handled correctly elsewhere in the same file (the discipline is inconsistent).
