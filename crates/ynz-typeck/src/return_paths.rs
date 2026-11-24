use ynz_ast::nodes::{Block, Stmt};
use ynz_diagnostics::SourceSpan;

/// Result of a structural return-path analysis over a block.
pub struct ReturnAnalysis {
    /// True when every possible execution path through the block ends with a `return`.
    pub all_paths_return: bool,
    /// Spans of statements that are structurally unreachable (appear after a definite return).
    pub dead_code: Vec<SourceSpan>,
}

/// Analyze whether every path through `block` ends with a `return`.
///
/// Rules:
/// - `Stmt::Return` is a definite-return point; all following statements in the
///   same block are dead code.
/// - `Stmt::Match` with an `else_arm` where ALL value arms AND the else arm
///   all return is a definite-return point.
/// - `Stmt::If`, `Stmt::While`, `Stmt::For` are NOT definite-return points —
///   their bodies may not execute at all.
/// - `Stmt::Let`, `Stmt::Assign`, `Stmt::Expr` are never return points.
///
/// Dead-code detection recurses into sub-blocks (if/match/while/for bodies) to
/// collect inner dead-code spans even when the outer block itself is not dead.
pub fn analyze_return_paths(block: &Block) -> ReturnAnalysis {
    analyze_stmts(&block.stmts)
}

pub fn analyze_stmts(stmts: &[Stmt]) -> ReturnAnalysis {
    let mut dead = Vec::new();
    let mut definite_return = false;

    for stmt in stmts {
        if definite_return {
            dead.push(stmt_span(stmt));
            continue;
        }

        match stmt {
            Stmt::Return { .. } => {
                definite_return = true;
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    let inner = analyze_stmts(&arm.body.stmts);
                    dead.extend(inner.dead_code);
                }
                if let Some(else_body) = else_arm {
                    let inner = analyze_stmts(&else_body.stmts);
                    dead.extend(inner.dead_code);

                    let all_arms_return = arms
                        .iter()
                        .all(|arm| analyze_stmts(&arm.body.stmts).all_paths_return);
                    let else_returns = analyze_stmts(&else_body.stmts).all_paths_return;
                    if all_arms_return && else_returns {
                        definite_return = true;
                    }
                }
            }
            Stmt::If { body, .. } => {
                let inner = analyze_stmts(&body.stmts);
                dead.extend(inner.dead_code);
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                let inner = analyze_stmts(&body.stmts);
                dead.extend(inner.dead_code);
            }
            Stmt::Expr(_) | Stmt::Let { .. } | Stmt::Assign { .. } => {}
            // M4: field assignment — same as a bare expression for return-path purposes.
            Stmt::FieldAssign { .. } => {}
            // M5 P1: index assignment — same as a bare expression for return-path purposes.
            Stmt::IndexAssign { .. } => {}
        }
    }

    ReturnAnalysis {
        all_paths_return: definite_return,
        dead_code: dead,
    }
}

fn stmt_span(stmt: &Stmt) -> SourceSpan {
    match stmt {
        Stmt::Expr(e) => e.span().clone(),
        Stmt::Let { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::If { span, .. }
        | Stmt::Match { span, .. }
        | Stmt::While { span, .. }
        | Stmt::For { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::FieldAssign { span, .. }
        | Stmt::IndexAssign { span, .. } => span.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ynz_ast::nodes::{Block, Expr, MatchArm, MatchPattern, MatchPatternKind, Stmt};
    use ynz_diagnostics::SourceSpan;

    fn span() -> SourceSpan {
        SourceSpan::new("test.ynz", 0, 1)
    }

    fn err_expr() -> Expr {
        Expr::Error(span())
    }

    fn empty_block() -> Block {
        Block {
            stmts: vec![],
            span: span(),
        }
    }

    fn return_block() -> Block {
        Block {
            stmts: vec![Stmt::Return {
                value: None,
                span: span(),
            }],
            span: span(),
        }
    }

    fn block_of(stmts: Vec<Stmt>) -> Block {
        Block {
            stmts,
            span: span(),
        }
    }

    /// `match x { 1 => return; <else_arm> }`, the one value arm returning.
    fn match_with_returning_arm(else_arm: Option<Block>) -> Stmt {
        let arm = MatchArm {
            pattern: MatchPattern {
                kind: MatchPatternKind::Value(err_expr()),
                span: span(),
            },
            body: return_block(),
            arrow_span: span(),
        };
        Stmt::Match {
            scrutinee: err_expr(),
            arms: vec![arm],
            else_arm,
            span: span(),
        }
    }

    /// One named test per case so the runner names the failing case.
    macro_rules! all_paths_return_case {
        ($test_name:ident, $block:expr, $expected:expr) => {
            // bug: a function with a path that falls off the end is accepted, or one whose every path returns is refused as missing a return | tier: High
            #[test]
            fn $test_name() {
                let result = analyze_return_paths(&$block);
                assert_eq!(result.all_paths_return, $expected);
            }
        };
    }

    // Case bug: an empty block is claimed to return.
    all_paths_return_case!(empty_block_does_not_return, empty_block(), false);
    // Case bug: the simplest valid function (one explicit return) is refused.
    all_paths_return_case!(
        single_return_stmt_returns_on_all_paths,
        return_block(),
        true
    );
    // Case bug: a statement after a return (dead code) makes the block stop counting as returning.
    all_paths_return_case!(
        return_followed_by_a_statement_still_returns,
        block_of(vec![
            Stmt::Return {
                value: None,
                span: span()
            },
            Stmt::Expr(err_expr())
        ]),
        true
    );
    // Case bug: `if (cond) { return }` is claimed to cover the false branch.
    all_paths_return_case!(
        simple_if_body_return_does_not_cover_all_paths,
        block_of(vec![Stmt::If {
            cond: err_expr(),
            body: return_block(),
            span: span()
        }]),
        false
    );
    // Case bug: a match whose every arm and else arm return is refused.
    all_paths_return_case!(
        match_with_else_all_arms_return_covers_all_paths,
        block_of(vec![match_with_returning_arm(Some(return_block()))]),
        true
    );
    // Case bug: a match with no else arm is claimed to cover the no-arm-matches path.
    all_paths_return_case!(
        match_without_else_does_not_cover_all_paths,
        block_of(vec![match_with_returning_arm(None)]),
        false
    );
    // Case bug: `while (cond) { return }` is claimed to return though the loop may run zero times.
    all_paths_return_case!(
        while_loop_body_return_does_not_cover_all_paths,
        block_of(vec![Stmt::While {
            cond: err_expr(),
            body: return_block(),
            span: span()
        }]),
        false
    );
    // Case bug: `for (i in range(0, 0)) { return }` is claimed to return though the body may never run.
    all_paths_return_case!(
        for_loop_body_return_does_not_cover_all_paths,
        block_of(vec![Stmt::For {
            var: "i".into(),
            var_span: span(),
            iter: err_expr(),
            body: return_block(),
            span: span(),
            destructure_pattern: None,
            map_destructure_pattern: None,
        }]),
        false
    );
}
