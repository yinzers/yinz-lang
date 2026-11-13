# S1 independent verification (card #375) — branch spike/v0-3-m9-s1-verify @ ea3a2de

Toolchain used (not committed): rustup stable; LLVM 18.1.8 prebuilt tarball at /tmp/llvm18 (llvm-config needed a libtinfo.so.5 shim: a stub .so with version NCURSES_TINFO_5.0.19991023 linked to libtinfo.so.6, plus .so dev symlinks for z/zstd/ffi/xml2/stdc++ in /tmp/shim; `LD_LIBRARY_PATH=/tmp/shim RUSTFLAGS="-L /tmp/shim" LLVM_SYS_181_PREFIX=/tmp/llvm18 cargo build -p ynz-driver`).
Run: `FREE=0|1 bash scratch/s1-string-ownership/run.sh target/debug/ynz scratch/s1-string-ownership/t*.ynz`. Raw: s1-verify-control-raw.txt, s1-verify-free-raw.txt.

## 1. Control (free OFF) — CONFIRMED
38 probe files in the dir (t27 is NOT committed: the spiker's raw has it, as the one BUILD FAIL, unsupported `maybe<string> = s`). All 38 build and run `exit=0 CLEAN` under -fsanitize=address.

## 2. Free ON — CONFIRMED, every listed probe, same verdicts and addresses as the spiker's notes
| probe | result |
|---|---|
| t01 literal | SEGV in ynz_free (.rodata address 0x5fa0ab) |
| t05/t05b/t05c int/float/number toString | "attempting free on address which was not malloc()-ed" |
| t06 | same invalid free; with free OFF output is `222`,`222` (a clobbered by b) -> shared thread-local buffer confirmed with no free involved |
| t03 `let b = a` | double-free |
| t04 `a.copy()` | double-free |
| t12 shape field | double-free (t13 array, t19 reassign, t26 map, t35 param->field also double-free) |
| t14 / t25 channel | double-free; t24 literal over channel SEGV |
| t15 background arg | heap-use-after-free READ in T0, task thread T4 freed it; t28, t34 also UAF (T17 / T2) |
| t23 `names[9].or(\`none\`)` | SEGV (frees .rodata default) |
| t00, t07, t08*, t09, t10, t11, t29, t30, t33 (interp, case, trim, substring, split...) | CLEAN |
Also CLEAN: t02, t17, t18, t20. Failing: t16 (param alias UAF), t21/t22 (literal field/elem SEGV), t31 (bool SEGV), t32 (error message SEGV).
Optimized (`OPT=`) spot check t00,t01,t03,t12,t14,t15: same verdicts.
Added by me: t23b/t23c — ONE site `names[i].or(\`none\`)` with i=0 returns the heap element (`a5`) and with i=9 the static literal (`none`); both print correctly with free off; with free on SEGV. Confirms the dynamic-provenance premise: the VALUE cannot be classified, only the EXPRESSION.
Caveat on the ASan method: Yinz code and the Rust runtime are uninstrumented, so UAF is seen only through libc calls (strlen/puts/CStr). Fine for these probes; it means "CLEAN" for non-freed probes proves no invalid libc free, not absence of every UAF.

## 3. Code reads — CONFIRMED
- string = bare ptr, NUL-terminated, no header/length: runtime allocators lib.rs:1622 (from_static), :1794, :2147, :2171; `ynz_map_set_str` stores `key as i64` directly (lib.rs:1066). IMP-strings.md's 24-byte SSO `YnzString` is documented "ABI locked" but not implemented.
- owned_copy.rs:151-153 `Type::String => OwnedCopy::ReceiverIsCopy`; header :50-52 "pointer to immortal, immutable bytes".
- runtime_decls.rs:118 "null for primitive/string element types"; emit.rs:15728 "string's DELIBERATELY glue-less immortal bytes — raw-malloc'd, invisible to the alloc counter, freeing would be unsound"; types.rs:297 `channel_elem_drop` same. The current design is a recorded "strings are immortal" decision, not an omission.

## 4. Challenge to the conclusion

### What the spike got right
The failure matrix (static, thread-local, aliasing, dynamic) is reproduced and complete for the probes. "Binary owned/static mark is not enough" holds. Note the recommended header scheme is *not* the only sound one, and the spike's framing hides three things.

### Findings the spike did not state
1. **Strings are deliberately outside the existing ownership machinery.** `effective_ownership.rs:297` `value_like()` lists `Type::String` ("bits or immortal string bytes ... never aliases a heap allocation"); IMP-ownership.md:229 classes string literals/interpolation as `Fresh (... immortal string bytes)`; `check.rs:2774` likewise. So moves/alias classes (option C) are not "typeck for string, weeks": they need a language-visible change (`let b = a; print(a)` becomes a move or a copy) plus removing string from `value_like` everywhere. That is a design call for Chief/Patrick, not an implementation choice.
2. **The 24-byte `YnzString` is ABI-LOCKED in IMP-strings.md (M7 SSO layout).** If that lands, strings become a by-value 24-byte struct with a tag byte and cap field; a refcount header at ptr-8 is then throw-away work, and the struct has room for an owned/static discriminator (cap top bit, heap vs inline) for free. The spike notes mention the layout is unimplemented but not that A conflicts with the locked ABI. The decision "which string ABI do we free against" comes first.
3. **No scope-exit pass exists** (IMP-ownership.md:411: "the compiler emits no release of any local at scope end"). The spike's free is only at fall-off end of `entrypoint`/`-> nothing` fns. Any scheme must also release on early `return`, `break`/`continue`, error paths and — for state-machine frames — on task cancellation / frame teardown. That belongs to C3 and the 4-6 d estimate must be read as *excluding* it.

### Is there a cheaper sound scheme than the ptr-8 refcount header? Yes, one: "B-prime", copy-on-store with an expression-level Fresh whitelist
- Classify the *expression*, not the value. `OwnedFreshString` (conservative whitelist): `${}` interpolation, toUpperCase/toLowerCase/trim/substring/replace/split elements/options.toString, `.receive()` result. EVERYTHING else (literal, toString-of-number, bool, redacted, error message, field/index/param/`.or(..)`/`.get(..)`/map value/loop cell) is `Borrowed`.
- A string is *stored* (let/assign, struct field, array add/set/literal, map set/key, send, spawn arg, return of a non-local) by **moving** an `OwnedFreshString` and by `strdup`-ing a `Borrowed` one. Each heap string then has exactly one owner pointer; release = plain `free` at owner death (scope exit / container teardown glue / frame teardown). No header, no atomics, no change to literal emission, bool/redacted/error statics or to the one-`ptr` ABI.
- t23/t23b handled: `.or(lit)` is a `Borrowed` *expression*, so its result is always copied on bind, whichever branch it took at runtime. Dynamic provenance is only fatal for schemes that infer from the *value*; it is fine for an expression-level conservative classifier. (The spike says this too under B, but dismisses it as "needs a compile-time mark anyway"; that mark is a pure function of the expression and is cheap.)
- Cross-thread (t15/t28/t34): spawn arg and channel send copy/move, so the task owns its bytes; no sharing, no atomics. This is also exactly the existing `emit_owned_copy` seam (`CopyMode::SpawnArg`, `.copy()`): change the `String` row of `owned_copy_plan` from `ReceiverIsCopy` to a heap-copy plan, and `.copy()`/spawn args become correct in one place (and `.copy()` then does what its name says).
- Fail-safe: an unclassified expression defaults to `Borrowed` => an extra copy, never a double free. In A the failure mode of a missed retain is UAF/double free; of a missed release a leak. In B-prime a missed *store-site copy* is still a double free, so the site audit is the same size; only the default direction of the classifier is safer.
- Cost: one malloc+memcpy per store of a Borrowed string (field/element reads bound to a new name, `.or(lit)`, param stored to a shape — t35). A: one *atomic* RMW per store, but no allocation; per-thread non-atomic is impossible without knowing the thread. Golden Rule 8/10: B-prime's extra copies equal what a careful Rust author writes (`String::clone`) only where Rust would also clone; where Rust borrows (`&str` field read), B-prime copies. This could exceed hand-written cost and needs Patrick's explicit approval (Rule 8 clarification). A's atomics on every retain likewise exceed an `Rc`-style Rust solution and need the same approval — neither is free of Rule 8.
- Residual hazards under B-prime, same as A: the 3 thread-local number buffers (lib.rs:112/186/205) must be fixed (or kept `Borrowed` and never freed; but t06 clobbering is a bug on its own); map string KEYS are stored by pointer (lib.rs:1066) so key store must copy and map teardown glue must free keys; array/map/shape/arc/channel/maybe/bg-envelope teardown glue must walk string fields (shape `ynz_free(ptr,size)` ignores field types today).

### Which to pick (my view)
1. If the 24-byte SSO ABI is still the target: build neither now; implement strings once against that layout (owned/static discriminator in the tag/cap) and keep strings immortal (option D) until then. The cheapest sound scheme is not building the wrong ABI twice.
2. If strings stay one `ptr` for v0.3: prefer **B-prime over A**. Same site count, no change to literals/statics/ABI, no atomics, simpler cross-thread story (ownership transfer, not sharing), reuses `emit_owned_copy`; the price is copies (needs Patrick's Rule-8 sign-off, documented). A is the better pick only if profiling shows the Borrowed-copy rate matters (read-heavy code over string fields/arrays), because A makes reads of an existing string free (retain) and avoids allocation.
3. Either way first fix, independent of scheme: the thread-local buffers (lib.rs:112/186/205; t06 is a live bug), and commit the missing t27 probe or drop its mention.

### Sanity-check of the 4-6 engineer-day estimate (header scheme A)
Counted from the code, not rerun: runtime allocators 5 (lib.rs:1622/1794/1854/2147/2171); literal/statics emitters 3 + bool/redacted/error-message statics; string store sites in emit.rs: let, assign, struct-lit field, field assign, array literal, array `add`, `set`, index assign, map literal, map `set` value, map `set` string KEY, channel send, spawn-arg, return of non-local, `maybe<string>` wrap, destructure/for-loop cells, `.or()`/`get`/`first`/`last` reads = ~17-22 distinct sites (spike says 25-30; plausible once variants such as SM-frame lowering and optimised paths are counted). Teardown glue kinds: array, map (value+key), shape (none today: HeapShape is `ynz_free(ptr,size)`), Arc'd shape (arc.rs shares string-field shapes across threads), channel (null glue today), maybe/option payload, bg-arg envelope, SM frame. That is ~8 glue kinds, each needing a typed walker plus tests; the spike books ~1 d for it.
My estimate: **6-10 engineer-days**, not 4-6, for A *given a scope-exit/frame-drop pass already exists* — because (a) glue for 8 kinds is new infrastructure, not a tweak; (b) atomic RC + `Arc` interplay needs a race-oriented test pass (TSan, not only ASan: ASan does not catch the data race on the count); (c) every optimised/SM lowering path duplicates the sites (spike saw `lower_function` vs `lower_sm_body`). It EXCLUDES the scope-exit/early-return/cancellation release pass (C3), which is the prerequisite and the larger unknown. B-prime is the same order (7-10 d) with the allocation cost in place of the TSan work. Neither estimate is derived from a prototype; both are site-count judgements.

### What was NOT verified
Only the 38 committed probes were run (debug build for all, `--no-optimize`-off for 6). The estimate and the B-prime proposal are reasoned from code reading, not prototyped. t27 absent from the branch.
