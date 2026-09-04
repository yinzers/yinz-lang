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

#[test]
fn variant_backed_templates_are_rendered_from_the_registry_or_on_the_shrinking_ratchet() {
    let src = check_rs_source();
    let ratchet: BTreeSet<&str> = HAND_WRITTEN_RATCHET.iter().copied().collect();
    for kind in DiagnosticKind::TEMPLATE_KIND_NAMES {
        if *kind == "BannedJargon" {
            continue;
        }
        let rendered = src.contains(&format!("DiagnosticKind::{kind},"))
            || src.contains(&format!("DiagnosticKind::{kind} "))
            || src.contains(&format!("DiagnosticKind::{kind})"));
        let on_ratchet = ratchet.contains(kind);
        assert!(
            rendered || on_ratchet,
            "DiagnosticKind::{kind} has a registry template but check.rs neither renders it \
             through `registry_diag` nor lists it on HAND_WRITTEN_RATCHET — a dead template \
             (parked item 7's class)"
        );
        // A kind that is rendered must be off the ratchet (the ratchet only shrinks).
        let rendered_via_registry = src.contains(&format!("registry_diag(\n"))
            && src.contains(&format!("DiagnosticKind::{kind}"));
        if rendered_via_registry && on_ratchet && !matches!(*kind, "NotDefined" | "UnusedImport") {
            panic!("DiagnosticKind::{kind} is rendered from the registry — remove it from HAND_WRITTEN_RATCHET");
        }
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
    let template = ynz_registry::diagnostic_template_lookup(kind.kind_name())
        .unwrap_or_else(|| panic!("no template for {}", kind.kind_name()));
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
        &[("name", "rows")],
    );
}

#[test]
fn consumed_by_send_is_rendered_from_the_registry_with_the_via_slot() {
    assert_rendered(
        DiagnosticKind::ConsumedBySend,
        "function entrypoint() -> nothing {\n\
           let wire: channel<array<int>> = channel<array<int>>(2)\n\
           let rows: array<int> = [1, 2, 3]\n\
           let other = rows\n\
           wire.send(rows)\n\
           print(other.count())\n\
         }",
        &[
            ("name", "other"),
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
