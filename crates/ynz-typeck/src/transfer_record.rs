//! The persisted transfer record — typeck's answer to "who holds this heap value, and may
//! anything free it here?" (v0.3-M9 C5a, plan #17; `IMP-ownership.md` "The persisted transfer
//! record").
//!
//! The scope walk already tracks alias classes while it runs (`Scope::alias_class`,
//! `check_transfer`, `binding_event_origin`), but a class dies with its scope. A scope-exit
//! release pass needs the FINAL partition, so this module keeps it: one class per set of
//! bindings that denote or reach one value, with
//!
//! * every member keyed by the declaration (`DeclKey`) and by its unique codegen name
//!   (`BindingNames`, C4) — never by the name the user wrote;
//! * the **release anchor**: the one member whose scope exits last (the *founder*), or the
//!   reason no member can be named statically;
//! * the **escapes**: the ways a member's value leaves this function's hands (returned,
//!   given, sent) — a class that escapes on any path is not released at scope exit by the
//!   bindings that merely named it.
//!
//! # What joins a class (the alias edges)
//!
//! Decided with the Software Architect (plan #17 C5a): storing a heap value into a place
//! JOINS the value's class with the place's class; it does not consume the source. Consuming
//! on store would reject programs that compile today (`bg_arg_alias_container_add_red.ynz`
//! reads `rows` after `bucket.add(rows)`) and would need new user-facing syntax or a
//! per-type `.copy()` ruling (B4: no new user-facing syntax).
//!
//! | event | edge |
//! |---|---|
//! | `let b = a`, `acc = per` | `b`/`acc` joins `a`/`per` |
//! | `let b = {x: a, y: c}`, `[a, c]` | `b` joins EVERY root: `a`, `c` |
//! | `b.items = a`, `o[i] = a` | the receiver's root joins the value's roots |
//! | `o.add(a)`, `o.set(i, a)` | the receiver's root joins the value's roots |
//! | `for (cell in m)` | `cell` joins `m` |
//!
//! # Who frees what
//!
//! A class must free every distinct object in it exactly once, container and stored elements
//! alike. That is the release pass's job (C6/C7); this record only says WHICH binding anchors
//! the class and whether the class may be released at all. Leaks are acceptable meanwhile;
//! an early or a double free never is.

use std::collections::HashMap;

use crate::binding_names::DeclKey;

/// How a member can take part in releasing its class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberRole {
    /// Bound only to fresh values it is the first holder of (`Origin::Owned`, or a `give`
    /// parameter): a candidate founder.
    Owner,
    /// A parameter declared `share`, `lend` or bare: the caller keeps the value.
    BorrowedParam,
    /// Bound at least once to a piece of something else, a loop cell, or a value the compiler
    /// cannot trace: it holds the value but never owns it.
    Piece,
}

/// One binding in a class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassMember {
    /// The declaration (`(span.start, span.end, declared name)`).
    pub decl: DeclKey,
    /// The binding's unique name inside its function, as codegen's AST carries it.
    pub codegen_name: String,
    pub role: MemberRole,
}

/// How a member's value can leave this function's hands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Escape {
    /// Returned (whole, or as a piece of the returned value), on at least one path.
    Returned,
    /// Given to a `give` position of `callee` (a call, a UFCS call, a `background` spawn), on
    /// at least one path.
    Given { callee: String },
    /// Sent into `channel`, on at least one path.
    Sent { channel: String },
    /// Stored into a place no binding names (`makeBag().items = a`).
    StoredIntoTemporary,
}

/// Why no member can be named the founder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoFounderReason {
    /// A member that is not an owner outlives every owner (or shares the outermost scope with
    /// none): the owner's scope ends while `holder` still holds the value. Which one of the
    /// values `holder` holds at that moment is only known at run time.
    OutlivedByNonOwner { holder: String },
    /// The members live in sibling scopes and none encloses the others.
    SiblingScopes,
}

/// Where the class's release is anchored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReleaseAnchor {
    /// Release the class once, when this member's scope exits: the member whose scope exits
    /// last. Members in the same outermost scope: the earliest declared.
    Founder { decl: DeclKey, codegen_name: String },
    /// A parameter that is not `give` is in the class: the caller owns every object in it,
    /// so nothing here releases it.
    CallerOwns { parameter: String },
    /// No member owns a fresh value (all pieces, loop cells or untraceable results), so no
    /// binding here has a release duty. The allocation, if any, belongs to code this
    /// function cannot see (an imported function's result).
    NoOwner,
    /// Owners exist but no static founder (see [`NoFounderReason`]).
    NoStaticFounder(NoFounderReason),
}

/// One alias class: bindings that denote or reach one value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AliasClass {
    /// In declaration order.
    pub members: Vec<ClassMember>,
    pub anchor: ReleaseAnchor,
    pub escapes: Vec<Escape>,
}

impl AliasClass {
    /// True when `codegen_name` is a member.
    pub fn has_member(&self, codegen_name: &str) -> bool {
        self.members
            .iter()
            .any(|member| member.codegen_name == codegen_name)
    }
}

/// The classes of one function.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FunctionTransfers {
    pub classes: Vec<AliasClass>,
}

impl FunctionTransfers {
    /// The class `codegen_name` belongs to.
    pub fn class_of(&self, codegen_name: &str) -> Option<&AliasClass> {
        self.classes
            .iter()
            .find(|class| class.has_member(codegen_name))
    }

    /// True when both bindings are in one class.
    pub fn same_class(&self, first: &str, second: &str) -> bool {
        self.class_of(first)
            .is_some_and(|class| class.has_member(second))
    }
}

/// The finished record of a module, carried on `TypedModule::transfers`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransferRecord {
    functions: HashMap<String, FunctionTransfers>,
}

impl TransferRecord {
    /// The classes of the function named `function_name` (functions are unique by name).
    pub fn function(&self, function_name: &str) -> Option<&FunctionTransfers> {
        self.functions.get(function_name)
    }
}
