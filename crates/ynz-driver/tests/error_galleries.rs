// WHY: Error gallery files are the hands-on review surface for compiler diagnostic
// quality — per plan-invariants.md `### Demo & Error Gallery`. These tests assert
// that each gallery file produces its expected diagnostic count and that key error
// classes are present by key-phrase. If this count changes, either a new diagnostic
// class was added (update count + add a key-phrase check) or a diagnostic regressed
// (compiler bug). Both require deliberate action, not silent drift.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

fn galleries_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/ynz-driver; walk up two levels to workspace root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/primantis-orders")
}

fn gallery(name: &str) -> PathBuf {
    galleries_dir().join(name)
}

/// Compile a gallery file and return (stderr, exit_code).
fn compile_gallery(path: &Path) -> (String, i32) {
    let out = Command::new(ynz_binary())
        .args(["run", path.to_str().unwrap()])
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz binary");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let code = out.status.code().unwrap_or(-1);
    (stderr, code)
}

fn count_errors(stderr: &str) -> usize {
    stderr.lines().filter(|l| l.starts_with("Error:")).count()
}

#[test]
fn m4_gallery_fires_expected_diagnostics() {
    // WHY: m4_errors.ynz covers banned declaration keywords, parser errors, and
    // typeck errors. The 50-error cap is expected to trigger (50 errors).
    // If this count drops, a diagnostic class regressed. If it rises and doesn't hit
    // the cap, new M4 error classes were added without updating this test.
    let (stderr, code) = compile_gallery(&gallery("m4_errors.ynz"));
    assert_ne!(code, 0, "m4 gallery must exit non-zero");

    let error_count = count_errors(&stderr);
    // 50 = error cap fires (file has more than 50 possible errors).
    assert_eq!(
        error_count, 50,
        "m4 gallery must produce exactly 50 errors (cap); got {error_count}.\nstderr:\n{stderr}"
    );

    // Key phrase checks — one per major M4 error class.
    assert!(
        stderr.contains("type"),
        "m4 gallery must mention `type` keyword ban; got:\n{stderr}"
    );
    assert!(
        stderr.contains("struct"),
        "m4 gallery must mention `struct` keyword ban; got:\n{stderr}"
    );
    assert!(
        stderr.contains("class"),
        "m4 gallery must mention `class` keyword ban; got:\n{stderr}"
    );
}

#[test]
fn m5_gallery_fires_expected_diagnostics() {
    // WHY: m5_errors.ynz covers collection/array errors. 8 diagnostics expected.
    let (stderr, code) = compile_gallery(&gallery("m5_errors.ynz"));
    assert_ne!(code, 0, "m5 gallery must exit non-zero");

    let error_count = count_errors(&stderr);
    assert!(
        (7..=12).contains(&error_count),
        "m5 gallery must produce 7–12 errors; got {error_count}.\nstderr:\n{stderr}"
    );
}

#[test]
fn m6_gallery_fires_expected_diagnostics() {
    // WHY: m6_errors.ynz covers union type errors plus the narrowed-union background
    // receiver rejection (FRAGO 026). 10 diagnostics expected.
    let (stderr, code) = compile_gallery(&gallery("m6_errors.ynz"));
    assert_ne!(code, 0, "m6 gallery must exit non-zero");

    let error_count = count_errors(&stderr);
    assert!(
        (7..=14).contains(&error_count),
        "m6 gallery must produce 7–14 errors; got {error_count}.\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot yet be used as a `background` receiver"),
        "m6 gallery must include the narrowed-union background-receiver rejection \
         (FRAGO 026); got:\n{stderr}"
    );
}

#[test]
fn m7_gallery_fires_expected_diagnostics() {
    // WHY: m7_errors.ynz covers errors keyword, string, and iterable diagnostics.
    // Expected: 7 compile errors (noErrorsContext, badOr, badFailed, stringMutation,
    // badContains, iterateInt, iterateBool).
    let (stderr, code) = compile_gallery(&gallery("m7_errors.ynz"));
    assert_ne!(code, 0, "m7 gallery must exit non-zero");

    let error_count = count_errors(&stderr);
    assert!(
        (6..=12).contains(&error_count),
        "m7 gallery must produce 6–12 errors; got {error_count}.\nstderr:\n{stderr}"
    );

    assert!(
        stderr.contains("not handled"),
        "m7 gallery must include an unhandled-errors diagnostic; got:\n{stderr}"
    );
    assert!(
        stderr.contains("for` loops"),
        "m7 gallery must include a not-iterable diagnostic; got:\n{stderr}"
    );
}

#[test]
fn m8_gallery_fires_expected_diagnostics() {
    // WHY: m8_errors.ynz covers concurrency keyword errors (2 errors) plus
    // v0.1-polish inline shape type errors (4+ errors). Expected: 5–10.
    // test-ratchet: v0.1-polish adds 3 inline-shape error triggers (unknown field,
    // missing field, hidden-in-inline) — count grows from 2 to ~6.
    // v0.3-M8 Phase 4 (test-ratchet): the channel-close + transfer-rule section adds the
    // four new compile diagnostics (ConsumedBySend ×5 incl. the three alias forms,
    // ParamNeedsGive ×5 incl. both frames of the relay chain, TransferNeedsCopy ×5 incl. the
    // dynamic-contract instance, HandleChannelArgNeedsBinding ×1), the extracted const-send
    // refusal, the existing use-after-give error at two new sites (+2 same-call alias-pair
    // sites, fix round 2), and the two diagnostics
    // `.close()` extends (no-args, per-receiver unknown-method list) — 26–36 in all. Every
    // new class is pinned by key phrase below so the count can never pass for the wrong reason.
    let (stderr, code) = compile_gallery(&gallery("m8_errors.ynz"));
    assert_ne!(code, 0, "m8 gallery must exit non-zero");

    let error_count = count_errors(&stderr);
    assert!(
        (26..=36).contains(&error_count),
        "m8 gallery must produce 26–36 errors; got {error_count}.\nstderr:\n{stderr}"
    );

    assert!(
        stderr.contains("background"),
        "m8 gallery must include a background-share diagnostic; got:\n{stderr}"
    );

    // ConsumedBySend — the WHAT's fixed clause (IMP-ownership.md "Teaching text").
    assert!(
        stderr.contains("`send()` gave it away"),
        "m8 gallery must include the ConsumedBySend diagnostic; got:\n{stderr}"
    );
    // ConsumedBySend through an alias class — the `{via}` slot names what was sent.
    assert!(
        stderr.contains("which is what was sent"),
        "m8 gallery must include the alias-class `{{via}}` form of ConsumedBySend; got:\n{stderr}"
    );
    // ParamNeedsGive — the WHAT's fixed closing sentence.
    assert!(
        stderr.contains("Only a `give` parameter can be given away."),
        "m8 gallery must include the ParamNeedsGive diagnostic; got:\n{stderr}"
    );
    // ParamNeedsGive fires for BOTH frames of the relay chain in one build.
    assert!(
        stderr.contains("parameter of `m8HopB`") && stderr.contains("parameter of `m8HopA`"),
        "m8 gallery must report both m8HopB's and m8HopA's missing `give` in one build; got:\n{stderr}"
    );
    // TransferNeedsCopy — the WHAT's fixed clause, plus each `{reason}` form.
    assert!(
        stderr.contains("so someone here still holds it."),
        "m8 gallery must include the TransferNeedsCopy diagnostic; got:\n{stderr}"
    );
    assert!(
        stderr.contains("is a field of `bucket`"),
        "m8 gallery must include TransferNeedsCopy's field reason; got:\n{stderr}"
    );
    assert!(
        stderr.contains("one cell of `matrix`"),
        "m8 gallery must include TransferNeedsCopy's loop-cell reason; got:\n{stderr}"
    );
    assert!(
        stderr.contains("returns a piece of its `b` argument"),
        "m8 gallery must include TransferNeedsCopy's returns-a-piece reason; got:\n{stderr}"
    );
    // HandleChannelArgNeedsBinding — the WHAT's fixed clause.
    assert!(
        stderr.contains("which is not a named binding"),
        "m8 gallery must include the HandleChannelArgNeedsBinding diagnostic; got:\n{stderr}"
    );
    // The extracted const refusal carries the send sink's WHAT-INSTEAD.
    assert!(
        stderr.contains("wire.send(rows.copy())"),
        "m8 gallery must include the const-send refusal with the send-sink advice; got:\n{stderr}"
    );
    // `.close()` with arguments — WHAT and WHAT-INSTEAD both render (fix round 2: the renderer
    // read byte spans as char offsets and dropped the teaching block of any span that landed
    // past the file's char count; trigger order is irrelevant now).
    assert!(
        stderr.contains("`.close()` takes no arguments"),
        "m8 gallery must include the close-takes-no-arguments diagnostic; got:\n{stderr}"
    );
    assert!(
        stderr.contains("Call it bare: wire.close()"),
        "m8 gallery must render the close-takes-no-arguments WHAT-INSTEAD; got:\n{stderr}"
    );
    // The same-call alias pair (fix round 2, Producer C1): the use-after-give error's `{via}`
    // slot names the class-mate that was given.
    assert!(
        stderr.contains("which is what was given away"),
        "m8 gallery must include the same-call alias-pair use-after-give; got:\n{stderr}"
    );
    // Every rendered diagnostic points at its own trigger line, never into a `// WHY:` comment
    // (fix round 2, Producer A: byte spans were read as char offsets).
    for line in stderr.lines() {
        if line.contains(" │ ") && line.contains("// WHY:") && !line.contains("─▶") {
            panic!("a diagnostic's caret line landed inside a `// WHY:` comment:\n{line}\n\nfull stderr:\n{stderr}");
        }
    }
    // The unknown-method list split per receiver: channel gains close(), handle does not.
    assert!(
        stderr.contains("Available methods: send(value), receive(), close()."),
        "m8 gallery must list close() among a channel's methods; got:\n{stderr}"
    );
    assert!(
        stderr.contains("Available methods: send(value), receive()."),
        "m8 gallery must keep the handle's method list without close(); got:\n{stderr}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// v0.2.1-M10 — Pirates-roster demo build: zero spurious unused-import warnings
// ─────────────────────────────────────────────────────────────────────────────

// WHY: Proves that the six M10 typeck inserts (Bugs 1 + 2.1–2.5) hold in a
// realistic multi-file project. The pirates-roster demo imports ScheduleDay,
// Announceable, StripeDistrictEvent, and StatCategory and uses each via an AST
// position that previously triggered a spurious "imported but never used" warning.
// If this test flips to FAIL, one of the Phase 0 fixes regressed.
#[test]
fn pirates_roster_demo_builds_with_zero_m10_pattern_warnings() {
    let demo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/pirates-roster");

    let out = Command::new(ynz_binary())
        .args(["build", demo_dir.join("entrypoint.ynz").to_str().unwrap()])
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz binary");

    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();

    // Build must succeed — compile errors mean a pattern isn't expressible.
    assert!(
        out.status.success(),
        "pirates-roster demo build must exit 0; stderr:\n{stderr}"
    );

    // The four M10-pattern symbols must NOT appear in any UnusedImport warning.
    // These are the symbols exercised via the six previously-false-positive AST
    // positions (options-variant, is-narrowing, dynamic, field-type, module-const,
    // generic-field). A warning here means a Phase 0 insert regressed.
    for symbol in &[
        "ScheduleDay",
        "Announceable",
        "StripeDistrictEvent",
        "StatCategory",
    ] {
        let pattern = format!("`{symbol}` is imported but never used");
        assert!(
            !stderr.contains(&pattern),
            "M10 regression: spurious unused-import warning for `{symbol}` in pirates-roster demo.\
             \nfull stderr:\n{stderr}"
        );
    }

    // Capture the warning-class signatures (path-stripped) as a snapshot. Strips
    // absolute paths so the snapshot is stable across checkouts and worktrees.
    // Pre-existing genuine warnings (newPirate, announce, clamp, square, dead-code)
    // must appear; the M10 symbols (ScheduleDay, Announceable, StripeDistrictEvent,
    // StatCategory) must NOT appear. The snapshot pins the warning set so any
    // new spurious warning is caught as a diff.
    let warning_lines: Vec<&str> = stderr
        .lines()
        .filter(|l| l.starts_with("Warning:"))
        .collect();
    let warnings_only = warning_lines.join("\n");
    insta::assert_snapshot!("pirates_roster_demo_warning_lines", warnings_only);
}

// WHY: v0_3_m1_errors.ynz exercises every new v0.3-M1 error/warning class:
// share-param carry-forward, lend-cross-thread, large-copy warning, and the
// happy-path fire-and-forget. If error count changes, either a new diagnostic
// class was added (update count + key-phrase) or something regressed (fix it).
#[test]
fn v0_3_m1_gallery_fires_expected_diagnostics() {
    let (stderr, code) = compile_gallery(&gallery("v0_3_m1_errors.ynz"));
    // Gallery has intentional errors; must exit non-zero.
    assert_ne!(code, 0, "v0_3_m1 gallery must exit non-zero");

    // Count compile errors (warnings are separate).
    let error_count = count_errors(&stderr);
    let warning_count = stderr.lines().filter(|l| l.starts_with("Warning:")).count();

    // Expected: 3 errors (no-entrypoint + share-param + lend-cross-thread) and 1 warning (large-copy).
    assert!(
        (2..=5).contains(&error_count),
        "v0_3_m1 gallery must produce 2–5 errors; got {error_count}.\nstderr:\n{stderr}"
    );
    assert_eq!(
        warning_count, 1,
        "v0_3_m1 gallery must produce exactly 1 warning (large-copy); got {warning_count}.\nstderr:\n{stderr}"
    );

    // Key-phrase checks — one per error/warning class.
    assert!(
        stderr.contains("borrows its arguments"),
        "v0_3_m1 gallery must include share-param diagnostic; got:\n{stderr}"
    );
    assert!(
        stderr.contains("mutates its arguments via `lend`"),
        "v0_3_m1 gallery must include lend-cross-thread diagnostic; got:\n{stderr}"
    );
    assert!(
        stderr.contains("Copying") && stderr.contains("bytes into a background task"),
        "v0_3_m1 gallery must include large-copy warning; got:\n{stderr}"
    );
}

// WHY: v0_3_m3b_errors.ynz exercises every mutate-through-`share`-param error class
// M3b added — the soundness floor for auto-parallelization (a `share` argument MUST be
// a read-only borrow, or the independence analysis is unsound). The five classes are:
// direct field mutation, direct element assign, collection-method mutation, `share self`
// receiver mutation, transitive-through-bare-callee, and share→explicit-lend escalation.
// If the error count drops, a class regressed (the auto-parallel soundness premise is
// silently broken). If it rises, a new class was added without a key-phrase check here.
#[test]
fn v0_3_m3b_gallery_fires_expected_diagnostics() {
    let (stderr, code) = compile_gallery(&gallery("v0_3_m3b_errors.ynz"));
    // Gallery has intentional errors; must exit non-zero.
    assert_ne!(code, 0, "v0_3_m3b gallery must exit non-zero");

    let error_count = count_errors(&stderr);
    // Expected 8 errors: no-entrypoint + 5 share-param classes, where the escalation
    // class emits TWO diagnostics (the transitive fixpoint catch AND the explicit-lend
    // escalation catch). Range gives headroom for incidental diagnostic refinements
    // without masking a class regression.
    assert!(
        (6..=10).contains(&error_count),
        "v0_3_m3b gallery must produce 6–10 errors; got {error_count}.\nstderr:\n{stderr}"
    );

    // Key-phrase checks — one per mutate-through-`share`-param class.
    // Direct field mutation (`pickle.quantity = ...`).
    assert!(
        stderr.contains("`pickle` is declared `share`")
            && stderr.contains("fields cannot be changed"),
        "v0_3_m3b gallery must include direct-field share-mutation diagnostic; got:\n{stderr}"
    );
    // Transitive-through-bare-callee (`relayOrder` passes `ticket` to bare `stamp`).
    assert!(
        stderr.contains("`ticket`") && stderr.contains("modified through `stamp`"),
        "v0_3_m3b gallery must include transitive share-mutation diagnostic; got:\n{stderr}"
    );
    // Collection-method mutation (`lineItems.add(42)`).
    assert!(
        stderr.contains("`lineItems` is declared `share`")
            && stderr.contains("elements cannot be changed"),
        "v0_3_m3b gallery must include collection-method share-mutation diagnostic; got:\n{stderr}"
    );
    // `share self` receiver mutation (`self.quantity = ...`).
    assert!(
        stderr.contains("`self` is declared `share`")
            && stderr.contains("fields cannot be changed"),
        "v0_3_m3b gallery must include share-self receiver-mutation diagnostic; got:\n{stderr}"
    );
    // Direct element assign (`prices[0] = 99`).
    assert!(
        stderr.contains("`prices` is declared `share`") && stderr.contains("elements cannot be changed"),
        "v0_3_m3b gallery must include direct-element-assign share-mutation diagnostic; got:\n{stderr}"
    );
    // share→explicit-lend escalation (`voucher` passed to `lend`-param `applyDiscount`).
    assert!(
        stderr.contains("`voucher`") && stderr.contains("needs to modify it (`lend`)"),
        "v0_3_m3b gallery must include share→explicit-lend escalation diagnostic; got:\n{stderr}"
    );
}

// WHY: the v0.3-M4 error gallery is the hands-on review surface for the channel<T> diagnostics —
// Phase 1 construction (5 classes) + Phase 2 send/recv method surface and handle-form (8 classes).
// If the error count drops, a diagnostic class regressed; if it rises, a new class was added
// without a key-phrase check here. Closed-channel `send()` is a RUNTIME typed error and is
// structurally unreachable from v0.3 source (documented in the gallery file, proven at the
// runtime substrate) — no compile trigger exists for it by design.
#[test]
fn v0_3_m4_gallery_fires_expected_diagnostics() {
    let (stderr, code) = compile_gallery(&gallery("v0_3_m4_errors.ynz"));
    assert_ne!(code, 0, "v0_3_m4 gallery must exit non-zero");

    let error_count = count_errors(&stderr);
    // Expected 15: 5 construction (non-positive ×2, wrong capacity type, missing element type,
    // too-many-args) + 7 method-surface (unknown method, send wrong element type, receive-with-
    // args, nested-expression position, unnamed receiver, non-derivable receiver origin,
    // unsupported element type) + 2 handle (non-suspending callee, send-with-no-channel-param)
    // + 1 Phase-3 auto-Arc boundary (generic share-param callee across `background` — the
    // closed silent-skip gap). Small headroom for incidental diagnostic refinements; the key
    // phrases below pin each class individually.
    assert!(
        (14..=17).contains(&error_count),
        "v0_3_m4 gallery must produce 14–17 errors; got {error_count}.\nstderr:\n{stderr}"
    );

    // ── Phase 1: construction classes ──
    // Non-positive capacity (bounded-by-construction, stdlib-design Rule 4).
    assert!(
        stderr.contains("capacity must be at least 1"),
        "v0_3_m4 gallery must include non-positive-capacity diagnostic; got:\n{stderr}"
    );
    // Wrong capacity type.
    assert!(
        stderr.contains("capacity must be an `int`"),
        "v0_3_m4 gallery must include wrong-capacity-type diagnostic; got:\n{stderr}"
    );
    // Missing element type.
    assert!(
        stderr.contains("`channel` needs an element type"),
        "v0_3_m4 gallery must include missing-element-type diagnostic; got:\n{stderr}"
    );
    // Too many arguments.
    assert!(
        stderr.contains("takes at most one argument"),
        "v0_3_m4 gallery must include too-many-args diagnostic; got:\n{stderr}"
    );

    // ── Phase 2: send/recv method-surface classes ──
    // Unknown conduit method (names the two available methods).
    assert!(
        stderr.contains("does not have a method called `push`"),
        "v0_3_m4 gallery must include unknown-conduit-method diagnostic; got:\n{stderr}"
    );
    // Send with the wrong element type — AND the mandated backpressure teaching text
    // (IMP-no-function-coloring: a suspended producer is backpressure working, not a deadlock).
    assert!(
        stderr.contains("carries `int` values, but you're sending `string`"),
        "v0_3_m4 gallery must include send-wrong-element-type diagnostic; got:\n{stderr}"
    );
    assert!(
        stderr.contains("backpressure working, not a deadlock"),
        "v0_3_m4 gallery must carry the backpressure teaching text in the send diagnostic; got:\n{stderr}"
    );
    // receive() with arguments.
    assert!(
        stderr.contains("`.receive()` takes no arguments"),
        "v0_3_m4 gallery must include receive-takes-no-arguments diagnostic; got:\n{stderr}"
    );
    // Statement-position discipline (channel-op-expression-position registry entry).
    assert!(
        stderr.contains("can suspend — it must be its own statement"),
        "v0_3_m4 gallery must include statement-position diagnostic; got:\n{stderr}"
    );
    // Named-binding receiver discipline.
    assert!(
        stderr.contains("needs the channel held in a named binding"),
        "v0_3_m4 gallery must include named-binding-receiver diagnostic; got:\n{stderr}"
    );
    // Receiver ORIGIN discipline (fix-loop): a channel from a shape field / collection
    // element / loop variable / cross-module call must be bound with a `channel<T>`
    // annotation — unannotated it is invisible to the may-block resolver and rejected
    // (previously an under-approximation that ICEd in codegen).
    assert!(
        stderr.contains("can't see where `line` got its channel"),
        "v0_3_m4 gallery must include the receiver-origin diagnostic; got:\n{stderr}"
    );
    // Unsupported element type (sender-stack dangling class: shape/number).
    assert!(
        stderr.contains("cannot cross a task boundary"),
        "v0_3_m4 gallery must include unsupported-element-type diagnostic; got:\n{stderr}"
    );

    // ── Phase 2: handle-form classes ──
    // Non-suspending callee (background-handle-nonsuspending-callee registry entry).
    assert!(
        stderr.contains("never suspends — the handle form needs a suspending function"),
        "v0_3_m4 gallery must include non-suspending-callee handle diagnostic; got:\n{stderr}"
    );
    // h.send() on a task whose function takes no channel parameter.
    assert!(
        stderr.contains("This task takes no channel"),
        "v0_3_m4 gallery must include send-with-no-channel-param diagnostic; got:\n{stderr}"
    );

    // ── Phase 3: auto-Arc boundary class ──
    // Generic share-param callee across `background` — the silent-skip gap Phase 3 closed.
    // The reject fires with the same "borrows its arguments" phrase as a concrete callee,
    // proving the generic callee is now covered by the ONE boundary predicate.
    assert!(
        stderr.contains("borrows its arguments"),
        "v0_3_m4 gallery must include the auto-Arc boundary (generic share-param) diagnostic; got:\n{stderr}"
    );

    // ── Phase 4: the two [[lint_rule]] Tier 3 lints (SUGGESTION severity — must be
    //    present in stderr WITHOUT raising the Error count above: a lint is a teaching
    //    surface, never a gate) ──
    // False-sharing padding declined on an exported shape — carries the rule code (the
    // caret tag renders the registry name), the shape name, the decline reason, and the
    // cache-line teaching text.
    assert!(
        stderr.contains("lint: cross-thread-fields-not-padded"),
        "v0_3_m4 gallery must fire cross-thread-fields-not-padded with its rule code; got:\n{stderr}"
    );
    assert!(
        stderr.contains("`PalletLabel`") && stderr.contains("it is exported"),
        "the padding-decline lint must name the shape and the export reason; got:\n{stderr}"
    );
    assert!(
        stderr.contains("cache line"),
        "the padding-decline lint must teach the cache-line mechanism; got:\n{stderr}"
    );
    // Blocking-sleep nudge — carries the rule code and echoes the user's own literal
    // milliseconds into the WHAT-INSTEAD.
    assert!(
        stderr.contains("lint: prefer-yielding-sleep"),
        "v0_3_m4 gallery must fire prefer-yielding-sleep with its rule code; got:\n{stderr}"
    );
    assert!(
        stderr.contains("wait sleep(5)"),
        "prefer-yielding-sleep must echo the literal ms into WHAT-INSTEAD; got:\n{stderr}"
    );
}

// WHY: v0_3_m7_errors.ynz exercises the aliasing-call rejection class v0.3-M7 Phase 2
// added (FRAGO 002): a call passing the same value — or overlapping pieces of one
// value — into two parameter positions where at least one position modifies it is an
// ownership-contract violation caught at compile time (`lend` = exclusive access).
// Three triggers: share+lend same value, lend+lend same value, whole+part overlap.
// If the error count drops, a trigger regressed (the miscompile class the rejection
// closes — false LLVM `noalias` under an optimizing pipeline — is reachable again).
#[test]
fn v0_3_m7_gallery_fires_expected_diagnostics() {
    let (stderr, code) = compile_gallery(&gallery("v0_3_m7_errors.ynz"));
    // Gallery has intentional errors; must exit non-zero.
    assert_ne!(code, 0, "v0_3_m7 gallery must exit non-zero");

    let error_count = count_errors(&stderr);
    // Expected 3 errors (one per trigger block). Range gives headroom for incidental
    // diagnostic refinements without masking a trigger regression.
    assert!(
        (3..=5).contains(&error_count),
        "v0_3_m7 gallery must produce 3–5 errors; got {error_count}.\nstderr:\n{stderr}"
    );

    // share + lend, same value.
    assert!(
        stderr.contains("passed to `copyQuantity` twice in the same call")
            && stderr.contains("`share` (a read-only view)")
            && stderr.contains("`lend` (the function modifies it)"),
        "v0_3_m7 gallery must include the share+lend aliasing diagnostic; got:\n{stderr}"
    );
    // lend + lend, same value.
    assert!(
        stderr.contains("passed to `swapQuantities` twice in the same call"),
        "v0_3_m7 gallery must include the lend+lend aliasing diagnostic; got:\n{stderr}"
    );
    // Whole + part overlap (`order.slip` is part of `order`).
    assert!(
        stderr.contains("`order.slip` is part of `order`"),
        "v0_3_m7 gallery must include the whole-vs-part overlap diagnostic; got:\n{stderr}"
    );
    // Teaching shape: the copyable fix and the non-circular WHY must render.
    assert!(
        stderr.contains(".copy()") && stderr.contains("only way that value is reached"),
        "v0_3_m7 aliasing diagnostic must carry its WHAT-INSTEAD/WHY teaching text; got:\n{stderr}"
    );
}
