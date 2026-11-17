//! Does a function's body need the length of an unsized `fixed<T>` parameter? (v0.3-M9 C1h, #433)
//!
//! A function built once (it waits, or auto-parallel promotion may run it as a task) has no
//! per-length versions, so the length of such a parameter reads as unknown there. That is harmless
//! while the body never asks for it: the spawn copy of an argument is sized and released from the
//! argument's own type at the call site (`release_glue_free_kind`), so a callee that only receives
//! the list works. This module is the ONE answer to "does the body ask for the length", read by the
//! refusal of `FixedLengthMissingInTaskFunction` (`check.rs`), next to the exclusion set it
//! refines (`why_built_once`).
//!
//! The body needs the length when it
//! - uses the parameter in any way but handing it on as a call argument (reads `count`, iterates,
//!   indexes, returns, stores, reassigns it, or spawns a task with it: the task's copy is sized
//!   from the length here);
//! - hands it on to a position that needs it (a local function whose own parameter needs the
//!   length, or any callee this module cannot read);
//! - or owns it and must release its items: a `give` parameter whose items own heap.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::{Expr, FunctionDecl, Item, Module, OwnershipModifier, Stmt};

use crate::signatures::SignatureTable;
use crate::string_ownership::children;
use crate::types::{FixedLength, Type};

/// Answers "does the body of `function` need the length of its parameter `index`", memoised
/// across the module's functions.
pub struct LengthUses<'a> {
    functions: HashMap<&'a str, &'a FunctionDecl>,
    sig_table: &'a SignatureTable,
    answers: RefCell<HashMap<(String, usize), bool>>,
    in_progress: RefCell<HashSet<(String, usize)>>,
}

impl<'a> LengthUses<'a> {
    /// O(functions) time and space.
    pub fn new(module: &'a Module, sig_table: &'a SignatureTable) -> Self {
        let functions = module
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Function(function) => Some((function.name.as_str(), function)),
                _ => None,
            })
            .collect();
        Self {
            functions,
            sig_table,
            answers: RefCell::new(HashMap::new()),
            in_progress: RefCell::new(HashSet::new()),
        }
    }

    /// True when the body of `function` needs the length of its parameter `index`. A function this
    /// module cannot read (imported, unknown) needs it: the safe answer. A cycle of functions that
    /// only hand the list to each other does not (no body in it asks). O(size of the bodies
    /// reachable through the list being handed on).
    pub fn needs_length(&self, function: &str, index: usize) -> bool {
        let key = (function.to_string(), index);
        if let Some(answer) = self.answers.borrow().get(&key) {
            return *answer;
        }
        if !self.in_progress.borrow_mut().insert(key.clone()) {
            return false;
        }
        let answer = self.compute(function, index);
        self.in_progress.borrow_mut().remove(&key);
        self.answers.borrow_mut().insert(key, answer);
        answer
    }

    fn compute(&self, function: &str, index: usize) -> bool {
        let (Some(declaration), Some(signature)) = (
            self.functions.get(function),
            self.sig_table.fns.get(function),
        ) else {
            return true;
        };
        let Some((parameter, parameter_type)) = signature.params.get(index) else {
            return true;
        };
        // A parameter that owns the list releases its items where the list is: with no length
        // there, the release would be wrong.
        let gives = matches!(
            signature.param_ownerships.get(index),
            Some(Some(OwnershipModifier::Give))
        );
        if gives {
            if let Type::BuiltinFixed { elem, .. } = parameter_type {
                if items_own_heap(elem) {
                    return true;
                }
            }
        }
        let mut uses = Uses {
            owner: self,
            parameter,
            needed: false,
        };
        uses.statements(&declaration.body.stmts);
        uses.needed
    }
}

/// Do the items of a `fixed` list of `elem` own heap a release must free? `number` of up to 34
/// digits is stored inline, like the scalars.
pub(crate) fn items_own_heap(elem: &Type) -> bool {
    !matches!(
        elem,
        Type::Int
            | Type::Float
            | Type::Bool
            | Type::Nothing
            | Type::Options { .. }
            | Type::Range { .. }
    ) && !matches!(elem, Type::Number { precision } if *precision <= 34)
}

/// One walk of a function body for the uses of one parameter.
struct Uses<'a, 'b> {
    owner: &'b LengthUses<'a>,
    parameter: &'b str,
    needed: bool,
}

impl Uses<'_, '_> {
    fn statements(&mut self, statements: &[Stmt]) {
        for statement in statements {
            if self.needed {
                return;
            }
            self.statement(statement);
        }
    }

    fn statement(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Expr(expr) => self.expr(expr),
            Stmt::Let { value, .. } => self.expr(value),
            Stmt::Assign { target, value, .. } => {
                if target == self.parameter {
                    self.needed = true;
                }
                self.expr(value);
            }
            Stmt::If { cond, body, .. } | Stmt::While { cond, body, .. } => {
                self.expr(cond);
                self.statements(&body.stmts);
            }
            Stmt::Match {
                scrutinee,
                arms,
                else_arm,
                ..
            } => {
                self.expr(scrutinee);
                for arm in arms {
                    self.statements(&arm.body.stmts);
                }
                if let Some(else_block) = else_arm {
                    self.statements(&else_block.stmts);
                }
            }
            Stmt::For { iter, body, .. } => {
                self.expr(iter);
                self.statements(&body.stmts);
            }
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.expr(value);
                }
            }
            Stmt::FieldAssign { target, value, .. } => {
                self.expr(target);
                self.expr(value);
            }
            Stmt::IndexAssign {
                receiver,
                index,
                value,
                ..
            } => {
                self.expr(receiver);
                self.expr(index);
                self.expr(value);
            }
        }
    }

    /// True when `expr` is the parameter itself.
    fn is_parameter(&self, expr: &Expr) -> bool {
        matches!(expr, Expr::Ident(name, _) if name == self.parameter)
    }

    /// An argument at `position` of the call of `callee`: handing the parameter on is a use only
    /// when the callee's parameter needs the length.
    fn argument(&mut self, callee: Option<&str>, position: usize, argument: &Expr) {
        if !self.is_parameter(argument) {
            self.expr(argument);
            return;
        }
        let handed_on_freely = callee.is_some_and(|callee| {
            self.owner
                .sig_table
                .fns
                .get(callee)
                .is_some_and(|signature| {
                    matches!(
                        signature.params.get(position),
                        Some((
                            _,
                            Type::BuiltinFixed {
                                size: FixedLength::OfParam { .. },
                                ..
                            }
                        ))
                    )
                })
                && !self.owner.needs_length(callee, position)
        });
        if !handed_on_freely {
            self.needed = true;
        }
    }

    fn expr(&mut self, expr: &Expr) {
        if self.needed {
            return;
        }
        match expr {
            Expr::Ident(..) => {
                if self.is_parameter(expr) {
                    self.needed = true;
                }
            }
            Expr::Call(call) => {
                let callee = match &call.callee {
                    Expr::Ident(name, _) => Some(name.as_str()),
                    _ => None,
                };
                if callee.is_none() {
                    self.expr(&call.callee);
                }
                for (position, argument) in call.args.iter().enumerate() {
                    self.argument(callee, position, argument);
                }
            }
            // `xs.total()` is `total(xs)` when `total` is a function written in Yinz.
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                let local = self.owner.sig_table.fns.contains_key(method.as_str());
                if local {
                    self.argument(Some(method), 0, receiver);
                } else {
                    self.expr(receiver);
                }
                for (position, argument) in args.iter().enumerate() {
                    self.argument(local.then_some(method.as_str()), position + 1, argument);
                }
            }
            // A task's copy of an argument is sized from the length known HERE.
            Expr::Background(inner, _) => match inner.as_ref() {
                Expr::Call(call) => {
                    for argument in &call.args {
                        self.expr(argument);
                    }
                }
                other => self.expr(other),
            },
            other => {
                for child in children(other) {
                    self.expr(child);
                }
            }
        }
    }
}
