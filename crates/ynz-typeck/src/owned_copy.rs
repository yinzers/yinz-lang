//! THE authoritative answer to one question: **what is an owned, independent copy of a value
//! of this type?**
//!
//! Before this module the question had two independent answers living in
//! `ynz-codegen`'s `emit.rs` — `copy_lowering_arm` (for `.copy()`) and
//! `prepare_bg_arg_for_ctx` (for a `background` argument) — and *both* defaulted to handing
//! back the receiver's own pointer: `copy_lowering_arm`'s `AliasNoOp` variant, and
//! `prepare_bg_arg_for_ctx`'s `array<pointer-elem>` branch plus its `_` arm. The two agreed
//! only by comment (the bg-arg arms cited `.copy()` as their justification), which is exactly
//! the twin-derivation class `.claude/rules/authoritative-derivation.md` bans. One live
//! use-after-free and one live silent-wrong answer came out of the pair.
//!
//! Both call sites now consume [`owned_copy_plan`]. Nothing re-derives it.
//!
//! ## The ruling this table encodes (Patrick, 2026-09-06)
//!
//! `.copy()` returns a genuinely independent value for every type where independence is
//! meaningful, and is a COMPILE ERROR where it is not. Nothing silently aliases, ever. There
//! are exactly two buckets — a real copy, or a [`CopyRefusal`] the user can read.
//!
//! "Independence is meaningful" means *the value has state something could change*. For a
//! type whose contents can never change — `int`, `string`, an `options` tag, a `range` — the
//! receiver already IS an independent value in every observable sense, so returning it is a
//! copy rather than an alias. That is [`OwnedCopy::ReceiverIsCopy`] /
//! [`OwnedCopy::FrameLocalImmutable`], not a stub.
//!
//! ## Residuals this table does NOT close, named rather than hidden
//!
//! - [`OwnedCopy::ShapeMemcpy`] copies a shape's own bytes. A field that is itself a pointer
//!   (a nested shape, an `array`, a `map`, a `maybe`) is copied as a pointer, so the copy and
//!   the original still share it. Making that deep needs a per-shape recursive clone with a
//!   matching release story; it is a separate, ratified piece of work, not something this
//!   module can pretend away.
//! - [`OwnedCopy::MapClone`] copies a map's four buffers. A value cell holding a pointer is
//!   copied as a pointer, the same one level down. Iterating a map's occupied slots from
//!   generated code is the missing machinery.
//!
//! Both residuals are pre-existing behaviour of already-shipped arms, unchanged here.
//! `array<T>` is the one container this module DOES take all the way down
//! ([`ElemCopy::Nested`]), because it is the type the use-after-free was found on.

use crate::types::{type_name, Type};

/// How to produce an owned, independent copy of a value of some type — or why one cannot
/// exist. Exhaustive over `Type` via [`owned_copy_plan`], which has no `_` arm, so a new
/// `Type` variant fails to compile until someone says what its `.copy()` does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnedCopy {
    /// The receiver's bits ARE the copy, for every consumer: an immediate (`int`, `float`,
    /// `bool`, an `options` i8 tag) or a pointer to immortal, immutable bytes (`string`,
    /// `sensitive string`). Nothing can change what is behind it, so a second name for the
    /// same bits is indistinguishable from a second value.
    ReceiverIsCopy,
    /// Immutable contents behind a pointer into the PRODUCING FRAME's storage — a
    /// `decimal128` number today. `.copy()` may hand back the receiver (same frame, nothing
    /// can mutate it); a `background` argument may NOT, because the frame dies while the task
    /// still holds the pointer, so it must be re-homed first. `heap_cell` names the re-homing
    /// mechanism the spawn path has for this type, or [`HeapCell::None`] when it has none and
    /// must refuse.
    FrameLocalImmutable { heap_cell: HeapCell },
    /// A `shape`: copy the struct's own bytes into fresh storage. See the pointer-field
    /// residual in this module's header.
    ShapeMemcpy,
    /// `fixed<T>`: copy the N inline cells into a fresh slot.
    FixedMemcpy,
    /// `array<T>`: a fresh header and buffer. `elem` says whether the element cells are
    /// finished once byte-copied or must themselves be copied.
    ArrayClone { elem: ElemCopy },
    /// `map<K, V>`: a fresh header and its four buffers. See the value-cell residual above.
    MapClone,
    /// `maybe<T>`: a fresh envelope cell, plus the payload for the inner types whose payload
    /// is itself copied by the shared maybe-ownership core.
    MaybeCellClone,
    /// No independent copy of this type exists. A compile-time refusal with teaching text.
    Refused(CopyRefusal),
}

/// Whether a container's element cells are finished once the buffer is byte-copied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElemCopy {
    /// The cell holds the whole element (an `int`, a `shape`'s inline bytes) or a pointer to
    /// immortal immutable bytes (a `string`). Byte-copying the buffer finishes the job.
    Inline,
    /// The cell holds a pointer to a separately-allocated value. The buffer copy must be
    /// followed by copying each element through this same table, or the two containers would
    /// share their items.
    Nested,
}

/// The re-homing mechanism a `background` spawn has for a [`OwnedCopy::FrameLocalImmutable`]
/// type whose storage dies with the spawner's frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeapCell {
    /// `number_to_heap_cell` — the 16-byte decimal cell the spawn path already mints.
    Number,
    /// No mechanism exists; a spawn of this type must fail loudly rather than pass a pointer
    /// that will dangle.
    None,
}

/// Why a type has no independent copy, in the three slots Golden Rule 11 requires. Rendered
/// through the `CopyNotIndependent` `[[diagnostic_template]]`, which supplies the sentence
/// shell; these are its `{detail}` / `{fix}` / `{why}` fills.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyRefusal {
    /// Trails the WHAT sentence, e.g. `" — a channel is the line two tasks talk over"`.
    /// May be empty.
    pub detail: String,
    /// The WHAT-INSTEAD slot: something the reader can do, with real Yinz in it.
    pub fix: String,
    /// The WHY slot: contextual, non-circular, no internals.
    pub why: String,
}

impl CopyRefusal {
    fn new(detail: impl Into<String>, fix: impl Into<String>, why: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            fix: fix.into(),
            why: why.into(),
        }
    }
}

/// A container refuses because its items refuse. One shared rendering so `array`, `fixed`,
/// `maybe` and `sensitive` cannot drift into four different explanations of one rule.
fn items_refuse(container: &Type, item_label: &str, item: &Type) -> CopyRefusal {
    CopyRefusal::new(
        format!(
            ", because its {item_label} are `{}` values and a `{}` cannot be copied",
            type_name(item),
            type_name(item)
        ),
        format!(
            "Build a new `{}` holding new items, or copy the pieces you actually need one at \
             a time.",
            type_name(container)
        ),
        "A copy of a collection is only separate if every item in it is separate too. Copying \
         the collection while sharing its items would leave two collections changing the same \
         items — the surprise this refusal exists to prevent."
            .to_string(),
    )
}

/// THE per-type owned-copy table. Exhaustive over `Type`, no `_` arm.
pub fn owned_copy_plan(ty: &Type) -> OwnedCopy {
    match ty {
        // ── Values whose contents can never change: the receiver is already the copy ──
        Type::Int | Type::Float | Type::Bool => OwnedCopy::ReceiverIsCopy,
        // String bytes are allocated once and never written again, and there is no operation
        // that changes a string in place, so two names for the same bytes can never disagree.
        Type::String => OwnedCopy::ReceiverIsCopy,
        // An `options` value is a small tag with nothing behind it.
        Type::Options { .. } => OwnedCopy::ReceiverIsCopy,

        // A `decimal128` number is immutable, but it lives in storage the producing frame
        // owns — fine to hand back inside one frame, not fine to hand to a task that outlives
        // it. The spawn path already re-homes it (`number_to_heap_cell`).
        Type::Number { precision } if *precision <= 34 => OwnedCopy::FrameLocalImmutable {
            heap_cell: HeapCell::Number,
        },
        // A range only carries its start and end and nothing can change them; same
        // frame-storage caveat, and no spawn-side re-homing exists for it.
        Type::Range { .. } => OwnedCopy::FrameLocalImmutable {
            heap_cell: HeapCell::None,
        },

        // ── Real copies ──────────────────────────────────────────────────────────────
        Type::Shape { .. } => OwnedCopy::ShapeMemcpy,

        Type::BuiltinFixed { elem, .. } => match elem_copy(elem) {
            Some(ElemCopy::Inline) => OwnedCopy::FixedMemcpy,
            // A `fixed` list stores its items inline in one block of cells; there is no
            // per-item pointer to follow, so an item that needs following cannot live here.
            Some(ElemCopy::Nested) | None => {
                OwnedCopy::Refused(items_refuse(ty, "items", elem.as_ref()))
            }
        },

        Type::BuiltinArray { elem } => match elem_copy(elem) {
            Some(kind) => OwnedCopy::ArrayClone { elem: kind },
            None => OwnedCopy::Refused(items_refuse(ty, "items", elem.as_ref())),
        },

        Type::BuiltinMap { .. } => OwnedCopy::MapClone,

        Type::Maybe { inner } => match owned_copy_plan(inner) {
            // The shared maybe-ownership core copies the envelope, and the payload for the
            // inner kinds it knows how to follow. An inner whose own copy needs a fresh
            // allocation is not one of those yet.
            OwnedCopy::ReceiverIsCopy
            | OwnedCopy::FrameLocalImmutable { .. }
            | OwnedCopy::ShapeMemcpy => OwnedCopy::MaybeCellClone,
            _ => OwnedCopy::Refused(items_refuse(ty, "contents", inner.as_ref())),
        },

        // A `sensitive` value is its inner value wearing a label the copy keeps, so the copy
        // is still redacted wherever it is printed. Refusing here would push people toward
        // `.reveal()` just to get a copyable value out — strictly worse for the secret. Only
        // the inner kinds whose copy IS the receiver's own bits qualify: anything needing
        // fresh storage would have to be unwrapped and re-wrapped, and `sensitive` wraps only
        // `string` today, so there is no such case to design for yet.
        Type::Sensitive { inner } => match owned_copy_plan(inner) {
            OwnedCopy::ReceiverIsCopy => OwnedCopy::ReceiverIsCopy,
            _ => OwnedCopy::Refused(items_refuse(ty, "contents", inner)),
        },

        // ── Refusals ─────────────────────────────────────────────────────────────────
        Type::BuiltinChannel { elem } => OwnedCopy::Refused(CopyRefusal::new(
            " — a channel is the line two tasks talk over, not a value you hold",
            format!(
                "Pass the channel itself to the task and every task holding it reads and \
                 writes the same line. If you want a second, separate line, make one: `let \
                 replies: channel<{0}> = channel<{0}>()`.",
                type_name(elem)
            ),
            "A second channel holding the same messages would not help you: the task on the \
             other end is listening on the first one. Anything you sent into the copy would \
             go nowhere, and nothing would tell you.",
        )),

        Type::BackgroundHandle { .. } => OwnedCopy::Refused(CopyRefusal::new(
            " — a handle names one running task",
            "Use the handle you already have; you can pass it along and call `.receive()` on \
             it wherever you need the task's answer. To get a second task, start one: `let \
             second = background worker(orders)`.",
            "There is only one task running. A second handle would still name that same task, \
             so it would not be a copy of anything — and both handles would be waiting for \
             the one answer the task sends back, where only one of them can get it.",
        )),

        Type::Dynamic { contract } => OwnedCopy::Refused(CopyRefusal::new(
            String::new(),
            format!(
                "Copy the value before you store it as a `{contract}`: `const backup = \
                 player.copy()`, then use `backup` where you need the second one."
            ),
            format!(
                "A `dynamic {contract}` value keeps which shape is really inside it until the \
                 program runs. Nothing here can know yet how big that shape is or which \
                 fields it carries, so there is no honest set of bytes to copy."
            ),
        )),

        Type::Union { .. } => OwnedCopy::Refused(CopyRefusal::new(
            String::new(),
            "Check which one it is first, then copy that: `if (order is Delivery) { const \
             backup = order.copy() }`.",
            "A value written with `|` could be either choice at this point in the program, and \
             each choice is a different size and a different set of fields. There is no single \
             set of bytes that copying could mean here.",
        )),

        // Bignum (`precision > 34`). Unreachable from source today — the parser turns a
        // wider-than-34 precision away with its own message — but classified honestly rather
        // than left to alias, since the moment it ships the alias would be silent.
        Type::Number { .. } => OwnedCopy::Refused(CopyRefusal::new(
            " with more than 34 digits of precision",
            "Use a plain `number`, which carries 34 digits: `let total: number = 12.5`. \
             Copying works on those.",
            "Numbers wider than that are still being designed and are not ready in this \
             version, so nothing here knows yet how their digits are stored — a copy could \
             not be made honestly.",
        )),

        Type::Nothing => OwnedCopy::Refused(CopyRefusal::new(
            " — there is no value here to copy",
            "Call `.copy()` on a value instead. If this came from a function call, that \
             function hands back `nothing`, so store the value you actually meant to copy in \
             a binding first.",
            "`nothing` means no value was produced at all, so there is nothing here to make a \
             second one of.",
        )),

        Type::MapEntry { .. } => OwnedCopy::Refused(CopyRefusal::new(
            " — an entry is the loop's view of one slot, not a value of its own",
            "Copy the piece you want: `const savedKey = entry.key`, or `const savedValue = \
             entry.value.copy()`.",
            "The loop rewrites that view on its next turn, so a copy of the view would stop \
             being about the slot you were looking at. Copying `entry.key` or `entry.value` \
             keeps the part you actually wanted.",
        )),

        Type::ErrorsCapable { .. } => OwnedCopy::Refused(CopyRefusal::new(
            " — it has not been checked for failure yet",
            "Check it first, then copy what came back: `if (result.failed()) { return }` and \
             then `const backup = result.copy()`.",
            "Until that check runs, this is either the answer or a failure, and those are not \
             the same thing to copy. Once it is checked the value is just the answer, and \
             copying it is ordinary.",
        )),

        // A type parameter is not a type yet. The refusal that matters fires at each call
        // site, where the real type is known — `check_postfix_op` deliberately does not
        // report this one, and codegen substitutes the concrete type before asking.
        Type::TypeParam { name } => OwnedCopy::Refused(CopyRefusal::new(
            format!(" — `{name}` is not a real type until this function is called"),
            "Call `.copy()` where the real type is known, or hand the value over with `give` \
             instead of copying it.",
            "Which type stands in for a type placeholder is decided at each call, so nothing \
             here can say what copying it would have to do.",
        )),

        Type::Generic { name, .. } => OwnedCopy::Refused(CopyRefusal::new(
            String::new(),
            format!(
                "Copy the fields you need out of it into a new `{name}` value instead of \
                 copying the whole thing."
            ),
            "This shape is built fresh for the types you filled in, and copying one is not \
             supported in this version — copying it silently while sharing its contents is \
             the thing this refusal replaces.",
        )),

        // Reached only when an earlier error already produced a diagnostic. `check_postfix_op`
        // returns before consulting this table, so the text below never reaches a user; it is
        // written honestly rather than left as a placeholder.
        Type::Error => OwnedCopy::Refused(CopyRefusal::new(
            " — the type of this value could not be worked out",
            "Fix the error reported above this one; the copy is fine once the value has a \
             type.",
            "Something earlier in this expression did not have a type the compiler could work \
             out, so there is no way to say what copying it would do.",
        )),
    }
}

/// How a container's cells behave for `ty` as its ELEMENT type: `None` when an element of
/// this type has no independent copy at all, so the container has none either.
fn elem_copy(ty: &Type) -> Option<ElemCopy> {
    match owned_copy_plan(ty) {
        // The cell holds the element outright (an int, a shape's inline bytes, an options
        // tag) or a pointer to bytes nothing can change (a string).
        OwnedCopy::ReceiverIsCopy | OwnedCopy::ShapeMemcpy => Some(ElemCopy::Inline),
        // A `number` cell is immutable bits; copying the cell copies the value.
        OwnedCopy::FrameLocalImmutable { .. } => Some(ElemCopy::Inline),
        // The cell is a pointer to a separate allocation — follow it.
        OwnedCopy::ArrayClone { .. } | OwnedCopy::MapClone | OwnedCopy::MaybeCellClone => {
            Some(ElemCopy::Nested)
        }
        // A `fixed` list is a block of cells with no header of its own; a cell holding one
        // would be holding a pointer into somebody's frame, which no copy can rescue.
        OwnedCopy::FixedMemcpy => None,
        OwnedCopy::Refused(_) => None,
    }
}

/// The `background` path's OWN re-homing mechanisms — the ones it applies before it ever asks
/// the owned-copy table, because for these types "give the task its own value" is not a copy
/// question at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnRehoming {
    /// A `channel` is SHARED with the task on purpose — both ends must be the same bounded
    /// buffer, so neither a copy nor a hand-over is right. The spawn takes a counted reference.
    ShareChannel,
    /// A map-entry loop view points at storage the loop rewrites on its next turn, so it is
    /// stabilized into its own cell regardless of what the ownership record says.
    StabilizeLoopView,
    /// A `decimal128` number lives in frame-owned storage, so it is copied into a heap cell.
    DecimalCell,
}

/// Does the `background` path re-home this type with a mechanism of its own?
///
/// ONE list, two readers: `prepare_bg_arg_for_ctx` dispatches its unconditional pre-gates off
/// this function, and [`spawn_arg_can_be_independent`] reads it to decide whether a spawn that
/// needs an independent value can get one. Before this existed the two facts were a match in
/// codegen and nothing at all in typeck, which is how a shipped-and-working `MapEntry` spawn
/// argument got refused by a rule that only knew about copying.
pub fn spawn_rehoming(ty: &Type) -> Option<SpawnRehoming> {
    match ty {
        Type::BuiltinChannel { .. } => Some(SpawnRehoming::ShareChannel),
        Type::MapEntry { .. } => Some(SpawnRehoming::StabilizeLoopView),
        Type::Number { precision } if *precision <= 34 => Some(SpawnRehoming::DecimalCell),
        _ => None,
    }
}

/// Can a `background` task be given a value of this type that the spawner does NOT share?
///
/// True when the spawn path re-homes the type itself ([`spawn_rehoming`]), or when the shared
/// owned-copy table can produce an independent copy of it. False is a compile-time refusal at
/// the spawn — the `background` face of the same ruling `.copy()` obeys.
pub fn spawn_arg_can_be_independent(ty: &Type) -> bool {
    spawn_rehoming(ty).is_some() || !matches!(owned_copy_plan(ty), OwnedCopy::Refused(_))
}

/// Is `.copy()` on a value of this type a genuinely INDEPENDENT copy — a value nobody else
/// reaches — so provenance may classify the result `Fresh`?
///
/// DERIVED from [`owned_copy_plan`], never a second predicate: every plan but
/// [`OwnedCopy::Refused`] produces an independent value, and a refused type never produces a
/// value at all (the program does not build).
pub fn copy_is_independent(ty: &Type) -> bool {
    !matches!(owned_copy_plan(ty), OwnedCopy::Refused(_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::type_variant_sampler::all_type_variants;

    #[test]
    fn every_type_variant_is_a_copy_or_a_refusal_and_never_a_silent_alias() {
        // WHY: the ruling this module encodes has exactly two buckets. A third — "hands back
        // the receiver while claiming to copy" — is the defect it replaced. `ReceiverIsCopy`
        // is NOT that third bucket: it is reserved for contents nothing can change, which
        // this test states by re-deriving the allowed set by hand.
        for ty in all_type_variants() {
            let plan = owned_copy_plan(&ty);
            let receiver_is_the_copy_is_sound = matches!(
                ty,
                Type::Int
                    | Type::Float
                    | Type::Bool
                    | Type::String
                    | Type::Options { .. }
                    | Type::Number { .. }
                    | Type::Range { .. }
                    | Type::Sensitive { .. }
            );
            if matches!(
                plan,
                OwnedCopy::ReceiverIsCopy | OwnedCopy::FrameLocalImmutable { .. }
            ) {
                assert!(
                    receiver_is_the_copy_is_sound,
                    "{ty:?}: handing back the receiver is only a copy for contents nothing \
                     can change — this type is not one of those"
                );
            }
        }
    }

    #[test]
    fn every_refusal_fills_all_three_teaching_slots() {
        for ty in all_type_variants() {
            if let OwnedCopy::Refused(r) = owned_copy_plan(&ty) {
                assert!(!r.fix.trim().is_empty(), "{ty:?}: empty WHAT-INSTEAD");
                assert!(!r.why.trim().is_empty(), "{ty:?}: empty WHY");
            }
        }
    }

    #[test]
    fn a_conduit_and_a_task_handle_are_refusals_not_copies() {
        assert!(matches!(
            owned_copy_plan(&Type::BuiltinChannel {
                elem: Box::new(Type::Int)
            }),
            OwnedCopy::Refused(_)
        ));
        assert!(matches!(
            owned_copy_plan(&Type::BackgroundHandle {
                result: Box::new(Type::Int),
                msg_elem: None,
            }),
            OwnedCopy::Refused(_)
        ));
    }

    #[test]
    fn an_array_of_arrays_copies_its_items_too() {
        // WHY: this is FR #9's own type. A one-level clone would leave the two containers
        // sharing their inner arrays, which is the alias the ruling removes one level down.
        assert_eq!(
            owned_copy_plan(&Type::BuiltinArray {
                elem: Box::new(Type::BuiltinArray {
                    elem: Box::new(Type::Int)
                })
            }),
            OwnedCopy::ArrayClone {
                elem: ElemCopy::Nested
            }
        );
        assert_eq!(
            owned_copy_plan(&Type::BuiltinArray {
                elem: Box::new(Type::Int)
            }),
            OwnedCopy::ArrayClone {
                elem: ElemCopy::Inline
            }
        );
    }

    #[test]
    fn a_fixed_list_of_ints_copies_and_pin_n_is_a_value_contract() {
        assert_eq!(
            owned_copy_plan(&Type::BuiltinFixed {
                elem: Box::new(Type::Int),
                size: Some(3),
            }),
            OwnedCopy::FixedMemcpy
        );
    }
}
