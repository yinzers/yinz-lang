// v0.3-M8 Phase 4 (parked items 7/8): the `[[diagnostic_template]]` ↔ `DiagnosticKind`
// parity gate. The `Consumed` template was DEAD DATA for two milestones — its emit site
// hand-wrote a twin that drifted in all three slots and nothing noticed, because no test
// linked a template's `kind_name` to a variant, or a variant to an emit site that renders
// the template. This file is that link, in three tiers:
//
// 1. Every template `kind_name` is classified: either it names a `DiagnosticKind` variant
//    (`DiagnosticKind::TEMPLATE_KIND_NAMES`) or it is on the pinned list of templates that
//    have NO variant (the watch-daemon and concurrency-lint templates other crates render
//    by name). A new template that is neither fails here.
// 2. Every variant-backed template is either rendered FROM the registry at its emit site
//    (`registry_diag(... DiagnosticKind::X ...)` in `check.rs` — a source-level check, the
//    same discipline `suspension_source_single_definition.rs` uses) or is on the pinned
//    ratchet of PRE-EXISTING kinds whose emit sites still hand-write their text. The ratchet
//    can only shrink: moving a kind onto the registry means deleting it from the list.
// 3. The five kinds this phase owns are rendered end-to-end: a program that triggers each one
//    produces a diagnostic whose WHAT is the template's WHAT with its slots filled — never a
//    twin.

use std::collections::BTreeSet;

use ynz_diagnostics::{DiagnosticKind, Severity};
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::check_query;

/// Templates that deliberately have no `DiagnosticKind` variant: rendered by name from
/// another crate (`ynz-watch` reads the `Watch*` entries; the concurrency lint/warning
/// templates are read by name in typeck). Pinned so a NEW template must be classified.
const TEMPLATES_WITHOUT_A_VARIANT: &[&str] = &[
    "WatchNoYinzToml",
    "WatchChildSpawnFailed",
    "WatchFsWatcherInitFailed",
    "WatchRssHardStop",
    "WatchMemoryPollingUnavailable",
    "LendAcrossThreadBoundary",
    "KernelModeRejectsBackground",
    "KernelModeRejectsWait",
    "BackgroundLargeStructCopy",
    "WaitInsideLoop",
    "LocalCrossesWait",
    "WaitOnNonMayBlockWarning",
    "WaitOnNonCallExpression",
    "UnawaitedSleepAsync",
    "WaitRequiredOnStateMachineCall",
    "StatementNestingTooDeep",
    "UnionWhereVariantExpectedNameIt",
    "UnionWhereVariantExpectedNoIsArm",
    "UnionVariantNotStorableNumberPrecision",
    "MapKeyNotStorableWideNumber",
    "MapKeyNotStorableSensitive",
];

/// PRE-EXISTING variant-backed templates whose emit sites still hand-write their text
/// (the drift class parked item 7 named). Each is a template that is NOT the source of the
/// text the user sees. This list may only shrink: rendering one of these from the registry
/// (`registry_diag`) means removing it here, and the test then holds it to the registry.
const HAND_WRITTEN_RATCHET: &[&str] = &[
    "TypeMismatch",
    "MutationOfConst",
    "NotDefined",
    "MissingField",
    "HiddenAccess",
    "ImportNotFound",
    "Borrowed",
    "MissingReturn",
    "BannedKeyword",
    "BannedJargon",
    "UnusedImport",
];

fn check_rs_source() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/check.rs");
    std::fs::read_to_string(path).expect("crates/ynz-typeck/src/check.rs is readable")
}

#[test]
fn every_template_kind_name_is_classified() {
    let variants: BTreeSet<&str> = DiagnosticKind::TEMPLATE_KIND_NAMES
        .iter()
        .copied()
        .collect();
    let no_variant: BTreeSet<&str> = TEMPLATES_WITHOUT_A_VARIANT.iter().copied().collect();
    for t in ynz_registry::diagnostic_templates() {
        assert!(
            variants.contains(t.kind_name) || no_variant.contains(t.kind_name),
            "[[diagnostic_template]] kind_name = {:?} names no DiagnosticKind variant and is not \
             on TEMPLATES_WITHOUT_A_VARIANT — classify it (add the variant, or pin it here \
             with the crate that renders it by name)",
            t.kind_name
        );
        assert!(
            !(variants.contains(t.kind_name) && no_variant.contains(t.kind_name)),
            "{:?} is pinned as variant-less but a variant exists — drop it from the pin list",
            t.kind_name
        );
    }
    // The reverse: every variant-backed kind that HAS a template … has one.
    for kind in DiagnosticKind::TEMPLATE_KIND_NAMES {
        assert!(
            ynz_registry::diagnostic_template_lookup(kind).is_some() || *kind == "BannedJargon", // rendered from [[banned_jargon]] entries, not a template
            "DiagnosticKind::{kind} has no [[diagnostic_template]] entry"
        );
    }
}

/// The `DiagnosticKind::X` variants that appear INSIDE a `registry_diag(...)` call's argument
/// list in `check.rs` — the kinds whose text really is rendered from the registry. Each call
/// is located by its `registry_diag(` token and closed at its matching `)` (parentheses
/// balanced, string literals skipped), so a `.with_kind(DiagnosticKind::X)` on a hand-written
/// diagnostic elsewhere never counts as "rendered" (the token-presence check this replaced
/// was satisfied by exactly that, and needed a hardcoded exemption to stay green).
fn kinds_rendered_via_registry_diag(src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let needle = "registry_diag(";
    let mut from = 0;
    while let Some(rel) = src[from..].find(needle) {
        let open = from + rel + needle.len() - 1;
        let args = call_argument_text(src, open);
        let mut rest = args.as_str();
        while let Some(i) = rest.find("DiagnosticKind::") {
            let after = &rest[i + "DiagnosticKind::".len()..];
            let end = after
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(after.len());
            out.insert(after[..end].to_string());
            rest = &after[end..];
        }
        from = open + args.len() + 1;
    }
    out
}

/// The text between the `(` at byte `open` and its matching `)`, skipping over string
/// literals (a `)` inside a slot value must not close the call early). Does NOT handle `//`
/// line comments or char literals (`'a'`) inside the argument list — v0.3-M8 Phase 4 fix
/// round 3, should-fix 7: rather than silently mis-scoping a `)` inside either (a comment's
/// `)` would close the call early; a char literal's `'` would fall through untouched, which
/// happens to be harmless for `'`, but the mis-scoping risk is the same class), this fails
/// loudly the moment it sees either, so a future `registry_diag(...)` call written with a
/// comment or char literal in its argument list is a compile-time-obvious test failure, not a
/// wrong parse nobody notices.
fn call_argument_text(src: &str, open: usize) -> String {
    debug_assert_eq!(&src[open..open + 1], "(");
    let mut depth = 0usize;
    let mut in_str = false;
    let mut escaped = false;
    let chars: Vec<(usize, char)> = src[open..].char_indices().collect();
    let mut idx = 0;
    while idx < chars.len() {
        let (i, c) = chars[idx];
        if in_str {
            match c {
                '\\' if !escaped => escaped = true,
                '"' if !escaped => in_str = false,
                _ => escaped = false,
            }
            idx += 1;
            continue;
        }
        match c {
            '"' => in_str = true,
            '\'' => panic!(
                "call_argument_text hit a `'` (char literal or lifetime) at byte {} in \
                 check.rs's registry_diag(...) argument list — this scanner does not handle \
                 char literals; extend it or rewrite the call site to avoid one",
                open + i
            ),
            '/' if chars.get(idx + 1).map(|(_, c)| *c) == Some('/') => panic!(
                "call_argument_text hit a `//` line comment at byte {} in check.rs's \
                 registry_diag(...) argument list — this scanner does not handle comments; \
                 extend it or rewrite the call site to avoid one",
                open + i
            ),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return src[open + 1..open + i].to_string();
                }
            }
            _ => {}
        }
        idx += 1;
    }
    panic!("unbalanced parentheses after byte {open} in check.rs");
}

#[test]
fn variant_backed_templates_are_rendered_from_the_registry_or_on_the_shrinking_ratchet() {
    let src = check_rs_source();
    let rendered_kinds = kinds_rendered_via_registry_diag(&src);
    assert!(
        !rendered_kinds.is_empty(),
        "no `registry_diag(... DiagnosticKind::X ...)` call found in check.rs — the parser drifted"
    );
    let ratchet: BTreeSet<&str> = HAND_WRITTEN_RATCHET.iter().copied().collect();
    for kind in DiagnosticKind::TEMPLATE_KIND_NAMES {
        if *kind == "BannedJargon" {
            continue;
        }
        let rendered = rendered_kinds.contains(*kind);
        let on_ratchet = ratchet.contains(kind);
        assert!(
            rendered || on_ratchet,
            "DiagnosticKind::{kind} has a registry template but check.rs neither renders it \
             through `registry_diag(... DiagnosticKind::{kind} ...)` nor lists it on \
             HAND_WRITTEN_RATCHET — a dead template (parked item 7's class)"
        );
        // A kind that is rendered must be off the ratchet (the ratchet only shrinks).
        assert!(
            !(rendered && on_ratchet),
            "DiagnosticKind::{kind} is rendered from the registry — remove it from \
             HAND_WRITTEN_RATCHET"
        );
    }
    // Every kind the parser found is a real variant with a template (the parser is not
    // picking up stray text).
    for kind in &rendered_kinds {
        assert!(
            DiagnosticKind::TEMPLATE_KIND_NAMES.contains(&kind.as_str()),
            "registry_diag renders DiagnosticKind::{kind}, which is not a template-backed variant"
        );
    }
}

fn errors_for(source: &str) -> Vec<ynz_diagnostics::Diagnostic> {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, "parity.ynz".to_string(), source.to_string());
    check_query(&db, sf)
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, Severity::Error))
        .cloned()
        .collect()
}

fn fill(template: &str, slots: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (k, v) in slots {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

fn assert_rendered(kind: DiagnosticKind, source: &str, slots: &[(&str, &str)]) {
    assert_template_rendered(kind.kind_name(), kind, source, slots);
}

/// `assert_rendered` for a variant-less sibling template (`template_name`) whose diagnostic
/// carries `kind`.
fn assert_template_rendered(
    template_name: &str,
    kind: DiagnosticKind,
    source: &str,
    slots: &[(&str, &str)],
) {
    let template = ynz_registry::diagnostic_template_lookup(template_name)
        .unwrap_or_else(|| panic!("no template for {template_name}"));
    let expected_what = fill(template.what_template, slots);
    let expected_instead = fill(template.what_instead_template, slots);
    let expected_why = fill(template.why_template, slots);
    let errors = errors_for(source);
    let hit = errors
        .iter()
        .find(|d| d.what == expected_what)
        .unwrap_or_else(|| {
            panic!(
                "{}: no diagnostic rendered the registry WHAT {expected_what:?}; got {:#?}",
                kind.kind_name(),
                errors.iter().map(|d| &d.what).collect::<Vec<_>>()
            )
        });
    let kind_name = kind.kind_name();
    assert_eq!(
        hit.what_instead, expected_instead,
        "{kind_name}: WHAT-INSTEAD drifted"
    );
    assert_eq!(hit.why, expected_why, "{kind_name}: WHY drifted");
    assert_eq!(
        hit.kind,
        Some(kind),
        "{kind_name}: the diagnostic must carry its kind"
    );
}

#[test]
fn consumed_is_rendered_from_the_registry() {
    assert_rendered(
        DiagnosticKind::Consumed,
        "function eat(give rows: array<int>) -> nothing { print(rows.count()) }\n\
         function entrypoint() -> nothing {\n\
           let rows: array<int> = [1, 2, 3]\n\
           eat(rows)\n\
           print(rows.count())\n\
         }",
        &[("name", "rows"), ("given", "rows"), ("via", "")],
    );
}

#[test]
fn consumed_through_an_alias_class_names_what_was_given_in_the_via_slot() {
    assert_rendered(
        DiagnosticKind::Consumed,
        // A class-mate is a piece view of the given value (a whole-value alias cannot exist any
        // more: `let other = rows` hands `rows` over, `ConsumedByStore`).
        "shape Bag { items: array<int> }\n\
         function eatBag(give bag: Bag) -> nothing { print(bag.items.count()) }\n\
         function entrypoint() -> nothing {\n\
           let bag: Bag = { items: [1, 2, 3] }\n\
           let view: array<int> = bag.items\n\
           eatBag(bag)\n\
           print(view.count())\n\
         }",
        &[
            ("name", "view"),
            ("given", "bag"),
            (
                "via",
                " — it shares its value with `bag`, which is what was given away",
            ),
        ],
    );
}

#[test]
fn an_alias_pair_consumed_by_the_same_call_is_reported_at_the_later_position() {
    // WHY: v0.3-M8 Phase 4 fix round 2 (Producer C1). Every call form infers all arguments
    // before any position consumes, so the consumed-read site never sees `other`; the ONE
    // place left is the transfer decision at position 1, with the pre-call snapshot telling
    // it the consume happened inside this call. RED before the fix: zero diagnostics.
    assert_rendered(
        DiagnosticKind::Consumed,
        "shape Bag { items: array<int> }\n\
         function eat2(give bag: Bag, give extra: array<int>) -> nothing { print(bag.items.count()) }\n\
         function entrypoint() -> nothing {\n\
           let bag: Bag = { items: [1, 2, 3] }\n\
           let view: array<int> = bag.items\n\
           eat2(bag, view)\n\
         }",
        &[
            ("name", "view"),
            ("given", "bag"),
            (
                "via",
                " — it shares its value with `bag`, which is what was given away",
            ),
        ],
    );
    // The mixed form: a `share` position reading what a `give` position of the same call
    // consumed is the same read after free.
    assert_rendered(
        DiagnosticKind::Consumed,
        "shape Bag { items: array<int> }\n\
         function mix(give bag: Bag, share extra: array<int>) -> nothing { print(bag.items.count()) }\n\
         function entrypoint() -> nothing {\n\
           let bag: Bag = { items: [1, 2, 3] }\n\
           let view: array<int> = bag.items\n\
           mix(bag, view)\n\
         }",
        &[
            ("name", "view"),
            ("given", "bag"),
            (
                "via",
                " — it shares its value with `bag`, which is what was given away",
            ),
        ],
    );
    // The same name twice: `{via}` is empty — it IS the name that was given.
    assert_rendered(
        DiagnosticKind::Consumed,
        "function eat2(give a: array<int>, give b: array<int>) -> nothing { print(a.count()) }\n\
         function entrypoint() -> nothing {\n\
           let rows: array<int> = [1, 2, 3]\n\
           eat2(rows, rows)\n\
         }",
        &[("name", "rows"), ("given", "rows"), ("via", "")],
    );
}

#[test]
fn a_class_consumed_before_the_call_is_not_reported_twice() {
    // `resolve_ident` reports the read at inference; the transfer decision must stay quiet.
    let errors = errors_for(
        "function eat(give rows: array<int>) -> nothing { print(rows.count()) }\n\
         function entrypoint() -> nothing {\n\
           let rows: array<int> = [1, 2, 3]\n\
           eat(rows)\n\
           eat(rows)\n\
         }",
    );
    let consumed: Vec<_> = errors
        .iter()
        .filter(|d| d.kind == Some(DiagnosticKind::Consumed))
        .collect();
    assert_eq!(
        consumed.len(),
        1,
        "exactly one use-after-give for the second `eat(rows)`; got {:#?}",
        errors.iter().map(|d| &d.what).collect::<Vec<_>>()
    );
}

#[test]
fn consumed_by_send_is_rendered_from_the_registry_with_the_via_slot() {
    assert_rendered(
        DiagnosticKind::ConsumedBySend,
        // The class-mate is a piece view of the sent value (`first` reaches `rows`).
        "function entrypoint() -> nothing {\n\
           let wire: channel<array<array<int>>> = channel<array<array<int>>>(2)\n\
           let rows: array<array<int>> = [[1, 2, 3]]\n\
           let first = rows.first()\n\
           wire.send(rows)\n\
           print(first.exists().toString())\n\
         }",
        &[
            ("name", "first"),
            ("channel", "wire"),
            ("sent", "rows"),
            (
                "via",
                " — it shares its value with `rows`, which is what was sent",
            ),
        ],
    );
}

#[test]
fn consumed_by_store_is_rendered_from_the_registry() {
    assert_rendered(
        DiagnosticKind::ConsumedByStore,
        "function entrypoint() -> nothing {\n\
           let rows: array<int> = [1, 2, 3]\n\
           let table: array<array<int>> = []\n\
           table.add(rows)\n\
           print(rows.count())\n\
         }",
        &[
            ("name", "rows"),
            ("place", "`table`"),
            ("stored", "rows"),
            ("copy_form", "table.add(rows.copy())"),
            ("via", ""),
        ],
    );
}

#[test]
fn consumed_by_store_through_a_class_mate_names_what_was_stored_in_the_via_slot() {
    assert_rendered(
        DiagnosticKind::ConsumedByStore,
        "shape Bag { items: array<int> }\n\
         function entrypoint() -> nothing {\n\
           let bag: Bag = { items: [1, 2, 3] }\n\
           let view: array<int> = bag.items\n\
           let kept: Bag = bag\n\
           print(view.count())\n\
           print(kept.items.count())\n\
         }",
        &[
            ("name", "view"),
            ("place", "`kept`"),
            ("stored", "bag"),
            ("copy_form", "let kept = bag.copy()"),
            (
                "via",
                " — it shares its value with `bag`, which is what was stored",
            ),
        ],
    );
}

#[test]
fn param_needs_give_is_rendered_from_the_registry() {
    assert_rendered(
        DiagnosticKind::ParamNeedsGive,
        "function producer(lend wire: channel<array<int>>, rows: array<int>) -> nothing {\n\
           wire.send(rows)\n\
         }\n\
         function entrypoint() -> nothing { }",
        &[
            ("name", "rows"),
            ("fn", "producer"),
            ("type", "array<int>"),
            ("modifier", "has no ownership word"),
            ("act", "sent into `wire`"),
            ("copy_form", "wire.send(rows.copy())"),
        ],
    );
}

#[test]
fn transfer_needs_copy_is_rendered_from_the_registry() {
    assert_rendered(
        DiagnosticKind::TransferNeedsCopy,
        "shape Bucket { rows: array<int> }\n\
         function eat(give rows: array<int>) -> nothing { print(rows.count()) }\n\
         function entrypoint() -> nothing {\n\
           let bucket: Bucket = { rows: [1, 2, 3] }\n\
           eat(bucket.rows)\n\
         }",
        &[
            ("expr", "bucket.rows"),
            ("act", "given to `eat`"),
            ("type", "array<int>"),
            ("reason", "a field of `bucket`"),
            ("fix", "bucket.rows.copy()"),
        ],
    );
}

#[test]
fn handle_channel_arg_needs_binding_is_rendered_from_the_registry() {
    assert_rendered(
        DiagnosticKind::HandleChannelArgNeedsBinding,
        "function makeWire() -> channel<int> {\n\
           let w: channel<int> = channel<int>(4)\n\
           return w\n\
         }\n\
         function doubler(lend commands: channel<int>) -> int errors {\n\
           let c = commands.receive()\n\
           let v = c.or(0)\n\
           return v + v\n\
         }\n\
         function entrypoint() -> nothing {\n\
           let h = background doubler(makeWire())\n\
           h.send(21)\n\
         }",
        &[("callee", "doubler"), ("expr", "makeWire()")],
    );
}

#[test]
fn message_before_failed_check_is_rendered_from_the_registry() {
    // WHY: v0.3-M8 Phase 4 fix round 3, Producer A — `.message` read on an errors-capable
    // value that was never checked with `.failed()` used to type as `string` unconditionally
    // (REF-errors.md:171-175 requires the check first). Not gated at typeck, this reached
    // codegen's `select` on a possibly-null error pointer and SIGABRT'd at runtime on any
    // not-yet-failed value.
    assert_rendered(
        DiagnosticKind::MessageBeforeFailedCheck,
        "function entrypoint() -> nothing {\n\
           let wire: channel<int> = channel<int>(2)\n\
           let ok = wire.send(9)\n\
           print(ok.message)\n\
         }",
        &[("name", "ok"), ("field", "message")],
    );
}

#[test]
fn message_is_admitted_inside_its_own_failed_check() {
    // The one shape REF-errors.md documents as legal: `.message` read INSIDE the true
    // branch of `if (name.failed())`. Must compile with NO MessageBeforeFailedCheck.
    let errors = errors_for(
        "function entrypoint() -> nothing {\n\
           let wire: channel<int> = channel<int>(2)\n\
           wire.close()\n\
           let late = wire.send(9)\n\
           if (late.failed()) {\n\
             print(late.message)\n\
           }\n\
         }",
    );
    assert!(
        !errors
            .iter()
            .any(|d| d.kind == Some(DiagnosticKind::MessageBeforeFailedCheck)),
        "a `.message` read inside `if (late.failed()) {{ ... }}` must be admitted; got:\n{:#?}",
        errors.iter().map(|d| &d.what).collect::<Vec<_>>()
    );
}

#[test]
fn message_before_failed_check_fires_on_a_shadowed_rebinding_inside_the_guard() {
    // WHY: v0.3 concurrency hardening Phase 3 (FRAGO 002 cluster C3, closes parked 33). RED
    // before the fix: `errors_failed_true_branch`/`errors_consumed`/`errors_success_narrowed`
    // keyed on the bare name "x", so a shadowing inner `let x = ...` INSIDE the guarded block
    // inherited the outer `x`'s checked status and this compiled clean, printing `""` at
    // runtime instead of refusing at compile time. GREEN after the fix: the sets are keyed by
    // `ScopeEntry::alias_class` (binding identity), so the inner `x` — a fresh alias class —
    // is never a member and is correctly refused.
    assert_rendered(
        DiagnosticKind::MessageBeforeFailedCheck,
        "function mayFail(n: int) -> string errors { return `v${n}` }\n\
         function entrypoint() -> nothing {\n\
           let x = mayFail(1)\n\
           if (x.failed()) {\n\
             let x = mayFail(3)\n\
             print(x.message)\n\
           }\n\
         }",
        &[("name", "x"), ("field", "message")],
    );
}

#[test]
fn a_for_destructure_binding_shadows_by_identity_not_just_by_type_mismatch() {
    // WHY: the plan's own note — `for ((k, v) in m)` desugars its destructure names to
    // synthetic `Stmt::Let`s (`ynz-parser`'s `parse_for_destructure`), which go through the
    // SAME binding-event path (`binding_event_origin`, a fresh `alias_class` per binding
    // event) as a source-level `let` — no special-casing needed, only verification. `v` here
    // shadows the outer `.failed()`-checked `v`; `map<string,int>`'s value type means the
    // inner `v` is `int`, which correctly refuses `.message` for an ordinary reason (`int` has
    // no fields) — a DIFFERENT diagnostic than `MessageBeforeFailedCheck`. That difference is
    // the point: it proves the inner `v`'s alias class was never a member of
    // `errors_failed_true_branch` in the first place (had the old bare-name key survived,
    // `.message` would have been evaluated as an EC field access at all, not stopped one step
    // earlier by ordinary field-lookup on `int`).
    let errors = errors_for(
        "function mayFail(n: int) -> string errors { return `v${n}` }\n\
         function entrypoint() -> nothing {\n\
           let v = mayFail(1)\n\
           let m: map<string, int> = { a: 1 }\n\
           if (v.failed()) {\n\
             for ((k, v) in m) {\n\
               print(v.toString())\n\
             }\n\
             print(v.message)\n\
           }\n\
         }",
    );
    assert!(
        !errors
            .iter()
            .any(|d| d.kind == Some(DiagnosticKind::MessageBeforeFailedCheck)),
        "the OUTER `v`, checked with `.failed()` and read AFTER the for-destructure loop's \
         scope pops, must still be admitted — the loop's own scope push/pop must not disturb \
         `errors_failed_true_branch`'s bookkeeping for a name it shadowed and released; \
         got:\n{:#?}",
        errors.iter().map(|d| &d.what).collect::<Vec<_>>()
    );
}

#[test]
fn message_is_still_admitted_for_the_outer_binding_after_a_shadow_pops() {
    // WHY: the fix must not overcorrect — after the shadowing inner block exits and its
    // alias class goes out of scope, the OUTER `x` (the one actually checked) must still be
    // admitted. Guards against a fix that empties the checked-set on any shadow rather than
    // scoping it correctly.
    let errors = errors_for(
        "function mayFail(n: int) -> string errors { return `v${n}` }\n\
         function entrypoint() -> nothing {\n\
           let x = mayFail(1)\n\
           if (x.failed()) {\n\
             { let x = mayFail(3); print(x.toString()) }\n\
             print(x.message)\n\
           }\n\
         }",
    );
    assert!(
        !errors
            .iter()
            .any(|d| d.kind == Some(DiagnosticKind::MessageBeforeFailedCheck)),
        "the outer `x`, still in scope and still checked after the inner shadow's block \
         exits, must be admitted; got:\n{:#?}",
        errors.iter().map(|d| &d.what).collect::<Vec<_>>()
    );
}

#[test]
fn ec_field_not_yet_available_is_rendered_from_the_registry() {
    // WHY: v0.3 concurrency hardening Phase 3 (FRAGO 002 singleton S1, closes parked 34). RED
    // before the fix: `.trace` inside a CORRECT `.failed()` guard compiled clean (typeck typed
    // it as `array<Frame>` unconditionally) and reached codegen's `Type::ErrorsCapable` field
    // arm, which has no lowering for anything but `.message` — an internal-error string
    // ("This is a compiler bug") reached a user who wrote a correct program. GREEN after the
    // fix: refused at compile time, with real teaching text, before codegen ever sees it.
    assert_rendered(
        DiagnosticKind::EcFieldNotYetAvailable,
        "function loadConfig() -> string errors { return `hello` }\n\
         function entrypoint() -> nothing {\n\
           let late = loadConfig()\n\
           if (late.failed()) {\n\
             print(late.trace.count().toString())\n\
           }\n\
         }",
        &[("field", "trace")],
    );
}

#[test]
fn ec_field_not_yet_available_fires_for_suggestions_and_source_too() {
    // WHY: same producer as `.trace` above — all three refused fields share ONE table
    // (`ynz_typeck::errors_fields::ec_field_lowering`). One example each closes the class
    // rather than leaving `.suggestions`/`.source` unverified.
    assert_rendered(
        DiagnosticKind::EcFieldNotYetAvailable,
        "function loadConfig() -> string errors { return `hello` }\n\
         function entrypoint() -> nothing {\n\
           let late = loadConfig()\n\
           if (late.failed()) {\n\
             print(late.suggestions.count().toString())\n\
           }\n\
         }",
        &[("field", "suggestions")],
    );
    assert_rendered(
        DiagnosticKind::EcFieldNotYetAvailable,
        "function loadConfig() -> string errors { return `hello` }\n\
         function entrypoint() -> nothing {\n\
           let late = loadConfig()\n\
           if (late.failed()) {\n\
             print(late.source.file)\n\
           }\n\
         }",
        &[("field", "source")],
    );
}

#[test]
fn ec_field_not_yet_available_does_not_shadow_the_failed_check_gate() {
    // WHY: ordering matters — an UNCHECKED read of `.trace` must still get
    // `MessageBeforeFailedCheck` (teach the `.failed()` rule first), not
    // `EcFieldNotYetAvailable`. `check_errors_field_is_lowered` only runs after
    // `check_errors_field_needs_failed_check` admits the read.
    assert_rendered(
        DiagnosticKind::MessageBeforeFailedCheck,
        "function loadConfig() -> string errors { return `hello` }\n\
         function entrypoint() -> nothing {\n\
           let late = loadConfig()\n\
           print(late.trace.count().toString())\n\
         }",
        &[("name", "late"), ("field", "trace")],
    );
}

#[test]
fn message_before_failed_check_refuses_a_non_ident_receiver() {
    // A `.message` read on a receiver that was never bound to a name can never have been
    // legally checked (`extract_failed_binding` only recognizes bare-ident receivers) —
    // always refused, with a source-shaped `{name}` rendered from the expression itself.
    assert_rendered(
        DiagnosticKind::MessageBeforeFailedCheck,
        "function freshResult() -> string errors { return `hello` }\n\
         function entrypoint() -> nothing {\n\
           print(freshResult().message)\n\
         }",
        &[("name", "freshResult()"), ("field", "message")],
    );
}

#[test]
fn union_where_variant_expected_is_rendered_from_the_registry() {
    // WHY: v0.3-M9 C1b — a union reaching a variant slot used to compile and crash; its refusal
    // text must come from the registry template, not a hand-written twin.
    assert_rendered(
        DiagnosticKind::UnionWhereVariantExpected,
        "shape Tiny { flag: boolean }\n\
         shape Big { label: string }\n\
         shape Thing = Tiny | Big\n\
         function showBig(share b: Big) -> string { return b.label }\n\
         function entrypoint() -> nothing {\n\
           let big: Big = { label: `hello` }\n\
           let thing: Thing = big\n\
           print(showBig(thing))\n\
         }",
        &[
            ("value", "thing"),
            ("union", "Tiny | Big"),
            ("variant", "Big"),
            ("needs", "`showBig` expects a `Big` here"),
        ],
    );
}

// Golden Rule 11: the WHAT-INSTEAD of `UnionWhereVariantExpected` may only name code that
// compiles today. The base template names `if (value) { is Variant => … }`; that is wrong for a
// value that is not a name, a variant that is not a shape, and a union with no shape variant
// (it is the variant's own pointer, not a tag box). Each recovery has its own registry template,
// and each suggestion below is compiled to prove it.

fn assert_compiles(source: &str) {
    let errors = errors_for(source);
    assert!(
        errors.is_empty(),
        "the suggested code must compile, got {:#?}",
        errors.iter().map(|d| &d.what).collect::<Vec<_>>()
    );
}

/// Every recovery of `UnionWhereVariantExpected` is rendered from its own registry template:
/// a variant-less sibling carries the diagnostic's kind, and the WHAT is the base template's.
fn assert_recovery_rendered(template_name: &str, source: &str, slots: &[(&str, &str)]) {
    let base = ynz_registry::diagnostic_template_lookup("UnionWhereVariantExpected")
        .expect("the base template exists");
    let template = ynz_registry::diagnostic_template_lookup(template_name)
        .unwrap_or_else(|| panic!("no template for {template_name}"));
    assert_eq!(
        template.what_template, base.what_template,
        "{template_name}: the WHAT must be the base template's"
    );
    let expected_what = fill(template.what_template, slots);
    let errors = errors_for(source);
    let hit = errors
        .iter()
        .find(|d| d.what == expected_what)
        .unwrap_or_else(|| {
            panic!(
                "{template_name}: no diagnostic rendered the WHAT {expected_what:?}; got {:#?}",
                errors.iter().map(|d| &d.what).collect::<Vec<_>>()
            )
        });
    assert_eq!(
        hit.what_instead,
        fill(template.what_instead_template, slots),
        "{template_name}: WHAT-INSTEAD drifted"
    );
    assert_eq!(
        hit.why,
        fill(template.why_template, slots),
        "{template_name}: WHY drifted"
    );
    assert_eq!(hit.kind, Some(DiagnosticKind::UnionWhereVariantExpected));
}

#[test]
fn union_where_variant_expected_base_suggestion_compiles() {
    // The `is` arm on the name, with the other variant handled, is what the base text suggests.
    assert_compiles(
        "shape Tiny { flag: boolean }\n\
         shape Big { label: string }\n\
         shape Thing = Tiny | Big\n\
         function showBig(share b: Big) -> string { return b.label }\n\
         function entrypoint() -> nothing {\n\
           let big: Big = { label: `hello` }\n\
           let thing: Thing = big\n\
           if (thing) {\n\
             is Big => { print(showBig(thing)) }\n\
             else => { print(`tiny`) }\n\
           }\n\
         }",
    );
}

#[test]
fn union_where_variant_expected_for_a_field_read_asks_for_a_name_first() {
    // `holder.inner` is an expression: an `is` arm checks a named binding only.
    assert_recovery_rendered(
        "UnionWhereVariantExpectedNameIt",
        "shape Tiny { flag: boolean }\n\
         shape Big { label: string }\n\
         shape Thing = Tiny | Big\n\
         shape Holder { inner: Tiny | Big }\n\
         function showBig(share b: Big) -> string { return b.label }\n\
         function entrypoint() -> nothing {\n\
           let big: Big = { label: `hello` }\n\
           let thing: Thing = big\n\
           let holder: Holder = { inner: thing }\n\
           print(showBig(holder.inner))\n\
         }",
        &[
            ("value", "holder.inner"),
            ("union", "Tiny | Big"),
            ("variant", "Big"),
            ("needs", "`showBig` expects a `Big` here"),
        ],
    );
    // The suggested code: name it, check the name, use it in the arm.
    assert_compiles(
        "shape Tiny { flag: boolean }\n\
         shape Big { label: string }\n\
         shape Thing = Tiny | Big\n\
         shape Holder { inner: Tiny | Big }\n\
         function showBig(share b: Big) -> string { return b.label }\n\
         function entrypoint() -> nothing {\n\
           let big: Big = { label: `hello` }\n\
           let thing: Thing = big\n\
           let holder: Holder = { inner: thing }\n\
           let checked = holder.inner\n\
           if (checked) {\n\
             is Big => { print(showBig(checked)) }\n\
             else => { print(`tiny`) }\n\
           }\n\
         }",
    );
}

#[test]
fn union_where_variant_expected_for_a_union_with_no_shape_variant_suggests_no_is_arm() {
    // `map<string, string> | nothing` is the map's own pointer: no `is` arm can name `map<..>`
    // (it does not even parse), and there is no tag box for the WHY to describe.
    let source =
        "function takesTable(share table: map<string, string>) -> string { return `ok` }\n\
         function entrypoint() -> nothing {\n\
           let table: map<string, string> = { `a`: `b` }\n\
           let maybeTable: map<string, string> | nothing = table\n\
           print(takesTable(maybeTable))\n\
         }";
    assert_recovery_rendered(
        "UnionWhereVariantExpectedNoIsArm",
        source,
        &[
            ("value", "maybeTable"),
            ("union", "map<string, string> | nothing"),
            ("variant", "map<string, string>"),
            ("needs", "`takesTable` expects a `map<string, string>` here"),
            ("others", "`nothing`"),
        ],
    );
    let diagnostic = errors_for(source).remove(0);
    assert!(
        !diagnostic.what_instead.contains("if (maybeTable)") && !diagnostic.why.contains("a box"),
        "a raw-pointer union has no `is` arm to suggest and no tag box to describe: {diagnostic:?}"
    );
    // The suggested code: let the receiving slot take the union.
    assert_compiles(
        "function describeTable(share table: map<string, string> | nothing) -> string { return `ok` }\n\
         function entrypoint() -> nothing {\n\
           let table: map<string, string> = { `a`: `b` }\n\
           let maybeTable: map<string, string> | nothing = table\n\
           print(describeTable(maybeTable))\n\
         }",
    );
}

#[test]
fn union_where_variant_expected_for_a_non_shape_variant_suggests_no_is_arm() {
    // `is int =>` does not lower (#418): the suggestion must not name it.
    let source = "function takesInt(share number: int) -> int { return number }\n\
         function entrypoint() -> nothing {\n\
           let v: int | string = 5\n\
           print(takesInt(v).toString())\n\
         }";
    assert_recovery_rendered(
        "UnionWhereVariantExpectedNoIsArm",
        source,
        &[
            ("value", "v"),
            ("union", "int | string"),
            ("variant", "int"),
            ("needs", "`takesInt` expects a `int` here"),
            ("others", "`string`"),
        ],
    );
    let diagnostic = errors_for(source).remove(0);
    assert!(
        !diagnostic.what_instead.contains("is int")
            && !diagnostic.what_instead.contains("handle each variant"),
        "no `is int` arm exists to suggest, and a receiving slot cannot handle an `int` variant: {diagnostic:?}"
    );
    // The caret label comes from the kind alone and is shown for every recovery, so it must not
    // tell the reader to `is`-check a value that no `is` arm can hand over (it printed
    // "check it with `is` first" under this very diagnostic).
    let caret_label = diagnostic
        .kind
        .as_ref()
        .expect("the diagnostic carries its kind")
        .tag();
    assert!(
        !caret_label.contains("`is`"),
        "the caret label names an `is` check no arm can offer here: {caret_label:?}"
    );
    assert_compiles(
        "function takesEither(share either: int | string) -> string { return `ok` }\n\
         function entrypoint() -> nothing {\n\
           let v: int | string = 5\n\
           print(takesEither(v))\n\
         }",
    );
}

// v0.3-M9 C1e (#421): a plain value that no union box can hold is refused in typeck with a
// registry teaching diagnostic, never at codegen's "This is a compiler bug" refusal. Each test
// renders its template from the registry and compiles the code its WHAT-INSTEAD names (Golden
// Rule 11).

#[test]
fn a_variant_the_union_box_cannot_hold_is_refused_with_its_registry_text() {
    // WHY: `Circle | number` passed typeck and died in codegen as "a compiler bug".
    let preamble = "shape Circle { radius: int }\n\
                    function describe(share value: Circle | number) -> string { return `ok` }\n";
    assert_rendered(
        DiagnosticKind::UnionVariantNotStorable,
        &format!(
            "{preamble}function entrypoint() -> nothing {{\n\
               let small: number = 5.0\n\
               print(describe(small))\n\
             }}"
        ),
        &[
            ("value", "small"),
            ("union", "Circle | number"),
            ("variant", "number"),
        ],
    );
    // The suggested code: the number in a shape, the shape as the variant.
    assert_compiles(
        "shape Circle { radius: int }\n\
         shape Boxed { value: number }\n\
         function describe(share value: Circle | Boxed) -> string { return `ok` }\n\
         function entrypoint() -> nothing {\n\
           let small: number = 5.0\n\
           let boxed: Boxed = { value: small }\n\
           print(describe(boxed))\n\
         }",
    );
}

#[test]
fn a_shorter_number_in_a_wider_number_variant_is_refused_with_its_registry_text() {
    // WHY: typeck assigns `number<10>` to `number<40>`, and the union box stored the narrow
    // value's slot address as the wide value's pointer.
    assert_template_rendered(
        "UnionVariantNotStorableNumberPrecision",
        DiagnosticKind::UnionVariantNotStorable,
        "shape Circle { radius: int }\n\
         function describe(share value: Circle | number<40>) -> string { return `ok` }\n\
         function entrypoint() -> nothing {\n\
           let small: number<10> = 5.0\n\
           print(describe(small))\n\
         }",
        &[
            ("value", "small"),
            ("value_ty", "number<10>"),
            ("union", "Circle | number<40>"),
            ("variant", "number<40>"),
        ],
    );
    assert_compiles(
        "shape Circle { radius: int }\n\
         function describe(share value: Circle | number<40>) -> string { return `ok` }\n\
         function entrypoint() -> nothing {\n\
           let amount: number<40> = 5.0\n\
           print(describe(amount))\n\
         }",
    );
}

#[test]
fn a_suspending_result_into_a_union_let_is_refused_with_its_registry_text() {
    // WHY: `let m: Mixed = wait count()` bound the raw int as the union's pointer and the first
    // `is` check read its tag through address 0x4.
    let program = |bind: &str| {
        format!(
            "shape Circle {{ name: string }}\n\
             shape Mixed = Circle | int\n\
             function count() -> int {{ wait sleep(1)\n return 4 }}\n\
             function entrypoint() -> nothing {{\n\
               {bind}\n\
             }}"
        )
    };
    assert_rendered(
        DiagnosticKind::SuspendingResultIntoUnionLet,
        &program("let m: Mixed = wait count()"),
        &[
            ("name", "m"),
            ("call", "wait count()"),
            ("union", "Circle | int"),
        ],
    );
    assert_compiles(&program(
        "let got = wait count()\n let m: Circle | int = got",
    ));
}

#[test]
fn a_suspending_result_returned_as_a_union_is_refused_with_its_registry_text() {
    // WHY: `return wait count()` from a union-returning pausing function stored the raw int in the
    // return slot; the compiler then panicked instead of teaching the recovery.
    let program = |returned: &str| {
        format!(
            "shape Circle {{ name: string }}\n\
             shape Mixed = Circle | int\n\
             function count() -> int {{ wait sleep(1)\n return 4 }}\n\
             function pick() -> Mixed {{ wait sleep(1)\n {returned} }}\n\
             function entrypoint() -> nothing {{\n\
               let m = pick()\n\
             }}"
        )
    };
    assert_rendered(
        DiagnosticKind::SuspendingResultIntoUnionReturn,
        &program("return wait count()"),
        &[("call", "wait count()"), ("union", "Circle | int")],
    );
    assert_rendered(
        DiagnosticKind::SuspendingResultIntoUnionReturn,
        &program("return count()"),
        &[("call", "count()"), ("union", "Circle | int")],
    );
    assert_compiles(&program("let got = count()\n return got"));
    assert_compiles(&program("let got = wait count()\n return got"));
}

const SAME_TWO_SHAPES: &str = "shape Tiny { flag: boolean }\n\
     shape Big { label: string }\n\
     function same<T>(share first: T, share second: T) -> T { return second }\n";

#[test]
fn argument_conflicts_with_type_parameter_is_rendered_from_the_registry() {
    // WHY: v0.3-M9 C1g — `same(big, tiny)` compiled and read a `Tiny` as a `Big`; the refusal's
    // text must come from the registry template (`cause`/`alternative` are the two slots that
    // depend on what bound the type parameter).
    assert_rendered(
        DiagnosticKind::ArgumentConflictsWithTypeParameter,
        &format!(
            "{SAME_TWO_SHAPES}function entrypoint() -> nothing {{\n\
               let big: Big = {{ label: `hello` }}\n\
               let tiny: Tiny = {{ flag: true }}\n\
               print(same(big, tiny).label)\n\
             }}"
        ),
        &[
            ("value", "tiny"),
            ("actual", "Tiny"),
            ("callee", "same"),
            ("param", "second"),
            ("typeParam", "T"),
            ("bound", "Big"),
            (
                "cause",
                "`second` and `first` share the type parameter `T`, and the argument `big` for \
                 `first` already made it `Big`",
            ),
            (
                "alternative",
                "Or give `first` a `Tiny` too, so `first` and `second` have the same type",
            ),
        ],
    );
}

#[test]
fn argument_conflicts_with_type_parameter_suggestions_compile() {
    // WHY: Golden Rule 11 — each alternative the WHAT-INSTEAD names must compile today:
    // `first` given a `Tiny` too, and the explicit type argument changed to `Tiny`.
    assert_compiles(&format!(
        "{SAME_TWO_SHAPES}function entrypoint() -> nothing {{\n\
           let tiny: Tiny = {{ flag: true }}\n\
           let other: Tiny = {{ flag: false }}\n\
           print(same(tiny, other).flag.toString())\n\
           print(same<Tiny>(tiny, other).flag.toString())\n\
         }}"
    ));
}

/// (High) The WHAT-INSTEAD of a store into a struct literal is a literal the reader can type:
/// every field is named, the stored part is `.copy()`, no `…` placeholder.
#[test]
fn consumed_by_store_into_a_struct_literal_names_a_literal_that_compiles() {
    assert_rendered(
        DiagnosticKind::ConsumedByStore,
        "shape Pair { first: array<int>, second: array<int> }\n\
         function entrypoint() -> nothing {\n\
           let numbers: array<int> = [1]\n\
           let moreNumbers: array<int> = [2]\n\
           let pair: Pair = { first: numbers, second: moreNumbers }\n\
           print(numbers.count())\n\
           print(pair.first.count())\n\
         }",
        &[
            ("name", "numbers"),
            (
                "place",
                "the literal `{ first: numbers, second: moreNumbers }`",
            ),
            ("stored", "numbers"),
            (
                "copy_form",
                "{ first: numbers.copy(), second: moreNumbers }",
            ),
            ("via", ""),
        ],
    );
}

/// (High) The same for a map literal: keys and values both written out.
#[test]
fn consumed_by_store_into_a_map_literal_names_a_literal_that_compiles() {
    assert_rendered(
        DiagnosticKind::ConsumedByStore,
        "function entrypoint() -> nothing {\n\
           let rows: array<int> = [1]\n\
           let table: map<string, array<int>> = { `first`: rows }\n\
           print(rows.count())\n\
           print(table.count())\n\
         }",
        &[
            ("name", "rows"),
            ("place", "the literal `{ `first`: rows }`"),
            ("stored", "rows"),
            ("copy_form", "{ `first`: rows.copy() }"),
            ("via", ""),
        ],
    );
}

/// (High) A literal part that is an operator expression is written out, not replaced by a
/// placeholder: the WHAT-INSTEAD names `{ first: numbers.copy(), count: (factor + 1) * 2 }`, which
/// a reader can type, not `{ first: numbers.copy(), count: this value }`.
#[test]
fn consumed_by_store_literal_with_an_operator_part_names_a_literal_that_compiles() {
    assert_rendered(
        DiagnosticKind::ConsumedByStore,
        "shape Tally { first: array<int>, count: int }\n\
         function entrypoint() -> nothing {\n\
           let numbers: array<int> = [1]\n\
           let factor: int = 3\n\
           let tally: Tally = { first: numbers, count: (factor + 1) * 2 }\n\
           print(numbers.count())\n\
           print(tally.count)\n\
         }",
        &[
            ("name", "numbers"),
            (
                "place",
                "the literal `{ first: numbers, count: (factor + 1) * 2 }`",
            ),
            ("stored", "numbers"),
            (
                "copy_form",
                "{ first: numbers.copy(), count: (factor + 1) * 2 }",
            ),
            ("via", ""),
        ],
    );
}

#[test]
fn map_key_not_storable_is_rendered_from_the_registry() {
    // WHY: v0.3-M9 C6 follow-up — a shape key, a key of any other type and a `number` of more
    // than 34 digits each get their own true reason, from one registry home. A `number` key of
    // 34 digits is a map key and gets no diagnostic. A `sensitive string` key is refused for its
    // own reason (Product Manager ruling A, C7 review r1).
    assert_rendered(
        DiagnosticKind::MapKeyNotStorable,
        "shape Point { x: int }\n\
         function entrypoint() -> nothing {\n\
           let spots: map<Point, int> = {}\n\
           print(spots.count())\n\
         }",
        &[
            ("key", "Point"),
            (
                "key_is",
                "is a shape, which holds several fields rather than one value",
            ),
            (
                "recovery",
                "Key the map by one of its fields (an `int` id or a `string` name) and keep the \
                 `Point` as the map's value: `map<string, Point>`.",
            ),
        ],
    );
    assert_rendered(
        DiagnosticKind::MapKeyNotStorable,
        "function entrypoint() -> nothing {\n\
           let rows: map<array<int>, int> = {}\n\
           print(rows.count())\n\
         }",
        &[
            ("key", "array<int>"),
            ("key_is", "is not a single value of 8 or 16 bytes"),
            (
                "recovery",
                "Key the map by a `string` or an `int` that stands for the `array<int>`, such as \
                 an id or a name.",
            ),
        ],
    );
    assert_template_rendered(
        "MapKeyNotStorableWideNumber",
        DiagnosticKind::MapKeyNotStorable,
        "function entrypoint() -> nothing {\n\
           let prices: map<number<50>, int> = {}\n\
           print(prices.count())\n\
         }",
        &[("key", "number<50>"), ("digits", "50")],
    );
    assert_template_rendered(
        "MapKeyNotStorableSensitive",
        DiagnosticKind::MapKeyNotStorable,
        "function entrypoint() -> nothing {\n\
           let secrets: map<sensitive string, int> = {}\n\
           print(secrets.count())\n\
         }",
        &[("key", "sensitive string")],
    );
    // The way out the sensitive-key refusal names, and the other keys a map holds.
    for accepted in [
        "map<number<34>, int>",
        "map<string, sensitive string>",
        "map<string, Point>",
    ] {
        assert_compiles(&format!(
            "shape Point {{ x: int }}\n\
             function entrypoint() -> nothing {{\n\
               let table: {accepted} = {{}}\n\
               print(table.count())\n\
             }}"
        ));
    }
}

#[test]
fn background_union_to_generic_parameter_is_rendered_from_the_registry() {
    // WHY: v0.3-M9 C6 — the refusal's text comes from the registry, and both ways out it names
    // (declare the union type, or pass a shape that holds the union) compile.
    let program = |callee: &str, argument: &str| {
        format!(
            "shape Circle {{ radius: float }}\n\
             shape Tag {{ name: string }}\n\
             shape Holder {{ choice: Circle | Tag }}\n\
             {callee}\n\
             function entrypoint() -> nothing {{\n\
               let tag: Tag = {{ name: `boxed` }}\n\
               let choice: Circle | Tag = tag\n\
               let other: Tag = {{ name: `kept` }}\n\
               let holder: Holder = {{ choice: other }}\n\
               background eat({argument})\n\
             }}"
        )
    };
    let generic = "function eat<T>(held: T) -> nothing { print(`ate`) }";
    assert_rendered(
        DiagnosticKind::BackgroundUnionToGenericParameter,
        &program(generic, "choice"),
        &[
            ("union", "Circle | Tag"),
            ("callee", "eat"),
            ("parameter", "held"),
        ],
    );
    assert_compiles(&program(
        "function eat(held: Circle | Tag) -> nothing { print(`ate`) }",
        "choice",
    ));
    assert_compiles(&program(generic, "holder"));
}

// ── v0.3-M9 C1h (#433): `fixed<T, N>` in signatures ─────────────────────────────────────────
//
// Each refusal is rendered from its registry template (the slots are the ones the emit site
// fills), and the code its WHAT-INSTEAD names compiles (Golden Rule 11).

#[test]
fn fixed_length_not_usable_is_rendered_from_the_registry_and_its_suggestion_compiles() {
    // WHY: `fixed<int, N>` names a length the compiler cannot read; the refusal's text must come
    // from the registry, and both ways out (a whole number, no length) must compile.
    assert_rendered(
        DiagnosticKind::FixedLengthNotUsable,
        "function total(share xs: fixed<int, N>) -> int { return 1 }\n\
         function entrypoint() -> nothing { print(1) }",
        &[
            ("written", "fixed<int, N>"),
            ("problem", "`N` is not a whole number"),
            (
                "alternative",
                "Write a whole number, such as `fixed<int, 4>`, or leave the length out: \
                 `fixed<int>`. To keep values of different types together, declare a shape with \
                 a named field for each",
            ),
            ("limit", "4294967295"),
        ],
    );
    assert_compiles(
        "function total(share xs: fixed<int, 4>) -> int { return xs.count() }\n\
         function again(share xs: fixed<int>) -> int { return xs.count() }\n\
         function entrypoint() -> nothing {\n\
           let xs: fixed<int> = [1, 2, 3, 4]\n\
           print(total(xs))\n\
           print(again(xs))\n\
         }",
    );
}

#[test]
fn fixed_length_not_allowed_here_is_rendered_from_the_registry_and_its_suggestion_compiles() {
    assert_rendered(
        DiagnosticKind::FixedLengthNotAllowedHere,
        "function entrypoint() -> nothing {\n\
           let xs: fixed<int, 3> = [1, 2, 3]\n\
           print(xs.count())\n\
         }",
        &[
            ("written", "fixed<int, 3>"),
            ("place", "on a local variable"),
            (
                "alternative",
                "Leave the length out: `fixed<int>`. The list you assign gives it its length",
            ),
        ],
    );
    assert_compiles(
        "function entrypoint() -> nothing {\n\
           let xs: fixed<int> = [1, 2, 3]\n\
           print(xs.count())\n\
         }",
    );
}

#[test]
fn fixed_length_mismatch_is_rendered_from_the_registry_and_its_suggestion_compiles() {
    assert_rendered(
        DiagnosticKind::FixedLengthMismatch,
        "function total(share xs: fixed<int, 4>) -> int { return xs.count() }\n\
         function entrypoint() -> nothing {\n\
           let xs: fixed<int> = [1, 2, 3]\n\
           print(total(xs))\n\
         }",
        &[
            (
                "what_is",
                "`xs` holds 3 items, but `total` takes exactly 4 items for `xs`",
            ),
            (
                "alternative",
                "Pass a list of 4 items, or change `xs` to `fixed<int>` so `total` takes a list \
                 of any length",
            ),
            ("element", "int"),
            ("expected", "4"),
        ],
    );
    assert_compiles(
        "function total(share xs: fixed<int>) -> int { return xs.count() }\n\
         function entrypoint() -> nothing {\n\
           let xs: fixed<int> = [1, 2, 3]\n\
           print(total(xs))\n\
         }",
    );
}

#[test]
fn fixed_length_missing_in_export_is_rendered_from_the_registry_and_its_suggestion_compiles() {
    assert_rendered(
        DiagnosticKind::FixedLengthMissingInExport,
        "export function make() -> fixed<int> {\n\
           let xs: fixed<int> = [1, 2, 3, 4]\n\
           return xs\n\
         }\n\
         function entrypoint() -> nothing { print(1) }",
        &[
            ("function", "make"),
            ("part", "return type"),
            ("written", "fixed<int>"),
            ("suggested", "fixed<int, 4>"),
        ],
    );
    assert_compiles(
        "export function make() -> fixed<int, 4> {\n\
           let xs: fixed<int> = [1, 2, 3, 4]\n\
           return xs\n\
         }\n\
         function entrypoint() -> nothing { print(1) }",
    );
}

#[test]
fn fixed_return_lengths_differ_is_rendered_from_the_registry_and_its_suggestion_compiles() {
    assert_rendered(
        DiagnosticKind::FixedReturnLengthsDiffer,
        "function pick(share flag: boolean) -> fixed<int> {\n\
           let two: fixed<int> = [1, 2]\n\
           let one: fixed<int> = [1]\n\
           if (flag) {\n\
             return two\n\
           }\n\
           return one\n\
         }\n\
         function entrypoint() -> nothing { print(pick(true).count()) }",
        &[
            ("function", "pick"),
            ("lengths", "2 items and 1 item"),
            ("first", "2 items"),
        ],
    );
    assert_compiles(
        "function pick(share flag: boolean) -> fixed<int> {\n\
           let two: fixed<int> = [1, 2]\n\
           let other: fixed<int> = [3, 4]\n\
           if (flag) {\n\
             return two\n\
           }\n\
           return other\n\
         }\n\
         function entrypoint() -> nothing { print(pick(true).count()) }",
    );
}

#[test]
fn fixed_return_length_unknown_is_rendered_from_the_registry() {
    assert_rendered(
        DiagnosticKind::FixedReturnLengthUnknown,
        "function frozen() -> fixed<int> {\n\
           let items: fixed<int> = [1, 2]\n\
           let longer = items.append(3)\n\
           return longer\n\
         }\n\
         function entrypoint() -> nothing { print(1) }",
        &[
            ("function", "frozen"),
            (
                "reason",
                "a `return` gives a list whose length is not known, for example one that comes \
                 from a shape field, from `filter`, or from a function whose own length cannot \
                 be worked out",
            ),
            ("element", "int"),
        ],
    );
}
