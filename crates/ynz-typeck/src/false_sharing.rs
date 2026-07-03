//! False-sharing auto-padding analysis (v0.3-M4 Phase 4) — the ONE derivation of
//! "which shape types get 64-byte cache-line padding?"
//!
//! # The one authoritative source (authoritative-derivation.md)
//!
//! Cross-thread access in v0.3 is created by exactly one mechanism: a `background`
//! spawn (bare fire-and-forget or the handle form — both route through the single
//! `Expr::Background` arm in `check.rs`, the same arm that owns the borrow rejects
//! and the give/copy/channel-share boundary classification). That arm RECORDS the
//! shape types whose values cross the spawn boundary; this module derives the padded
//! set from that record in ONE place, and every consumer threads the result:
//!
//! - **codegen layout** (`ynz-codegen::shape_types::emit_shape_types`) pads exactly
//!   the shapes in `TypedModule::cross_thread_padded_shapes`;
//! - **frame-slot sizing** (`ynz-codegen`'s `frame_layouts_query`) measures the SAME
//!   padded LLVM struct types, so frame regions grow with the layout automatically;
//! - **the `cross-thread-fields-not-padded` Tier 3 lint** fires from the SAME
//!   partition's decline bucket — the lint and the transform can never disagree.
//!
//! No consumer re-derives "does this shape cross a thread boundary?" — that is the
//! four-milestone twin-drift corpse this discipline exists to prevent.
//!
//! # The `--no-auto-parallel` gate (first layout transform this flag gates)
//!
//! Under `--no-auto-parallel` (and under `--kernel`, where `background` is a compile
//! error and nothing is ever recorded) this analysis yields the EMPTY set and fires
//! no lint: the flag selects the sequential cross-impl oracle posture, so the padded
//! and unpadded lowerings must stay byte-identical in observable output while the
//! sequential layout stays genuinely unpadded. The gate reads
//! [`crate::queries::no_auto_parallel_env`] — the SAME single kill-switch predicate
//! the CPU-promotion query and codegen lowering read — never a second env-var parse.
//!
//! # Why "visible outside this file" declines padding
//!
//! Each Yinz module compiles to its own object file, and this analysis runs
//! per-module. A shape whose layout another module can also emit — an exported
//! shape, an imported shape, or a structurally-named inline (`__anon__*`) shape —
//! would get two different layouts in one linked program if only the spawning module
//! padded it: a silent memory-corruption class. So padding applies only to shapes
//! declared in THIS module and not exported; everything else in the cross-thread set
//! is declined loudly via the `cross-thread-fields-not-padded` lint.

use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::{Item, Module};
use ynz_diagnostics::{Diagnostic, SourceSpan};

use crate::{shapes::ShapeTable, types::Type};

/// The registry name of the decline lint (its `[[lint_rule]]` entry).
pub const CROSS_THREAD_FIELDS_NOT_PADDED: &str = "cross-thread-fields-not-padded";

/// Collect every user-defined shape name reachable inside `ty`, recursing through
/// the container types a value can cross a `background` boundary in.
///
/// Conservative-INCLUSIVE by design: over-collecting pads a shape that never truly
/// sees concurrent field writes — a safe memory-for-throughput trade (the Invariants'
/// documented direction) — while under-collecting silently forfeits the isolation.
///
/// Time: O(N) where N = type nodes under `ty`.  Space: O(S) collected names.
pub fn collect_shape_type_names(ty: &Type, out: &mut HashSet<String>) {
    match ty {
        Type::Shape { name } => {
            out.insert(name.clone());
        }
        Type::BuiltinChannel { elem } => collect_shape_type_names(elem, out),
        Type::BuiltinArray { elem } => collect_shape_type_names(elem, out),
        Type::BuiltinFixed { elem, .. } => collect_shape_type_names(elem, out),
        Type::Maybe { inner } => collect_shape_type_names(inner, out),
        Type::ErrorsCapable { inner } => collect_shape_type_names(inner, out),
        Type::BuiltinMap { key, val } | Type::MapEntry { key, val } => {
            collect_shape_type_names(key, out);
            collect_shape_type_names(val, out);
        }
        Type::Generic { args, .. } => {
            for a in args {
                collect_shape_type_names(a, out);
            }
        }
        // Scalars, strings, ranges, dynamic fat pointers, type params, handles,
        // nothing/error: no user shape layout to pad at this node.
        _ => {}
    }
}

/// The finalized false-sharing partition for one module.
pub struct FalseSharingOutcome {
    /// Shape names the codegen layout MUST pad (64-byte cache-line field isolation).
    pub padded: HashSet<String>,
    /// One `cross-thread-fields-not-padded` lint per declined shape.
    pub lints: Vec<Diagnostic>,
}

/// Partition the recorded cross-thread shape set into padded vs declined-with-lint.
///
/// `cross_thread` is the raw record from the `Expr::Background` arm: shape name → the
/// first spawn-site span that crossed it (the lint's anchor). Deterministic output:
/// shapes are processed in sorted-name order, one lint per shape regardless of how
/// many spawn sites crossed it.
///
/// Time: O(S · I) where S = cross-thread shapes, I = module items (declaration scan).
/// Space: O(S).
pub fn finalize_false_sharing(
    cross_thread: &HashMap<String, SourceSpan>,
    module: &Module,
    shape_table: &ShapeTable,
    kernel_mode: bool,
) -> FalseSharingOutcome {
    let mut outcome = FalseSharingOutcome {
        padded: HashSet::new(),
        lints: Vec::new(),
    };

    // Kernel mode has no scheduler (background is a compile error — nothing records),
    // and --no-auto-parallel selects the sequential oracle posture: both yield the
    // empty set, so padding self-gates off and the lint stays silent.
    if kernel_mode || crate::queries::no_auto_parallel_env() || cross_thread.is_empty() {
        return outcome;
    }

    let mut names: Vec<&String> = cross_thread.keys().collect();
    names.sort();

    for name in names {
        // Unknown to the shape table (e.g. an unresolved generic instantiation that an
        // earlier error already diagnosed): nothing to lay out, nothing to teach.
        if shape_table.get(name).is_none() {
            continue;
        }

        let local_decl = module.items.iter().find_map(|item| match item {
            Item::ShapeDecl(s) if s.name == *name => Some(s),
            _ => None,
        });

        let decline_reason: Option<&str> = if name.starts_with("__anon_") {
            // Structural naming means every file using the same inline structure emits
            // the same type — its layout is cross-file by construction.
            Some("it is an inline shape whose layout is shared by structure across files")
        } else {
            match local_decl {
                Some(decl) if decl.is_exported => Some("it is exported"),
                Some(_) => None, // module-local, not exported → paddable
                None => Some("it is declared in another file"),
            }
        };

        match decline_reason {
            None => {
                outcome.padded.insert(name.clone());
            }
            Some(reason) => {
                let span = cross_thread[name].clone();
                let vars: HashMap<&str, &str> =
                    HashMap::from([("shape", name.as_str()), ("reason", reason)]);
                let diag =
                    crate::lints::lint_diagnostic(CROSS_THREAD_FIELDS_NOT_PADDED, span, &vars)
                        .unwrap_or_else(|| {
                            panic!(
                                "[[lint_rule]] `{CROSS_THREAD_FIELDS_NOT_PADDED}` missing from \
                         registry/features.toml — the firing site and the registry drifted"
                            )
                        });
                outcome.lints.push(diag);
            }
        }
    }

    outcome
}
