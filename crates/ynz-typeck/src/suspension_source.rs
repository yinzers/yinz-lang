//! THE authoritative suspension-source classifier — the single home for the question
//! "does a call to this callee introduce a base (leaf) suspension point?"
//!
//! # Why this module exists (R6 — authoritative-derivation)
//!
//! The leaf set of base suspension-source intrinsics was historically DEFINED TWICE with no
//! compile-time link — once in [`crate::intrinsics`] (consumed by typeck) and once, byte-for-byte
//! copied, in `ynz-codegen`'s `emit.rs` (consumed by codegen). That is the exact twin-computation
//! drift class that shipped silent miscompiles across M3a/M3d/M3e/M3g (see
//! `.claude/rules/authoritative-derivation.md` and the M3g AAR, commit `bb2281d`). M4 must extend
//! the suspension set with channel operations — a fifth hand-extension of two parallel lists is the
//! banned pattern.
//!
//! This module is the unification: exactly ONE definition of the base suspension-source
//! classification, exported from `ynz-typeck` and consumed by BOTH typeck and codegen
//! (`ynz-codegen` already depends on `ynz-typeck`, so no dependency inversion is needed).
//! `ynz-codegen` MUST NOT re-derive its own copy — it threads [`is_base_suspension_intrinsic`].
//!
//! # What "base" means
//!
//! A *base* suspension source is a LEAF: a call that is inherently a suspension point on its own,
//! not because it transitively calls another suspending function. The transitive propagation up the
//! call graph (a function `suspends` if it reaches a base source) is a SEPARATE, downstream concern
//! owned by [`crate::may_block`], which SEEDS its fixpoint from this classifier. This module answers
//! only the leaf question; it never re-implements the transitive analysis.
//!
//! # Extension points — add new suspension sources HERE, never a second list
//!
//! When a new kind of suspension source is introduced, its classification is added to THIS module
//! and threaded from here into every consumer. The two known future extensions:
//!
//! - **Channel methods-on-a-value (v0.3-M4 Phase 1).** `channel.send()` on a full channel and
//!   `channel.receive()` on an empty channel suspend the calling task. These are method calls on a
//!   value, not named free-function intrinsics, so the name-keyed [`is_base_suspension_intrinsic`]
//!   below does not express them — Phase 1 adds a sibling classifier arm HERE (e.g. keyed on the
//!   receiver type + method name) and threads it into the same consumers, so there is still exactly
//!   ONE authoritative home for "is this a base suspension source?", never a channel-specific list
//!   scattered into codegen or admission.
//! - **FFI `may-block` foreign functions (future — no `foreign` keyword exists yet).** When the
//!   `foreign function ... may-block` surface ships (per IMP-no-function-coloring "FFI annotation
//!   requirement"), a foreign function's declared `may-block` flag makes its calls base suspension
//!   sources. That flag is classified HERE too.

/// THE authoritative set of base (leaf) suspension-source intrinsic names.
///
/// This is the SINGLE definition. No other crate or module may define a second copy — a build-
/// blocking tripwire (`ynz-typeck/tests/suspension_source_single_definition.rs`) fails the build if
/// a second hardcoded copy of this list appears anywhere in `ynz-typeck` or `ynz-codegen` source.
///
/// - `sleep` — the yielding sleep intrinsic (hands the thread back to the scheduler).
/// - `__testFallibleAsync` — an internal-only test intrinsic (never user-visible).
///
/// Both are name-keyed free-function intrinsics. Channel methods and FFI `may-block` (the future
/// extensions documented at the module level) are NOT names in this list — they are classified by
/// sibling arms added to this module, not by growing this constant.
pub const BASE_SUSPENSION_INTRINSICS: &[&str] = &["sleep", "__testFallibleAsync"];

/// THE authoritative classifier: does a call to the free-function named `name` introduce a base
/// (leaf) suspension point?
///
/// This is the primary public API — every consumer (typeck admission, the may-block fixpoint seed,
/// codegen state-machine selection, suspension-point counting) threads THIS function rather than
/// re-deriving membership against a local copy of [`BASE_SUSPENSION_INTRINSICS`].
///
/// Name-keyed only: a user-defined function that shadows an intrinsic name (`function sleep() {...}`
/// that does not itself call the intrinsic) is NOT a base suspension source. Callers that must honor
/// shadowing check `local_fns` FIRST (see [`crate::may_block`]) — this classifier answers the pure
/// leaf-intrinsic membership question, leaving shadow resolution to the caller that has the scope.
#[inline]
pub fn is_base_suspension_intrinsic(name: &str) -> bool {
    BASE_SUSPENSION_INTRINSICS.contains(&name)
}
