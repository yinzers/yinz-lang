# Security Analysis Report — Yinz Compiler

**Analyzed**: 2026-05-19
**Scope**: Entire `crates/**/*.rs` (compiler, driver, codegen, parser, typeck, runtime, diagnostics)
**Threat model**: A malicious `.ynz` source file (or project) compiled by `ynz build` / `ynz run` should not gain code execution beyond what normal compilation+linking permits, should not read arbitrary files outside the project, should not crash the compiler with attacker-friendly DoS, and should not corrupt object/binary outputs in surprising locations.
**Vulnerabilities Found**: 9 — 3 fixed Batch 4b (High: 1, Medium: 1 remain from those); remaining: 6 (High: 1, Medium: 3, Low: 2)

---

## ~~HIGH — Path traversal in import resolution (cross-project file disclosure via parse)~~ FIXED (Batch 5c)

**Category**: Path traversal
**OWASP**: A01:2021 — Broken Access Control
**Location**: `crates/ynz-typeck/src/resolve_import.rs`

**Fix**: Two-layer defense in `resolve_import.rs`:
- Layer 1 (segment check): `module_path_has_traversal_segments()` rejects any path where a `/`-split segment is exactly `..` or `.` before the filesystem is touched. Emits a teaching diagnostic with WHAT/WHAT-INSTEAD/WHY.
- Layer 2 (boundary check): after `std::fs::canonicalize`, verifies the resolved path `starts_with` the canonicalized project root. Catches symlink-based escapes that layer 1 cannot see. Returns `None` silently (maps to the existing "not found" diagnostic flow).

**Security test suite**: `crates/ynz-typeck/tests/path_traversal.rs` — 6 should-FAIL cases (mid-path `..`, subdir indirection, `.`+`..` mixed, multiple dotdots overshooting, symlink escape) + 5 should-PASS cases (normal nested, flat, deep nested, dir-with-dots, file-with-dots). All 11 pass.

---

## ~~HIGH — Predictable temp filename for runtime library extraction (TOCTOU + symlink swap)~~ FIXED (Batch 4b)

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

**Fixed**: `build.rs` now uses `tempfile::NamedTempFile::new()` (O_CREAT|O_EXCL, random suffix). The `tempfile` crate is now a runtime dep in `ynz-driver/Cargo.toml`. The `NamedTempFile` variable stays alive until the linker finishes, then drops and deletes the file. Verified: all 93 driver tests pass.

---

## ~~HIGH — Output binary written next to source (cross-user binary squat / shared-dir race)~~ PARTIALLY FIXED (Batch 4b)

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

**Status (Batch 4b)**: `ynz run` now writes the binary into `tempfile::tempdir()` (mode 0o700, random name) and executes from there — the TOCTOU race is eliminated for `run`. `ynz build` still writes next to source (user expectation); `O_NOFOLLOW` hardening for the `ynz build` output path is a remaining item (low priority on single-user dev machines).

**Environment note**: Single-user dev machines using `/home/user/projects/` are not vulnerable. Shared CI runners, classroom labs, multi-developer build hosts are vulnerable.

---

## ~~MEDIUM — Object/binary files written into project tree with no symlink check~~ FIXED (Batch 4b)

~~**Category**: Path traversal via symlink (compile-time write primitive)~~
~~**Location**: `crates/ynz-driver/src/build.rs:115,294`~~

**Fixed**: `collect_ynz_files` now uses `symlink_metadata()` instead of `is_dir()` — symlinks to directories are never followed during the walk. Additionally, all `.o` intermediates now go into `tempfile::tempdir()` rather than next to the source files, so even if a symlink were somehow followed, no `.o` write would land in the source tree.

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

~~## MEDIUM — Stack overflow via deeply-nested expressions or statements~~ **FIXED (Batch 5a — parser side)**

**Category**: Resource exhaustion (compiler DoS)

Parser-side fix shipped: `expr_depth` field on `Parser`, cap at 256, WHAT/WHAT-INSTEAD/WHY diagnostic + token drain to statement boundary on overflow. typeck (`check_expr`/`infer_expr`) and codegen (`lower_expr`) sides remain open — if the parser never produces an AST deeper than 256, typeck/codegen won't see one in practice, but a crafted AST from another source could still hit them.

---

## MEDIUM — Unbounded interpolation depth stack in lexer

**Category**: Resource exhaustion
**Location**: `crates/ynz-parser/src/lexer.rs:27,746`

**Issue**: `interp_depth_stack: Vec<u32>` tracks open interpolations across `${...}`. Each `${` push (`lexer.rs:746`) adds an entry; there's no cap. A pathological input with millions of unclosed interpolations consumes O(N) memory in the stack vector. The bytes vector for the BacktickString segment is also rebuilt on each push (`lexer.rs:742`), but each is bounded.

**Attack scenario**: Source file containing `\`${\`${\`${\`${...` repeated millions of times. Compiler memory grows linearly with input size — not exponential, but a single file can pin a large vector without bound.

**Impact**: Memory exhaustion on very large inputs. Limited because file size already bounds memory (the source itself must be in RAM). Worth bounding to a sensible value to fail fast with a teaching diagnostic.

**Secure fix**: Cap `interp_depth_stack.len()` at e.g. 64 — far more than any real code uses. Emit a teaching diagnostic on overflow: "String interpolation nested too deeply (limit: 64). Break the string into smaller pieces."

---

## ~~MEDIUM — Diagnostic includes raw OS error (limited information disclosure)~~ FIXED (Batch 5c)

**Category**: Information disclosure
**Location**: `crates/ynz-typeck/src/resolve_import.rs`

**Fix**: Swept all diagnostics in `resolve_import.rs`. No diagnostic interpolates `resolved_path.display()`, OS error text (`{e}`), or any canonicalized path. All error messages reference `module_str` (the user's input string) only. The dead `cycle_path` variable (computed from `resolved_path.display()` but unused) was also removed. Combined with the path-traversal fix (HIGH #1 above), the filesystem-enumeration oracle via "Not Found" vs "Permission denied" distinction is eliminated — traversal paths are rejected before reaching the filesystem.

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

## Summary (after Batch 5c)

- Path traversal: 0 remain (import resolution FIXED — layer-1 segment check + layer-2 boundary check + 11-test security suite; symlink walk FIXED Batch 4b)
- Insecure file handling: 1 remains (ynz build O_NOFOLLOW — ynz run tempdir FIXED, runtime lib NamedTempFile FIXED)
- Resource exhaustion: 2 (expression recursion, interpolation depth)
- Information disclosure: 0 remain (diagnostic path scrub FIXED — no resolved/canonicalized paths in any diagnostic)
- Input validation: 1
- Runtime null-deref: 1

## Priority

### Emergency (Deploy Fix Today)
None. The compiler is pre-v1.0 and is not exposed to untrusted input in production deployments yet.

### High (This Sprint)
- ~~**#1 Path traversal in `resolve_module_path`**~~ **FIXED (Batch 5c)**
- **#2 Insecure temp-file extraction** — straightforward `tempfile::NamedTempFile` swap. Low effort, high payoff for shared-CI users.
- **#3 Output binary write path** — at minimum, `ynz run` should drop the binary into a per-invocation tempdir.

### Medium (Next Sprint)
- **#4 Symlink-following project walk** — `symlink_metadata` swap.
- **#5 Stack overflow on deeply-nested expressions** — add depth limit mirroring the existing type-depth pattern (`parser.rs:868`).
- **#6 Bounded interpolation stack** — small cap with teaching diagnostic.
- ~~**#7 Diagnostic path scrubbing**~~ **FIXED (Batch 5c)**

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
- ~~No path-traversal hardening on import resolution.~~ **FIXED (Batch 5c)**
- Tempfile usage is predictable-PID-based, not random-named. Standard CWE-377 footgun.
- No expression-depth limits in parser/typeck/codegen. Compiler DoS via crafted source.
- Symlink-following file walk allows cross-tree write primitives in the build pipeline.
- Output binary placement next to source is a UX choice with security side-effects on shared dirs.
- Runtime heap functions skip null-checks on a path that's already known to be handled correctly elsewhere in the same file (the discipline is inconsistent).
