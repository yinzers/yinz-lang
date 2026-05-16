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

fn analyze_stmts(stmts: &[Stmt]) -> ReturnAnalysis {
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
        }
    }

    ReturnAnalysis { all_paths_return: definite_return, dead_code: dead }
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
        | Stmt::FieldAssign { span, .. } => span.clone(),
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
        Block { stmts: vec![], span: span() }
    }

    fn return_block() -> Block {
        Block {
            stmts: vec![Stmt::Return { value: None, span: span() }],
            span: span(),
        }
    }

    #[test]
    fn empty_block_does_not_return() {
        // WHY: an empty block has no return — typeck must flag this as missing-return
        // for non-nothing functions. The analysis must not falsely claim coverage.
        let result = analyze_return_paths(&empty_block());
        assert!(!result.all_paths_return);
        assert!(result.dead_code.is_empty());
    }

    #[test]
    fn single_return_stmt_returns_on_all_paths() {
        // WHY: the simplest valid case — one explicit return covers all paths.
        let block = Block {
            stmts: vec![Stmt::Return { value: None, span: span() }],
            span: span(),
        };
        let result = analyze_return_paths(&block);
        assert!(result.all_paths_return);
    }

    #[test]
    fn dead_code_after_return_is_detected() {
        // WHY: `return; print(1)` — the print is unreachable. The analysis must
        // identify the print's span so typeck can emit a dead-code warning.
        let block = Block {
            stmts: vec![
                Stmt::Return { value: None, span: span() },
                Stmt::Expr(err_expr()),
            ],
            span: span(),
        };
        let result = analyze_return_paths(&block);
        assert!(result.all_paths_return);
        assert_eq!(result.dead_code.len(), 1, "dead-code span expected for stmt after return");
    }

    #[test]
    fn simple_if_body_return_does_not_cover_all_paths() {
        // WHY: `if (cond) { return x }` — the false branch falls through.
        // The analysis must conservatively conclude NOT all paths return.
        let block = Block {
            stmts: vec![Stmt::If {
                cond: err_expr(),
                body: return_block(),
                span: span(),
            }],
            span: span(),
        };
        let result = analyze_return_paths(&block);
        assert!(!result.all_paths_return, "simple if does NOT guarantee return");
    }

    #[test]
    fn match_with_else_all_arms_return_covers_all_paths() {
        // WHY: `if (x) { 1 => return 1; else => return 0 }` — every arm returns.
        // The analysis must recognize this as covering all paths.
        let arm = MatchArm {
            pattern: MatchPattern { kind: MatchPatternKind::Value(err_expr()), span: span() },
            body: return_block(),
            arrow_span: span(),
        };
        let block = Block {
            stmts: vec![Stmt::Match {
                scrutinee: err_expr(),
                arms: vec![arm],
                else_arm: Some(return_block()),
                span: span(),
            }],
            span: span(),
        };
        let result = analyze_return_paths(&block);
        assert!(result.all_paths_return, "match with else + all-returning arms covers all paths");
    }

    #[test]
    fn match_without_else_does_not_cover_all_paths() {
        // WHY: `if (x) { 1 => return 1 }` — no else_arm means fall-through.
        // The analysis must NOT count this as covering all paths.
        let arm = MatchArm {
            pattern: MatchPattern { kind: MatchPatternKind::Value(err_expr()), span: span() },
            body: return_block(),
            arrow_span: span(),
        };
        let block = Block {
            stmts: vec![Stmt::Match {
                scrutinee: err_expr(),
                arms: vec![arm],
                else_arm: None,
                span: span(),
            }],
            span: span(),
        };
        let result = analyze_return_paths(&block);
        assert!(!result.all_paths_return, "match without else has a fall-through path");
    }

    #[test]
    fn while_loop_body_return_does_not_cover_all_paths() {
        // WHY: `while (cond) { return x }` — the loop may not execute at all.
        // The analysis must treat while/for as not guaranteeing any path.
        let block = Block {
            stmts: vec![Stmt::While {
                cond: err_expr(),
                body: return_block(),
                span: span(),
            }],
            span: span(),
        };
        let result = analyze_return_paths(&block);
        assert!(!result.all_paths_return, "while loop does NOT guarantee return");
    }

    #[test]
    fn for_loop_body_return_does_not_cover_all_paths() {
        // WHY: same as while — `for (i in range(0, 0)) { return x }` — zero
        // iterations means the body never runs, so return is not guaranteed.
        let block = Block {
            stmts: vec![Stmt::For {
                var: "i".into(),
                var_span: span(),
                iter: err_expr(),
                body: return_block(),
                span: span(),
            }],
            span: span(),
        };
        let result = analyze_return_paths(&block);
        assert!(!result.all_paths_return, "for loop does NOT guarantee return");
    }

    #[test]
    fn dead_code_inside_match_arm_is_reported() {
        // WHY: `if (x) { 1 => { return 1; print(2) }; else => return 0 }` —
        // `print(2)` is dead inside the arm body. The analysis must surface the
        // inner dead-code span even though the outer block itself is all-returning.
        let arm_with_dead = {
            let body = Block {
                stmts: vec![
                    Stmt::Return { value: None, span: span() },
                    Stmt::Expr(err_expr()),
                ],
                span: span(),
            };
            MatchArm {
                pattern: MatchPattern { kind: MatchPatternKind::Value(err_expr()), span: span() },
                body,
                arrow_span: span(),
            }
        };
        let block = Block {
            stmts: vec![Stmt::Match {
                scrutinee: err_expr(),
                arms: vec![arm_with_dead],
                else_arm: Some(return_block()),
                span: span(),
            }],
            span: span(),
        };
        let result = analyze_return_paths(&block);
        assert!(result.all_paths_return);
        assert_eq!(result.dead_code.len(), 1, "dead code inside arm body must be reported");
    }
}
