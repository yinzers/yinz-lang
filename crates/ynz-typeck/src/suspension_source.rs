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

/// THE authoritative set of method NAMES that are base suspension sources when invoked on a
/// suspension-capable conduit receiver (`channel<T>` or a background task handle).
///
/// This is the v0.3-M4 sibling arm the module doc earmarked: channel operations are method calls
/// on a VALUE, not named free-function intrinsics, so [`BASE_SUSPENSION_INTRINSICS`] cannot
/// express them. The method-name list and the classification predicate live HERE — never a
/// channel-specific list scattered into codegen, admission, or the crossing analysis.
///
/// - `send` — suspends when the channel/handle inbox is full (backpressure).
/// - `receive` — suspends when the channel is empty / the task has not yet delivered.
pub const CHANNEL_SUSPENDING_METHODS: &[&str] = &["send", "receive"];

/// THE authoritative classifier for conduit-method suspension: does calling the method named
/// `method` on a receiver introduce a base (leaf) suspension point, GIVEN that the receiver is a
/// suspension-capable conduit (`channel<T>` or a `background` task handle)?
///
/// `receiver_is_conduit` is supplied by the consumer, because receiver-type knowledge lives at
/// different layers:
///
/// - **typeck / codegen** answer it exactly from `expr_types`
///   (`Type::BuiltinChannel { .. } | Type::BackgroundHandle { .. }`).
/// - **the may-block fixpoint** (which runs before expression checking, so it has no
///   `expr_types`) answers it via the syntactic conduit-binding resolver in
///   [`crate::may_block`]. Typeck keeps the two views equivalent BY CONSTRUCTION: a
///   channel/handle suspending method is only accepted on a plain-identifier receiver whose
///   binding typeck itself derived through THE shared origin predicate
///   (`may_block::let_binds_derivable_conduit` — param annotation, `let` annotation,
///   `channel<T>()` construction, `let h = background f()` spawn, direct ident alias, or a
///   local function's declared `channel<T>` return). Both views accumulate their conduit
///   sets by calling that ONE predicate (never a second hand-synced copy —
///   authoritative-derivation.md), and every receiver whose origin the predicate does NOT
///   derive (a shape field, a collection element, a loop variable, a cross-module call —
///   unless the binding carries a `channel<T>` annotation) is a teaching compile error in
///   `check_conduit_method_call`, so the fixpoint can never under-approximate what typeck
///   accepted.
///
/// A UFCS user function that happens to be named `send`/`receive` on a NON-conduit receiver is
/// NOT a suspension source (`receiver_is_conduit == false`).
#[inline]
pub fn channel_method_suspends(receiver_is_conduit: bool, method: &str) -> bool {
    receiver_is_conduit && CHANNEL_SUSPENDING_METHODS.contains(&method)
}
