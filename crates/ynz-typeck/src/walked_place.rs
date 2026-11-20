//! Which writes free the list a `for` loop is still walking (v0.3-M9 C10, plan #17;
//! `IMP-ownership.md` "Walked list").
//!
//! An assignment releases the value it replaces. A loop that walks `outer.inner.rows` reads the
//! list straight from it, so a write inside the body that releases that list (the whole `outer`,
//! `outer.inner`, an element of a list the loop walks through) leaves the next pass reading freed
//! memory. Codegen holds the list only for the exact binding or exact field path the body assigns
//! (`Cg::begin_loop_held`); every other write that frees it is refused here, as Rust refuses it.
//!
//! Three answers live here and nowhere else (`.claude/rules/authoritative-derivation.md`):
//! * what a place is made of (`PlaceStep`, `place_steps`, `walked_place_steps`);
//! * whether one write frees the walked list (`write_frees_walked_list`);
//! * which parameter places a function writes (`analyze_written_param_places`): computed once per
//!   module before the bodies are checked, carried on `FunctionSig::written_param_places` across
//!   modules, and read by the loop check for a call that passes the walked root to a callee.
//!
//! Cost: O(size of the loop body * path length) per loop; the callee summary is O(F * P * S) per
//! round for F functions with P parameters and S written places, and rounds are bounded because a
//! written place is cut at `MAX_WRITTEN_PLACE_STEPS` steps.

use std::collections::{BTreeSet, HashMap, HashSet};

use ynz_ast::nodes::{Block, Expr, FunctionDecl, Item, Module, OwnershipModifier, Stmt};
use ynz_diagnostics::SourceSpan;

use crate::signatures::FunctionSig;

/// A written place longer than this is cut to its first steps. The cut place is a prefix of the
/// real one, and a prefix is written wherever the real place is: the refusal only grows, never
/// misses. It bounds the fixpoint over recursive callees (`walk(node.next)` writes one step
/// deeper on every round).
pub const MAX_WRITTEN_PLACE_STEPS: usize = 6;

/// One step of a place a statement can write or a loop can walk: the binding it starts at, a
/// field, or one element of a list (`Element(Some(0))` for `cube[0]`, `Element(None)` for an index
/// that is not a literal and may be any element).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceStep<'a> {
    Root(&'a str),
    Field(&'a str),
    Element(Option<i64>),
}

/// A step below the root of a place a function writes through one of its parameters
/// (`holder.rows[0]` written through `holder` is `[Field("rows"), Element(Some(0))]`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WrittenStep {
    Field(String),
    Element(Option<i64>),
}

/// The steps below a parameter's root that a function body assigns; empty when the body rebinds
/// the parameter itself.
pub type WrittenPlace = Vec<WrittenStep>;

impl PlaceStep<'_> {
    /// True when the two steps may denote the same part of a value (an element with an index that
    /// is not a literal may be any element).
    fn may_be_same_part(self, other: PlaceStep<'_>) -> bool {
        match (self, other) {
            (PlaceStep::Element(first), PlaceStep::Element(second)) => {
                first.is_none() || second.is_none() || first == second
            }
            (first, second) => first == second,
        }
    }
}

impl WrittenStep {
    /// This step as a step of a place read in a function body.
    pub fn as_place_step(&self) -> PlaceStep<'_> {
        match self {
            WrittenStep::Field(name) => PlaceStep::Field(name),
            WrittenStep::Element(position) => PlaceStep::Element(*position),
        }
    }

    fn from_place_step(step: PlaceStep<'_>) -> Option<WrittenStep> {
        match step {
            PlaceStep::Root(_) => None,
            PlaceStep::Field(name) => Some(WrittenStep::Field(name.to_string())),
            PlaceStep::Element(position) => Some(WrittenStep::Element(position)),
        }
    }
}

fn element_step(index: &Expr) -> PlaceStep<'static> {
    PlaceStep::Element(match index {
        Expr::IntLit(position, _) => Some(*position),
        _ => None,
    })
}

/// The steps a place expression is made of, root binding first: `cube[0].rows` is
/// `[Root("cube"), Element(Some(0)), Field("rows")]`. `None` for any other expression (a call, a
/// literal).
///
/// Time: O(P) where P = path length  Space: O(P).
pub fn place_steps(expr: &Expr) -> Option<Vec<PlaceStep<'_>>> {
    match expr {
        Expr::Ident(name, _) => Some(vec![PlaceStep::Root(name.as_str())]),
        Expr::FieldAccess {
            receiver, field, ..
        } => {
            let mut steps = place_steps(receiver)?;
            steps.push(PlaceStep::Field(field.as_str()));
            Some(steps)
        }
        Expr::IndexAccess {
            receiver, index, ..
        } => {
            let mut steps = place_steps(receiver)?;
            steps.push(element_step(index));
            Some(steps)
        }
        _ => None,
    }
}

/// The place a `for` loop walks: `iter` itself, or the list a `maybe` place holds when the loop
/// reads it through `.or(default)` (`for (row in cube[0].or(empty))` walks `cube[0]`). `None`
/// when the loop walks a value that is no place (a call result, a literal).
///
/// Time: O(P) where P = path length  Space: O(P).
pub fn walked_place_steps(iter: &Expr) -> Option<Vec<PlaceStep<'_>>> {
    match iter {
        Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } if method == "or" && args.len() == 1 => place_steps(receiver),
        _ => place_steps(iter),
    }
}

/// The place an assignment statement writes (`matrix = ...`, `outer.inner = ...`,
/// `cube[0] = ...`); `None` for any other statement or a target that is no place.
///
/// Time: O(P) where P = path length  Space: O(P).
pub fn written_place(stmt: &Stmt) -> Option<Vec<PlaceStep<'_>>> {
    match stmt {
        Stmt::Assign { target, .. } => Some(vec![PlaceStep::Root(target.as_str())]),
        Stmt::FieldAssign { target, .. } => place_steps(target),
        Stmt::IndexAssign {
            receiver, index, ..
        } => {
            let mut steps = place_steps(receiver)?;
            steps.push(element_step(index));
            Some(steps)
        }
        _ => None,
    }
}

/// `outer.inner`, `cube[0]`, `cube[...]` (an index that is not a literal): a place as the
/// diagnostics name it.
///
/// Time: O(P) where P = path length  Space: O(P).
pub fn place_text(steps: &[PlaceStep<'_>]) -> String {
    steps
        .iter()
        .map(|step| match step {
            PlaceStep::Root(name) => (*name).to_string(),
            PlaceStep::Field(name) => format!(".{name}"),
            PlaceStep::Element(Some(position)) => format!("[{position}]"),
            PlaceStep::Element(None) => "[...]".to_string(),
        })
        .collect()
}

/// True when writing `written` frees the list the loop at `walked` is reading, and the loop does
/// not hold that list for the write.
///
/// * The root binding or a prefix of the walked place, written: the walked list goes with the
///   old value.
/// * The walked place itself, written: `exact_write_is_held` says whether the loop holds the list
///   for this write. Codegen holds it for a statement that assigns the exact binding or exact
///   field path (`begin_loop_held`), not for a walked place that goes through an element, and not
///   for a write a callee makes.
/// * A place below the walked one (`rows[0] = ...` while walking `rows`) frees an item, not the
///   list; the loop variable's own copy rules cover it (C6).
///
/// Time: O(P) where P = path length  Space: O(1).
pub fn write_frees_walked_list(
    written: &[PlaceStep<'_>],
    walked: &[PlaceStep<'_>],
    exact_write_is_held: bool,
) -> bool {
    let (
        Some((PlaceStep::Root(written_root), written_tail)),
        Some((PlaceStep::Root(walked_root), walked_tail)),
    ) = (written.split_first(), walked.split_first())
    else {
        return false;
    };
    if written_root != walked_root || written_tail.len() > walked_tail.len() {
        return false;
    }
    let names_a_prefix = written_tail
        .iter()
        .zip(walked_tail)
        .all(|(written_step, walked_step)| written_step.may_be_same_part(*walked_step));
    if !names_a_prefix {
        return false;
    }
    let writes_walked_place_itself = written_tail.len() == walked_tail.len();
    let held = writes_walked_place_itself
        && exact_write_is_held
        && !walked_tail
            .iter()
            .any(|step| matches!(step, PlaceStep::Element(_)));
    !held
}

/// The first assignment in `body` (at any nesting depth) that frees the list the loop at `walked`
/// is reading (`matrix = ...`, `outer.inner = ...`, `cube[0] = ...`): where it is and the place
/// it writes.
///
/// Time: O(N * P) where N = AST nodes, P = path length  Space: O(D + P) recursion depth, one place.
pub fn assignment_freeing_walked_list<'a>(
    body: &'a Block,
    walked: &[PlaceStep<'_>],
) -> Option<(&'a SourceSpan, Vec<PlaceStep<'a>>)> {
    let frees_walked_list = |assignment: &Stmt| {
        written_place(assignment)
            .is_some_and(|written| write_frees_walked_list(&written, walked, true))
    };
    let assignment = body
        .stmts
        .iter()
        .find_map(|stmt| crate::cpu_admission::stmt_find_assignment(stmt, &frees_walked_list))?;
    let span = match assignment {
        Stmt::Assign { span, .. }
        | Stmt::FieldAssign { span, .. }
        | Stmt::IndexAssign { span, .. } => span,
        _ => return None,
    };
    Some((span, written_place(assignment)?))
}

/// One call a statement makes: the callee's name, the arguments in parameter order (`p.f(a)` is
/// `f(p, a)`, the receiver first), and whether it was written in the method form.
pub struct CallSite<'a> {
    pub callee: &'a str,
    pub arguments: Vec<&'a Expr>,
    pub is_method_form: bool,
    pub call: &'a Expr,
}

/// Visit every call in `expr`, nested ones included.
///
/// Time: O(N) where N = AST nodes  Space: O(D) recursion depth.
pub fn visit_calls_in_expr<'a>(expr: &'a Expr, visit: &mut dyn FnMut(CallSite<'a>)) {
    let visit_all = |expressions: &mut dyn Iterator<Item = &'a Expr>,
                     visit: &mut dyn FnMut(CallSite<'a>)| {
        for expression in expressions {
            visit_calls_in_expr(expression, visit);
        }
    };
    match expr {
        Expr::Call(call) => {
            if let Expr::Ident(callee, _) = &call.callee {
                visit(CallSite {
                    callee,
                    arguments: call.args.iter().collect(),
                    is_method_form: false,
                    call: expr,
                });
            }
            visit_all(&mut call.args.iter(), visit);
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            visit(CallSite {
                callee: method,
                arguments: std::iter::once(receiver.as_ref())
                    .chain(args.iter())
                    .collect(),
                is_method_form: true,
                call: expr,
            });
            visit_calls_in_expr(receiver, visit);
            visit_all(&mut args.iter(), visit);
        }
        Expr::BinOp { lhs, rhs, .. } => {
            visit_all(&mut [lhs.as_ref(), rhs.as_ref()].into_iter(), visit)
        }
        Expr::UnaryOp { operand, .. } => visit_calls_in_expr(operand, visit),
        Expr::FieldAccess { receiver, .. } | Expr::PostfixOp { receiver, .. } => {
            visit_calls_in_expr(receiver, visit)
        }
        Expr::IndexAccess {
            receiver, index, ..
        } => visit_all(&mut [receiver.as_ref(), index.as_ref()].into_iter(), visit),
        Expr::StructLit { fields, .. } => {
            visit_all(&mut fields.iter().map(|field| &field.value), visit)
        }
        Expr::ArrayLit { elements, .. } => visit_all(&mut elements.iter(), visit),
        Expr::MapLit { entries, .. } => visit_all(
            &mut entries.iter().flat_map(|(key, value)| [key, value]),
            visit,
        ),
        Expr::Is { expr: inner, .. } | Expr::Wait(inner, _) | Expr::Background(inner, _) => {
            visit_calls_in_expr(inner, visit)
        }
        Expr::InterpolatedString(parts, _) => {
            for part in parts {
                if let ynz_ast::nodes::StringPart::Expr(inner, _) = part {
                    visit_calls_in_expr(inner, visit);
                }
            }
        }
        Expr::Ident(..)
        | Expr::StringLit(..)
        | Expr::IntLit(..)
        | Expr::NumberLit(..)
        | Expr::BoolLit(..)
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. }
        | Expr::Error(..) => {}
    }
}

/// Visit every call in `stmt` and the statements nested in it.
///
/// Time: O(N) where N = AST nodes  Space: O(D) recursion depth.
pub fn visit_calls_in_stmt<'a>(stmt: &'a Stmt, visit: &mut dyn FnMut(CallSite<'a>)) {
    let visit_block = |block: &'a Block, visit: &mut dyn FnMut(CallSite<'a>)| {
        for nested in &block.stmts {
            visit_calls_in_stmt(nested, visit);
        }
    };
    match stmt {
        Stmt::Expr(value) | Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            visit_calls_in_expr(value, visit)
        }
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                visit_calls_in_expr(value, visit);
            }
        }
        Stmt::FieldAssign { target, value, .. } => {
            visit_calls_in_expr(target, visit);
            visit_calls_in_expr(value, visit);
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            visit_calls_in_expr(receiver, visit);
            visit_calls_in_expr(index, visit);
            visit_calls_in_expr(value, visit);
        }
        Stmt::If { cond, body, .. } | Stmt::While { cond, body, .. } => {
            visit_calls_in_expr(cond, visit);
            visit_block(body, visit);
        }
        Stmt::For { iter, body, .. } => {
            visit_calls_in_expr(iter, visit);
            visit_block(body, visit);
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            visit_calls_in_expr(scrutinee, visit);
            for arm in arms {
                visit_block(&arm.body, visit);
            }
            if let Some(block) = else_arm {
                visit_block(block, visit);
            }
        }
    }
}

/// The places a callee writes through the parameter an argument is passed to, rewritten onto the
/// caller's place: for a call `reset(holder.inner)` whose `reset` writes `[Field("rows")]`
/// through its parameter, `holder.inner.rows`. Only a parameter the callee may change counts
/// (`lend`, or no modifier: the compiler infers it from the body); a `share` parameter is
/// read-only and a `give` parameter's value is the callee's own.
///
/// Time: O(P * S) where P = argument path length, S = written places  Space: O(P + S).
pub fn places_written_by_call<'a>(
    argument: &'a Expr,
    parameter_ownership: Option<&OwnershipModifier>,
    written_through_parameter: &'a [WrittenPlace],
) -> Vec<Vec<PlaceStep<'a>>> {
    if matches!(
        parameter_ownership,
        Some(OwnershipModifier::Share | OwnershipModifier::Give)
    ) {
        return Vec::new();
    }
    let Some(argument_steps) = place_steps(argument) else {
        return Vec::new();
    };
    written_through_parameter
        .iter()
        .map(|tail| {
            let mut absolute = argument_steps.clone();
            absolute.extend(tail.iter().map(WrittenStep::as_place_step));
            absolute
        })
        .collect()
}

/// The places each function of `module` writes through each of its parameters: for `function
/// reset(lend holder: Holder) { holder.rows = ... }`, `[[[Field("rows")]]]`. A function that
/// passes a parameter to a callee that writes it writes it too, so the answer is a fixpoint over
/// the module's call graph: it starts empty and grows until a round adds nothing. Recursive and
/// mutually recursive functions need no special case; the cut at `MAX_WRITTEN_PLACE_STEPS` keeps
/// the set finite. A callee from another module is read from `imported` (its summary was computed
/// when its own module was checked).
///
/// Time: O(R * F * N) where R = rounds, F = functions, N = AST nodes per body
/// Space: O(F * P * S) for P parameters and S written places per parameter.
pub fn analyze_written_param_places(
    module: &Module,
    imported: &HashMap<String, FunctionSig>,
) -> HashMap<String, Vec<Vec<WrittenPlace>>> {
    // A name defined twice is reported as a duplicate definition elsewhere; its summary is the
    // first definition's. Two bodies under one name would replace each other's summary on every
    // round and the fixpoint would never settle (a compiler hang on the very program that
    // deserves the diagnostic).
    let mut defined_names: HashSet<&str> = HashSet::new();
    let functions: Vec<&FunctionDecl> = module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function) if defined_names.insert(function.name.as_str()) => {
                Some(function)
            }
            _ => None,
        })
        .collect();
    let mut summaries: HashMap<String, Vec<BTreeSet<WrittenPlace>>> = functions
        .iter()
        .map(|function| {
            (
                function.name.clone(),
                vec![BTreeSet::new(); function.params.len()],
            )
        })
        .collect();
    loop {
        let mut changed = false;
        for function in &functions {
            let fresh = summarize_function(function, &summaries, imported, &functions);
            if summaries.get(&function.name) != Some(&fresh) {
                summaries.insert(function.name.clone(), fresh);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    summaries
        .into_iter()
        .map(|(name, per_parameter)| {
            let places = per_parameter
                .into_iter()
                .map(|set| set.into_iter().collect())
                .collect();
            (name, places)
        })
        .collect()
}

/// What `function` writes through each parameter, given what every callee writes so far.
fn summarize_function(
    function: &FunctionDecl,
    summaries: &HashMap<String, Vec<BTreeSet<WrittenPlace>>>,
    imported: &HashMap<String, FunctionSig>,
    functions: &[&FunctionDecl],
) -> Vec<BTreeSet<WrittenPlace>> {
    let mut written: Vec<BTreeSet<WrittenPlace>> = vec![BTreeSet::new(); function.params.len()];
    // `let view = holder.inner` is not an alias a write can travel through: a read that is
    // written is a copy (`non_owned_reads`, rule (a)), so `view.rows = ...` changes the view's own
    // value and never `holder.inner.rows`. Only the parameter names are roots of a written place.
    let parameter_position: HashMap<&str, usize> = function
        .params
        .iter()
        .enumerate()
        .map(|(position, parameter)| (parameter.name.as_str(), position))
        .collect();

    let record = |steps: &[PlaceStep<'_>], written: &mut Vec<BTreeSet<WrittenPlace>>| {
        let Some((PlaceStep::Root(root), tail)) = steps.split_first() else {
            return;
        };
        let Some(position) = parameter_position.get(root) else {
            return;
        };
        let mut place: WrittenPlace = tail
            .iter()
            .filter_map(|step| WrittenStep::from_place_step(*step))
            .collect();
        place.truncate(MAX_WRITTEN_PLACE_STEPS);
        written[*position].insert(place);
    };

    let mut stack: Vec<&Stmt> = function.body.stmts.iter().collect();
    while let Some(stmt) = stack.pop() {
        if let Some(steps) = written_place(stmt) {
            record(&steps, &mut written);
        }
        visit_calls_in_stmt_shallow(stmt, &mut |site| {
            let Some(callee_writes) = callee_summary(site.callee, summaries, imported) else {
                return;
            };
            let Some(ownerships) = callee_ownerships(site.callee, imported, functions) else {
                return;
            };
            if ownerships.len() != site.arguments.len() {
                return;
            }
            for (position, argument) in site.arguments.iter().enumerate() {
                let Some(through_parameter) = callee_writes.get(position) else {
                    continue;
                };
                for absolute in places_written_by_call(
                    argument,
                    ownerships[position].as_ref(),
                    through_parameter,
                ) {
                    record(&absolute, &mut written);
                }
            }
        });
        push_nested_statements(stmt, &mut stack);
    }
    written
}

/// What the callee `name` writes through each parameter: the module's own fixpoint state, or the
/// summary an imported signature carries. Local functions shadow imports of the same name.
fn callee_summary(
    name: &str,
    summaries: &HashMap<String, Vec<BTreeSet<WrittenPlace>>>,
    imported: &HashMap<String, FunctionSig>,
) -> Option<Vec<Vec<WrittenPlace>>> {
    if let Some(local) = summaries.get(name) {
        return Some(
            local
                .iter()
                .map(|set| set.iter().cloned().collect())
                .collect(),
        );
    }
    imported
        .get(name)
        .map(|sig| sig.written_param_places.clone())
}

fn callee_ownerships(
    name: &str,
    imported: &HashMap<String, FunctionSig>,
    functions: &[&FunctionDecl],
) -> Option<Vec<Option<OwnershipModifier>>> {
    if let Some(local) = functions.iter().find(|function| function.name == name) {
        return Some(
            local
                .params
                .iter()
                .map(|parameter| parameter.ownership.clone())
                .collect(),
        );
    }
    imported.get(name).map(|sig| sig.param_ownerships.clone())
}

/// Push the statements directly inside `stmt`'s blocks.
fn push_nested_statements<'a>(stmt: &'a Stmt, stack: &mut Vec<&'a Stmt>) {
    match stmt {
        Stmt::If { body, .. } | Stmt::While { body, .. } | Stmt::For { body, .. } => {
            stack.extend(body.stmts.iter())
        }
        Stmt::Match { arms, else_arm, .. } => {
            for arm in arms {
                stack.extend(arm.body.stmts.iter());
            }
            if let Some(block) = else_arm {
                stack.extend(block.stmts.iter());
            }
        }
        _ => {}
    }
}

/// Visit the calls of `stmt` itself (its own expressions), not those of nested blocks.
fn visit_calls_in_stmt_shallow<'a>(stmt: &'a Stmt, visit: &mut dyn FnMut(CallSite<'a>)) {
    match stmt {
        Stmt::If { cond, .. } | Stmt::While { cond, .. } => visit_calls_in_expr(cond, visit),
        Stmt::For { iter, .. } => visit_calls_in_expr(iter, visit),
        Stmt::Match { scrutinee, .. } => visit_calls_in_expr(scrutinee, visit),
        simple => visit_calls_in_stmt(simple, visit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ynz_parser::{parse_query, CompilerDb, SourceFile};

    use PlaceStep::{Element, Field, Root};

    // WHY (High): the one answer to "does this write free the walked list" decides which programs
    // are refused; every row below is a place a loop can walk and a write it can make.
    #[test]
    fn a_write_frees_the_walked_list_when_it_names_the_place_or_a_prefix_of_it() {
        let walked_field = [Root("outer"), Field("inner"), Field("rows")];
        let walked_element = [Root("cube"), Element(Some(0))];
        let cases: [(&[PlaceStep], &[PlaceStep], bool, bool); 11] = [
            // a prefix, the root
            (&[Root("outer"), Field("inner")], &walked_field, true, true),
            (&[Root("outer")], &walked_field, true, true),
            // exact field path: held for a statement, freed by a callee
            (&walked_field, &walked_field, true, false),
            (&walked_field, &walked_field, false, true),
            // below the walked place: an item, not the list
            (
                &[
                    Root("outer"),
                    Field("inner"),
                    Field("rows"),
                    Element(Some(0)),
                ],
                &walked_field,
                true,
                false,
            ),
            // another field, another root
            (&[Root("outer"), Field("name")], &walked_field, true, false),
            (&[Root("other"), Field("inner")], &walked_field, true, false),
            // an element the loop walks through is never held, same index or any index
            (
                &[Root("cube"), Element(Some(0))],
                &walked_element,
                true,
                true,
            ),
            (&[Root("cube"), Element(None)], &walked_element, true, true),
            (
                &[Root("cube"), Element(Some(1))],
                &walked_element,
                true,
                false,
            ),
            (&[Root("cube")], &walked_element, true, true),
        ];
        for (written, walked, exact_write_is_held, expected) in cases {
            assert_eq!(
                write_frees_walked_list(written, walked, exact_write_is_held),
                expected,
                "writing {} while walking {} (held: {exact_write_is_held})",
                place_text(written),
                place_text(walked)
            );
        }
    }

    // The fixpoint is the one place here that can fail by never returning, so the summary runs on
    // its own thread and a missed deadline is a test failure rather than a hung suite. These
    // programs settle in well under a millisecond; 10 s is far past any loaded machine. A stuck
    // thread is left spinning until the test process exits.
    const FIXPOINT_LIVENESS_BOUND: std::time::Duration = std::time::Duration::from_secs(10);

    fn summaries(source: &str) -> HashMap<String, Vec<Vec<WrittenPlace>>> {
        let source = source.to_string();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let db = CompilerDb::default();
            let file = SourceFile::new(&db, "walked.ynz".to_string(), source);
            let parsed = parse_query(&db, file);
            let written = analyze_written_param_places(&parsed.module, &HashMap::new());
            let _ = sender.send(written);
        });
        receiver
            .recv_timeout(FIXPOINT_LIVENESS_BOUND)
            .expect("the written-places fixpoint did not settle within the liveness bound")
    }

    fn field(name: &str) -> WrittenStep {
        WrittenStep::Field(name.to_string())
    }

    // WHY (High): the summary is what lets a loop refuse a call; a missed write is a use after free.
    #[test]
    fn a_function_writes_the_places_it_assigns_through_a_parameter_and_through_its_callees() {
        let written = summaries(
            "shape Holder { name: String rows: Array<Array<Int>> }\n\
             function direct(lend holder: Holder) -> Nothing { holder.rows = [[1]] }\n\
             function element(lend holder: Holder) -> Nothing { holder.rows[0] = [1] }\n\
             function reader(share holder: Holder) -> Int { return holder.rows.count() }\n\
             function forwarded(lend held: Holder) -> Nothing { direct(held) }\n\
             function viaView(lend holder: Holder) -> Nothing {\n\
               let view = holder.rows\n\
               view[1] = [2]\n\
             }\n\
             function twoParameters(lend first: Holder, lend second: Holder) -> Nothing {\n\
               second.name = `x`\n\
             }\n",
        );
        assert_eq!(written["direct"], vec![vec![vec![field("rows")]]]);
        assert_eq!(
            written["element"],
            vec![vec![vec![field("rows"), WrittenStep::Element(Some(0))]]]
        );
        assert_eq!(written["reader"], vec![Vec::<WrittenPlace>::new()]);
        assert_eq!(written["forwarded"], vec![vec![vec![field("rows")]]]);
        // A read that is written is a copy (`non_owned_reads` rule (a)): `view[1] = ...` changes
        // the view's own value, never `holder.rows`.
        assert_eq!(written["viaView"], vec![Vec::<WrittenPlace>::new()]);
        assert_eq!(
            written["twoParameters"],
            vec![Vec::<WrittenPlace>::new(), vec![vec![field("name")]]]
        );
    }

    // WHY (High): a function defined twice (a diagnosed error) with bodies that write different
    // places must not keep the fixpoint from settling: the compiler hung on this program.
    #[test]
    fn a_function_defined_twice_keeps_the_summary_of_its_first_definition() {
        let written = summaries(
            "shape Holder { rows: Array<Array<Int>> }\n\
             function reset(lend holder: Holder) -> Nothing { holder.rows = [[1]] }\n\
             function reset(lend holder: Holder) -> Nothing { print(holder.rows.count()) }\n",
        );
        assert_eq!(written["reset"], vec![vec![vec![field("rows")]]]);
    }

    // WHY (High): mutual recursion must reach a fixpoint, and a recursion that writes one step
    // deeper on every round must stop: the place is cut at `MAX_WRITTEN_PLACE_STEPS`.
    #[test]
    fn mutually_recursive_functions_reach_a_fixpoint_and_a_deepening_recursion_is_cut() {
        let written = summaries(
            "shape Node { next: Node rows: Array<Int> }\n\
             function ping(lend node: Node, depth: Int) -> Nothing {\n\
               if (depth > 0) { pong(node, depth - 1) }\n\
             }\n\
             function pong(lend node: Node, depth: Int) -> Nothing {\n\
               ping(node, depth)\n\
               node.rows = [1]\n\
             }\n\
             function deepen(lend node: Node) -> Nothing {\n\
               deepen(node.next)\n\
               node.rows = [1]\n\
             }\n",
        );
        for name in ["ping", "pong"] {
            assert_eq!(written[name][0], vec![vec![field("rows")]], "{name}");
        }
        let deepest = written["deepen"][0]
            .iter()
            .map(Vec::len)
            .max()
            .expect("deepen writes something");
        assert_eq!(deepest, MAX_WRITTEN_PLACE_STEPS);
        assert!(
            written["deepen"][0].contains(&vec![field("rows")]),
            "the direct write stays: {:?}",
            written["deepen"][0]
        );
    }
}
