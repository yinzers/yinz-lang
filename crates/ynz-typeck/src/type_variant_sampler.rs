//! ONE exhaustive per-`Type`-variant sampler — the authoritative source both parity-test
//! suites that need "one representative of every `Type` variant" consume, rather than each
//! hand-maintaining its own list (v0.3-M8 Phase 4 fix round 3, should-fix 3). Before this
//! module, `crates/ynz-typeck/src/types.rs`'s `channel_elem_supported_names_match_the_predicate`
//! hand-picked its `rejected` list while `channel_elem_drop` kept a `_ => None` wildcard, and
//! `crates/ynz-codegen/src/emit.rs`'s `copy_parity_tests::all_variants` was a second,
//! independently-typed copy of the same 23-arm list with its own hand-counted
//! `TYPE_VARIANT_COUNT` — exactly the twin-derivation class `authoritative-derivation.md` bans:
//! two lists that must agree on "every `Type` variant" with nothing forcing them to.
//!
//! `all_type_variants()` returns one representative `Type` per variant; `variant_tag` names
//! which variant a value is, via a `match` with NO wildcard arm, so adding a `Type` variant
//! fails to compile here until this sampler is taught its tag — the same "exhaustiveness as a
//! forcing function" pattern `copy_lowering_arm` and `channel_elem_drop` already use for their
//! own dispatches.
//!
//! Not `#[cfg(test)]`: `ynz-codegen`'s `copy_parity_tests` consumes this as a real dependency
//! from another crate's test binary, and `cfg(test)` items never cross a crate boundary — this
//! module is real (if inert in a release build) `ynz-typeck` API surface, the same shape as any
//! other `pub` helper other crates build against.

use crate::types::Type;

/// Which `Type` variant `ty` is, as a bare name (`"Nothing"`, `"BuiltinArray"`, …). The
/// exhaustiveness driver: no `_` arm, so a new `Type` variant fails to compile here until this
/// function is taught its tag.
pub fn variant_tag(ty: &Type) -> &'static str {
    match ty {
        Type::Nothing => "Nothing",
        Type::String => "String",
        Type::Error => "Error",
        Type::Int => "Int",
        Type::Float => "Float",
        Type::Number { .. } => "Number",
        Type::Bool => "Bool",
        Type::Range { .. } => "Range",
        Type::Shape { .. } => "Shape",
        Type::Dynamic { .. } => "Dynamic",
        Type::TypeParam { .. } => "TypeParam",
        Type::Generic { .. } => "Generic",
        Type::BuiltinArray { .. } => "BuiltinArray",
        Type::BuiltinFixed { .. } => "BuiltinFixed",
        Type::Maybe { .. } => "Maybe",
        Type::BuiltinMap { .. } => "BuiltinMap",
        Type::MapEntry { .. } => "MapEntry",
        Type::BuiltinChannel { .. } => "BuiltinChannel",
        Type::BackgroundHandle { .. } => "BackgroundHandle",
        Type::Options { .. } => "Options",
        Type::Union { .. } => "Union",
        Type::ErrorsCapable { .. } => "ErrorsCapable",
        Type::Sensitive { .. } => "Sensitive",
    }
}

/// One representative value per `Type` variant, in declaration order. `variant_tag` is the
/// forcing function that keeps this list honest — `every_declared_variant_is_represented`
/// below asserts the two agree on the count and the tag set, so a variant added to `Type`
/// without a sample here (or vice versa) fails a test instead of drifting silently.
pub fn all_type_variants() -> Vec<Type> {
    vec![
        Type::Nothing,
        Type::String,
        Type::Error,
        Type::Int,
        Type::Float,
        Type::Number { precision: 34 },
        Type::Bool,
        Type::Range {
            element: Box::new(Type::Int),
            end_inclusive: false,
        },
        Type::Shape {
            name: "Player".to_string(),
        },
        Type::Dynamic {
            contract: "Damageable".to_string(),
        },
        Type::TypeParam {
            name: "T".to_string(),
        },
        Type::Generic {
            name: "Pair".to_string(),
            args: vec![Type::Int, Type::Int],
        },
        Type::BuiltinArray {
            elem: Box::new(Type::Int),
        },
        Type::BuiltinFixed {
            elem: Box::new(Type::Int),
            size: None,
        },
        Type::Maybe {
            inner: Box::new(Type::Int),
        },
        Type::BuiltinMap {
            key: Box::new(Type::String),
            val: Box::new(Type::Int),
        },
        Type::MapEntry {
            key: Box::new(Type::String),
            val: Box::new(Type::Int),
        },
        Type::BuiltinChannel {
            elem: Box::new(Type::Int),
        },
        Type::BackgroundHandle {
            result: Box::new(Type::Int),
            msg_elem: None,
        },
        Type::Options {
            name: "Status".to_string(),
        },
        Type::Union {
            variants: vec![Type::Int, Type::String],
        },
        Type::ErrorsCapable {
            inner: Box::new(Type::Int),
        },
        Type::Sensitive {
            inner: Box::new(Type::String),
        },
    ]
}

/// The variant count `all_type_variants`/`variant_tag` agree on. Bump it when a `Type` variant
/// is added — `variant_tag`'s non-exhaustive-pattern compile error is what brings you here, and
/// `every_declared_variant_is_represented` below catches a sample list that fell out of step.
pub const TYPE_VARIANT_COUNT: usize = 23;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_declared_variant_is_represented() {
        let samples = all_type_variants();
        let tags: std::collections::BTreeSet<&str> = samples.iter().map(variant_tag).collect();
        assert_eq!(
            tags.len(),
            TYPE_VARIANT_COUNT,
            "one sample per Type variant, no duplicates: {tags:?}"
        );
        assert_eq!(
            samples.len(),
            TYPE_VARIANT_COUNT,
            "all_type_variants must have exactly one entry per variant"
        );
    }
}
