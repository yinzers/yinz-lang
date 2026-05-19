# Round 2 — Fix-by-Fix Breakdown

Three parallel agents on disjoint file sets. ~15 fixes total. Round 1's results land first; this is the pre-approved plan for the next dispatch.

- **Batch 5a — Parser / lexer hardening**: `crates/ynz-parser/` only (~8 fixes)
- **Batch 5b — Runtime safety**: `crates/ynz-runtime/` only (~5 fixes)
- **Batch 5c — Path traversal + diagnostic path scrub**: `crates/ynz-typeck/src/resolve_import.rs` only (~2 fixes)

Each batch fully isolated to one crate / one file. Parallel-safe.

Delete this file when Round 2's commit lands.

---

## Batch 5a — Parser / lexer hardening

All in `crates/ynz-parser/`. Defensive fixes against malicious or pathological `.ynz` source files.

### 5a.1 — Cap expression nesting depth (CRITICAL)

**What's broken**: A `.ynz` file containing deeply-nested expressions like `((((((((((((((((((((((x))))))))))))))))))))))` repeated ~50k times deterministically crashes the compiler with a stack overflow. No diagnostic, no error message — just SIGABRT.

**Yinz analogy**: imagine if Yinz had no limit on shape inheritance depth. `shape A1 extends A2 extends A3 extends ... extends A100000` would blow the stack at typecheck time. The compiler needs to refuse to recurse past a sensible limit and emit a teaching diagnostic instead.

**File**: `crates/ynz-parser/src/parser.rs` — `parse_expr` and any sub-call that recursively descends (paren expressions, binary ops, unary, etc.).

**Existing pattern**: type recursion is already capped at 16 (see `parse_type_with_depth`). Expression recursion is unbounded.

**Fix**: thread a depth parameter through `parse_expr` (and its callees). Cap at 256 — well above any handwritten code, well below the ~8 MB Rust stack limit. On overflow, emit a teaching diagnostic:

```
COMPILE ERROR: Expression nesting too deep (max 256 levels).
WHAT INSTEAD: Break this expression into smaller pieces with named
              `let` bindings — Yinz prefers step-by-step over chains
              anyway (Golden Rule 7).
WHY: The parser uses one stack frame per nested expression. At 256
     levels we're well past any reasonable code; further nesting would
     crash the compiler. The limit catches both typos (a million open
     parens) and adversarial inputs.
```

**Risk**: Medium. Adds a depth parameter to a hot path. Existing tests should all pass — no real Yinz hits depth 256. New test: a fixture with 300 nested parens should produce the diagnostic, not crash.

### 5a.2 — Cap identifier length

**What's broken**: A `.ynz` file with a single 10 MB identifier (`aaaa...aaa` for 10 million chars) causes the lexer to allocate a 10 MB `String` for that one token. No defense against memory exhaustion.

**File**: `crates/ynz-parser/src/lexer.rs:486-639` (the identifier-lexing function).

**Fix**: cap identifier length at a reasonable limit (suggest 1024 bytes — `floatNumberInExtremelyDescriptivelyNamedTypeWithLongPrefixesForReadability` is ~70 chars; 1024 is 14× that). On overflow, emit:

```
COMPILE ERROR: Identifier too long (max 1024 characters).
WHAT INSTEAD: Split it into a short binding name and a longer descriptive
              comment, or trim the name.
WHY: Identifiers this long usually mean either a typo where the closing
     quote or boundary character is missing, or an attempt to exhaust
     memory. The limit is set far above realistic usage.
```

**Risk**: Low. No real Yinz code has 1024+ char identifiers.

### 5a.3 — Fix multi-byte Unicode error cascade

**What's broken**: A `.ynz` file with `日本語` in the middle of a function body (not inside an identifier or string) currently produces ONE diagnostic per BYTE of the codepoint. Each of those characters is 3 bytes in UTF-8 → 3 diagnostics per char, garbled spans.

**Yinz analogy**: imagine if every unknown character in a string interpolation triggered three separate `print` calls instead of one — what should be a single error becomes a flood.

**File**: `crates/ynz-parser/src/lexer.rs` — the unknown-character error path.

**Fix**: When the lexer encounters an unknown byte that turns out to be the start of a multi-byte UTF-8 codepoint, advance by the full codepoint length (1-4 bytes) and emit one diagnostic per codepoint, not per byte. Use `str::char_indices()` or check the first byte's high bits to determine the codepoint length.

**Risk**: Low. The error path itself doesn't run on valid code; this just makes errors cleaner.

### 5a.4 — Reject leading underscore in hex/binary literals

**What's broken**: `let bad = 0x_FF` currently parses as `0xFF` (the leading underscore between the `0x` prefix and the digits is silently stripped). Spec says digits can use `_` as a thousands separator (`1_000_000`), not as a leading character (`_FF`).

**File**: `crates/ynz-parser/src/lexer.rs:1157-1172` (`validate_underscores`).

**Fix**: tighten `validate_underscores` to reject leading `_` in hex (`lex_hex_int`) and binary (`lex_binary_int`) paths. Decimal literals are unaffected because the decimal path only enters on a digit, so a leading `_` is impossible there.

Emit the existing "underscore in wrong position" diagnostic at the bad offset.

**Risk**: Low. New diagnostic on a path that today silently accepts the bad form.

### 5a.5 — Cap string-interpolation depth

**What's broken**: A `.ynz` file with deeply-nested `${${${...}}}` interpolation grows the lexer's `interp_depth_stack` Vec without bound. Linear memory growth driven by input size (so bounded by file size, but still wasteful).

**File**: `crates/ynz-parser/src/lexer.rs:27, 746` (`interp_depth_stack`).

**Fix**: cap `interp_depth_stack.len()` at 64. No real Yinz code interpolates more than 4-5 levels. On overflow, emit:

```
COMPILE ERROR: String interpolation nested too deeply (max 64 levels).
WHAT INSTEAD: Build the string with named `let` bindings instead of
              inline interpolation.
WHY: Each `${...}` opens a new nesting level. Beyond 64 the lexer
     can't recover meaningfully; the limit catches malformed input
     and adversarial nesting.
```

**Risk**: Low.

### 5a.6 — Reject `\0` in string literals (CRITICAL — half of NUL byte fix)

**What's broken**: `` `hello\0world` `` currently compiles. The `\0` escape produces a NUL byte in the string. At runtime, `print(x)` truncates at the NUL because strings are emitted as C strings. Silent data loss.

**Yinz analogy**: imagine if `print` silently dropped everything after the first space in your string. That's what's happening, but for NUL bytes instead of spaces.

**File**: `crates/ynz-parser/src/lexer.rs:782-784` (where the `\0` escape is accepted).

**Fix**: at the `\0` escape site in the lexer, emit a diagnostic instead of accepting:

```
COMPILE ERROR: `\0` (NUL byte) is not a valid escape in Yinz strings.
WHAT INSTEAD: Use a non-NUL placeholder like `*` or split the string
              into pieces if you need a sentinel.
WHY: Yinz strings are passed to the runtime as C strings (length-
     prefixed string slices ship in v0.X). Embedding a NUL byte would
     silently truncate the string at print/concatenate time. The
     compiler refuses up-front to avoid the silent-truncation footgun.
```

**Note for codegen-side**: this lexer-only fix handles the surgical case. A future migration to `{ptr, len}` string slices (per `design/strings.md`) would let us allow `\0` again. Until then, the lexer reject keeps users safe.

**Risk**: Medium. May break tests that intentionally used `\0` — investigate any failures.

**DECISION REQUEST**: ship the lexer-only fix here (5a.6), OR wait for the bigger `{ptr, len}` overhaul in a later batch? I recommend ship-now — the overhaul is design/strings.md territory, much bigger than an audit fix.

### 5a.7 — Reject `3.` as a valid float literal

**What's broken**: `let x = 3.` (no digit after the decimal point) currently lexes as the number `3.0`. Spec implies it should require at least one digit after the `.` (e.g., `3.0`). Inconsistent with spec.

**File**: `crates/ynz-parser/src/lexer.rs:996-1014` (where the float path consumes the `.`).

**Fix**: require at least one digit after the `.` in the float path. If absent, emit:

```
COMPILE ERROR: Decimal point without fractional digits.
WHAT INSTEAD: Write `3.0` if you mean three, or `3` if you mean integer three.
WHY: `3.` is ambiguous — it could be the start of a float literal or
     a typo. Yinz requires the digit so the intent is clear at the
     point of writing.
```

**Risk**: Low.

### 5a.8 — Multi-line string recovery: emit single error, not cascade

**What's broken**: A double-quoted `"..."` string spanning multiple lines (which is already invalid because double quotes are banned at M7) currently triggers cascade errors — the lexer stops recovery at `\n`, then parses the second "line" as code, which usually fails in a confusing secondary way.

**File**: `crates/ynz-parser/src/lexer.rs:649-668` (`lex_double_quote_error`).

**Fix**: when the lexer hits the unterminated double-quoted form, consume to the next blank line or EOF (not just to `\n`), emit ONE diagnostic pointing at the opening quote, and skip ahead.

**Risk**: Low. Existing tests that rely on the cascade behavior need updating — but those tests are arguably bad (they're testing a UX bug).

---

## Batch 5b — Runtime safety

All in `crates/ynz-runtime/`. Critical safety fixes in the C-ABI runtime.

### 5b.1 — Null-check malloc/realloc in map runtime (CRITICAL)

**What's broken**: The map runtime calls `malloc` and `realloc` without checking for null. On OOM the kernel returns null; the next line of code dereferences a null pointer → SIGSEGV. The user's program crashes with no diagnostic.

**Yinz analogy**: in your Yinz code, calling `.value` on an unguarded `maybe<T>` is a compile error. The runtime is doing the equivalent — using a maybe-null pointer without checking — but in unsafe Rust where the compiler can't catch it.

**Files**: 
- `crates/ynz-runtime/src/lib.rs:405-423` (`map_alloc`) — 5 unchecked `malloc` calls
- `crates/ynz-runtime/src/lib.rs:534-545` (`order_push`) — unchecked `realloc`
- Also check `crates/ynz-runtime/src/lib.rs:464-499` (`map_grow_int` / `map_grow_str`) — same pattern likely

**The fix**: check the return value after each `malloc`/`realloc`. On null, call `std::process::abort()` after a write to stderr explaining what happened.

There's already a model in the same file: `ynz_alloc` (line ~250) and `ynz_error_new` (line ~1591) do this correctly. Match that pattern.

The teaching message on abort:

```
RUNTIME ERROR: Out of memory while growing a map.
  The program tried to insert into a map but the system couldn't
  allocate more memory. Yinz aborts rather than continuing with an
  inconsistent map.
```

**Risk**: Low-medium. OOM paths are hard to test directly; the fix is mechanical. Use an existing OOM testing pattern if available (or skip the test — the fix is obviously correct from inspection).

### 5b.2 — Bounded probing in `ynz_map_set_str` (prevent infinite loop)

**What's broken**: After the load-factor growth check, the probe loop at line ~648 has no termination guard if every slot is occupied (no EMPTY or DELETED markers). The growth check normally prevents this (force-grow at 75%), but if `map_grow_str` silently fails (e.g., OOM combined with 5b.1's gap), the next insert spins forever.

**File**: `crates/ynz-runtime/src/lib.rs:629-656` (`ynz_map_set_str`) and `:452-462` (`find_insert_slot`).

**Fix**: track probe count; if it equals capacity, the table is genuinely full — abort with a teaching message:

```
RUNTIME ERROR: Map full and unable to grow.
  The map exceeded its load factor and the grow attempt failed
  (likely OOM). Yinz aborts rather than spin forever.
```

5b.1 + 5b.2 together: 5b.1 makes grow fail cleanly (abort on null malloc) BEFORE we ever hit the full probe. 5b.2 is the belt-and-suspenders if some other code path skips growth.

**Risk**: Low.

### 5b.3 — Don't silently return `"0"` when bignum CString fails

**What's broken**: `bignum_binop` formats the bignum result with `CString::new(s).unwrap_or_else(|_| CString::new("0").unwrap())`. If `s` contains a NUL byte (currently impossible from decimal formatting, but if a future change ever produces "Infinity" or "NaN" or anything weirder, this kicks in), the fallback silently returns "0". User sees a wrong math result with no diagnostic.

**Yinz analogy**: imagine if `int.parse("abc")` returned `0` instead of `none` or an error. The function would lie to the caller about what happened.

**File**: `crates/ynz-runtime/src/lib.rs:2292`.

**Fix**: replace the silent zero fallback with an abort + diagnostic:

```rust
let cstr = std::ffi::CString::new(s).unwrap_or_else(|_| {
    eprintln!("INTERNAL ERROR: bignum formatting produced an unexpected NUL byte. \
               This is a compiler bug — please file an issue at <URL> with the source file attached.");
    std::process::abort();
});
```

Use the same URL/format the panic-hook fix (Round 3 7.x) will land. For now hardcode a placeholder URL.

**Risk**: Low. Today's code paths never trigger this; the change makes future regressions loud instead of silent.

### 5b.4 — Document SipHash slice infallibility

**What's broken**: `try_into().unwrap()` on infallible byte-slice → fixed-array conversions in the SipHash impl. The `unwrap` will never fire today (the slice IS always 8 bytes), but if someone refactors the slicing and accidentally changes the length, the panic message is `TryFromSliceError(())` with zero context.

**File**: `crates/ynz-runtime/src/lib.rs:302-303, 336`.

**Fix**: add `# Safety` / `// SAFETY:` comments at each `try_into().unwrap()` explaining why the unwrap is safe (slice length is always 8 from the surrounding logic). OR use array-indexing directly: `[k[0], k[1], k[2], k[3], k[4], k[5], k[6], k[7]]` which avoids the conversion entirely.

I lean toward the array-indexing approach — eliminates the unsafe-feeling `.unwrap()` from the file.

**Risk**: Low. Pure refactor.

### 5b.5 — Document SipHash zero-key choice

**What's broken**: The SipHash key is initialized once per process from `/dev/urandom` (good — see lines 286-295). But the standard SipHash initialization vectors at lines 300+ are unexplained — they look like magic numbers. A future maintainer might "clean up" the magic constants without realizing they're load-bearing IVs from the SipHash paper.

**File**: `crates/ynz-runtime/src/lib.rs:300-360`.

**Fix**: add a comment block above the SipHash function citing the paper (Aumasson & Bernstein, 2012) and identifying the constants as the standard initialization vectors. Note that the project's per-process random key (already set up) provides hash-DoS protection for user-controlled keys.

**Risk**: Zero. Doc only.

### Note: runtime overflow/div-zero source location

**Deferred to Round 3.** This finding (Forensics High) — when `int + int` overflows at runtime, the panic message names only the operator, not the source line — is a cross-crate fix (codegen must pass source location to the runtime trap functions). Doesn't fit Round 2's file-disjoint constraint. Punts to Round 3 alongside the panic-hook + `--emit-ir` flag.

---

## Batch 5c — Path traversal + diagnostic path scrub

Both fixes in `crates/ynz-typeck/src/resolve_import.rs`. Small batch — could be merged into another Round 2 batch, but keeping it standalone respects the file-disjoint rule (resolve_import.rs is owned by typeck; if any Round 2 work expanded into typeck we'd conflict).

### 5c.1 — Block mid-path `..` traversal in imports

**What's broken**: An attacker-controlled `.ynz` file can import:
```ynz
import { x } from `foo/../../../home/victim/secret-project/internal`
```
The parser rejects leading `./` and `../` but NOT mid-string `..` components. After `Path::join + canonicalize` resolves the `..` segments, the import points to a file entirely outside the project root. If that file exists and parses as `.ynz`, its contents leak through diagnostic output.

**Yinz analogy**: it's like the file-system equivalent of SQL injection — the import string is user input that gets joined with a base path without sanitization, and the `..` is the injection vector.

**File**: `crates/ynz-typeck/src/resolve_import.rs:51-72` (`resolve_module_path`).

**Fix**: two layers of defense:

1. **Component check** — split the `module_str` by `/` and reject any segment that is exactly `..` or `.`:
   ```rust
   if module_str.split('/').any(|seg| seg == ".." || seg == ".") {
       // emit teaching diagnostic, return None
   }
   ```

2. **Project-root boundary check** — after canonicalize, verify the resolved path starts with the canonicalized project root:
   ```rust
   let canon = std::fs::canonicalize(&candidate).ok()?;
   let root_canon = std::fs::canonicalize(base).ok()?;
   if !canon.starts_with(&root_canon) { return None; }
   ```

Layer 1 catches the user's intent (typed `..` in an import path); layer 2 catches anything sneaky (symlinks, etc.).

Diagnostic for layer 1:
```
COMPILE ERROR: Import path "{module_str}" contains `..` or `.` segments.
WHAT INSTEAD: Use a project-root-relative path. If you need to import
              from a sibling directory, use the full path from the
              project root (`subdir/module`), never `../sibling/module`.
WHY: Yinz import paths are anchored to the project root (the directory
     containing `yinz.toml`). Allowing `..` would let an import escape
     the project boundary and could leak source from outside the
     project to diagnostic output.
```

**Risk**: Medium. The boundary check is the critical part; the component check is defense-in-depth.

**Mandatory tests (per Patrick's directive)** — exhaustive proof that traversal is caught:

**Should-FAIL cases** (these must each emit the path-traversal diagnostic; if any of them quietly resolves, the fix is incomplete):

```ynz
import { x } from `foo/../../etc/passwd`         // mid-path .. escaping the project root
import { x } from `subdir/../../escape`          // mid-path .. via subdir indirection
import { x } from `./inner/../../escape`         // ./ followed by .. that climbs out
import { x } from `subdir/./../../escape`        // mid-path . AND .. mixed
import { x } from `foo/bar/../../../../escape`   // multiple .. that overshoots
```

Plus symlink-based escape (post Batch 4b, the project walker is symlink-aware; this test verifies imports don't follow them either):

```
ln -s /tmp /proj/escape_link
// File at /proj/main.ynz:
import { x } from `escape_link/anything`        // resolves outside project root
```

The symlink test mirrors the file walker's defense; if it slips through here it'd allow an attacker-controlled `escape_link` symlink to load arbitrary files.

**Should-PASS cases** (these must each resolve normally; if any of them gets blocked, the fix is too aggressive):

```ynz
import { x } from `subdir/file`               // normal nested import
import { x } from `file`                      // flat import in project root
import { x } from `deeply/nested/module`      // multi-level path
import { x } from `dir.with.dots/file`        // dots in dir name (NOT path separators)
import { x } from `file_with_dots..rs.test`   // dots in file name
```

The last two cases matter: the component check must look at path COMPONENTS (`split('/')`), not `contains(".")`. A directory or file whose name happens to contain dots is fine; only segments that ARE exactly `..` or `.` are the attack vector.

**Test placement**: alongside the existing import-resolution tests (probably `crates/ynz-typeck/tests/`). Multi-file pattern with real temp directories, mirroring the `incremental_rebuild_invalidates_when_imported_signature_changes` test that landed in Batch 3. Each should-fail case is its own `#[test]` so a single regression doesn't mask others. Each should-pass case is its own `#[test]` for the same reason.

WHY comments per `testing.md`: each test states the invariant ("X is/isn't a path-traversal attempt"), what bug it'd catch if regressed, and the temptation to resist ("don't relax this — security regression"). Tests must NEVER be loosened without explicit security review.

### 5c.2 — Don't leak canonicalized paths in import error messages

**What's broken**: When import resolution fails (file not found, permission denied), the diagnostic includes the OS error which may carry the absolute canonicalized path:

```
Cannot read module "foo": No such file or directory (os error 2): /home/victim/secret/foo.ynz
```

Combined with 5c.1's mid-path-`..` vector (before the fix), an attacker can probe the filesystem layout of the host (does `/home/victim/secret/` exist? what files are in it?) via crafted imports + the OS error text.

**File**: `crates/ynz-typeck/src/resolve_import.rs:283`.

**Fix**: show the user's INPUT string in error messages, not the resolved/canonicalized path. The user typed `foo` — that's what should appear. The fully-resolved path is internal debugging info.

```rust
diags.push(Diagnostic::error(
    span.clone(),
    format!("Cannot find module `{module_str}`."),
    "Check the spelling and that the file exists under your project root.",
    "Imports resolve relative to the directory containing `yinz.toml`. \
     If `{module_str}` is in a subdirectory, the path should include it.",
));
```

Note: don't include the OS-error text either. "No such file" vs "permission denied" is a filesystem-enumeration oracle.

**Risk**: Low. The teaching value of the diagnostic increases (it speaks the user's language), and the leak channel closes.

---

## Summary

| Batch | Files touched | Fixes | Critical fixes included |
|---|---|---:|---|
| 5a Parser/lexer | `crates/ynz-parser/**` | 8 | 5a.1 (deep nesting), 5a.6 (NUL byte) |
| 5b Runtime | `crates/ynz-runtime/**` | 5 | 5b.1 (OOM null-deref) |
| 5c Path traversal | `crates/ynz-typeck/src/resolve_import.rs` | 2 | 5c.1 (mid-path `..`) |

**Total Round 2 fixes**: ~15
**Critical findings cleared**: 4 (out of 8 remaining post-Round-1 estimate)

---

## Decision items — RESOLVED

1. **5a.6 NUL byte truncation** — ✅ ship lexer-only fix in Round 2. The `{ptr, len}` overhaul stays as a separate future batch.
2. **Runtime overflow source location** — ✅ confirmed deferred to Round 3.
3. **5a.8 multi-line string cascade** — ✅ keep in Round 2.

Round 2 ready to dispatch.
