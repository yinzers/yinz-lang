//! AST-to-string emitter for the Yinz formatter.
//!
//! Emits canonical output for every node type. Comments are not interleaved here —
//! a higher-level entry point handles comment-aware emission.
//!
//! Invariants:
//! - Output is a pure function of the AST (same AST → same output).
//! - Operator-precedence parentheses are preserved correctly.
//! - Backtick strings are reconstructed from `StringPart` without re-flowing content.
//! - Every AST variant is handled exhaustively — no `_ => unimplemented!()`.

use ynz_ast::nodes::{
    BinOpKind, Block, ConstDecl, ContractSig, Expr, FieldDecl, FunctionDecl, GenericParam,
    ImportDecl, ImportKind, Item, MatchPattern, MatchPatternKind, Module, OptionsDecl,
    OwnershipModifier, Param, PostfixOpKind, ReExport, ShapeDecl, Stmt, StringPart, Type,
    TypePath, UnaryOpKind,
};

use crate::comment_merge::CommentContext;
use crate::render::{fits_on_one_line, indent};

// ── Public entry point (comment-aware) ────────────────────────────────────────

/// Emit a complete module as a canonical Yinz string. Caller is responsible for any comment interleaving.
///
/// Returns a string ending with exactly one trailing newline.
pub fn emit_module(module: &Module) -> String {
    let mut out = String::new();
    for item in &module.items {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&emit_item(item, 0));
        out.push('\n');
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

// ── Item emission ─────────────────────────────────────────────────────────────

fn emit_item(item: &Item, ind: usize) -> String {
    match item {
        Item::Function(f) => emit_function(f, ind),
        Item::ShapeDecl(s) => emit_shape(s, ind),
        Item::OptionsDecl(o) => emit_options(o, ind),
        Item::ImportDecl(i) => emit_import(i),
        Item::ConstDecl(c) => emit_top_const(c, ind),
        Item::ReExport(r) => emit_reexport(r),
    }
}

fn emit_function(f: &FunctionDecl, ind: usize) -> String {
    let prefix = indent(ind);
    let export = if f.is_exported { "export " } else { "" };

    // Doc comment (if any)
    let mut out = String::new();
    if let Some(doc) = &f.doc {
        for line in doc.lines() {
            out.push_str(&format!("{prefix}/// {line}\n"));
        }
    }

    let params: Vec<String> = f.params.iter().map(emit_param).collect();
    let ret = emit_type(&f.return_type);
    // errors suffix is already carried by Type::ErrorCapable in f.return_type; do not double-emit
    let errors = "";
    let generics = emit_generics(&f.generics);

    // Try single-line signature first
    let sig_single = format!(
        "{prefix}{export}function {name}{generics}({params}) -> {ret}{errors}",
        name = f.name,
        params = params.join(", "),
    );
    let sig = if fits_on_one_line(&format!("{sig_single} {{")) {
        format!("{sig_single} {{")
    } else {
        // Multi-line params
        let ind2 = indent(ind + 1);
        let params_ml = params
            .iter()
            .map(|p| format!("{ind2}{p}"))
            .collect::<Vec<_>>()
            .join(",\n");
        format!("{prefix}{export}function {name}{generics}(\n{params_ml}\n{prefix}) -> {ret}{errors} {{",
            name = f.name)
    };

    let body = emit_block(&f.body, ind + 1);
    out.push_str(&format!("{sig}{body}{prefix}}}"));
    out
}

fn emit_param(p: &Param) -> String {
    let own = match &p.ownership {
        Some(OwnershipModifier::Share) => "share ",
        Some(OwnershipModifier::Lend) => "lend ",
        Some(OwnershipModifier::Give) => "give ",
        None => "",
    };
    format!("{own}{}: {}", p.name, emit_type(&p.ty))
}

fn emit_generics(generics: &[GenericParam]) -> String {
    if generics.is_empty() {
        return String::new();
    }
    let params = generics
        .iter()
        .map(|g| {
            if g.constraints.is_empty() {
                g.name.clone()
            } else {
                let cs = g
                    .constraints
                    .iter()
                    .map(|(name, _)| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{} follows {cs}", g.name)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("<{params}>")
}

fn emit_shape(s: &ShapeDecl, ind: usize) -> String {
    let prefix = indent(ind);
    let export = if s.is_exported { "export " } else { "" };
    let base = if s.is_base { "base " } else { "" };
    let generics = emit_generics(&s.generics);

    // Union type alias: `shape Foo = A | B`
    if let Some(alias) = &s.alias_ty {
        return format!("{prefix}{export}shape {}{generics} = {}", s.name, emit_type(alias));
    }

    let mut out = String::new();
    if let Some(doc) = &s.doc {
        for line in doc.lines() {
            out.push_str(&format!("{prefix}/// {line}\n"));
        }
    }

    let extends = s
        .extends
        .as_ref()
        .map(|(name, _)| format!(" extends {name}"))
        .unwrap_or_default();
    let follows = if s.follows.is_empty() {
        String::new()
    } else {
        let names = s
            .follows
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!(" follows {names}")
    };

    out.push_str(&format!(
        "{prefix}{export}{base}shape {}{generics}{extends}{follows} {{\n",
        s.name
    ));

    for field in &s.fields {
        out.push_str(&emit_field(field, ind + 1));
        out.push('\n');
    }

    for sig in &s.contract_sigs {
        out.push_str(&emit_contract_sig(sig, ind + 1));
        out.push('\n');
    }

    out.push_str(&format!("{prefix}}}"));
    out
}

fn emit_field(f: &FieldDecl, ind: usize) -> String {
    let prefix = indent(ind);
    let hidden = if f.is_hidden { "hidden " } else { "" };
    if let Some(doc) = &f.doc {
        let mut out = String::new();
        for line in doc.lines() {
            out.push_str(&format!("{prefix}/// {line}\n"));
        }
        let default = f
            .default
            .as_ref()
            .map(|e| format!(" = {}", emit_expr(e, 0)))
            .unwrap_or_default();
        out.push_str(&format!(
            "{prefix}{hidden}{}: {}{}",
            f.name,
            emit_type(&f.ty),
            default
        ));
        return out;
    }
    let default = f
        .default
        .as_ref()
        .map(|e| format!(" = {}", emit_expr(e, 0)))
        .unwrap_or_default();
    format!(
        "{prefix}{hidden}{}: {}{}",
        f.name,
        emit_type(&f.ty),
        default
    )
}

fn emit_contract_sig(sig: &ContractSig, ind: usize) -> String {
    let prefix = indent(ind);
    let own = match sig.receiver {
        Some(ynz_ast::nodes::ReceiverKind::Share) => "share self",
        Some(ynz_ast::nodes::ReceiverKind::Lend) => "lend self",
        Some(ynz_ast::nodes::ReceiverKind::Give) => "give self",
        None => "",
    };
    let params: Vec<String> = sig.params.iter().map(emit_param).collect();
    let all_params = if own.is_empty() {
        params.join(", ")
    } else if params.is_empty() {
        own.to_string()
    } else {
        format!("{own}, {}", params.join(", "))
    };
    let ret = emit_type(&sig.return_type);
    format!("{prefix}{}({all_params}) -> {ret}", sig.name)
}

fn emit_options(o: &OptionsDecl, ind: usize) -> String {
    let prefix = indent(ind);
    let export = if o.is_exported { "export " } else { "" };
    let ind2 = indent(ind + 1);
    let variants = o
        .variants
        .iter()
        .map(|(name, _, display)| {
            if let Some(d) = display {
                format!("{ind2}{name}: `{d}`")
            } else {
                format!("{ind2}{name}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{prefix}{export}options {} {{\n{variants}\n{prefix}}}", o.name)
}

fn emit_import(i: &ImportDecl) -> String {
    match &i.kind {
        ImportKind::Named(items) => {
            let names = items
                .iter()
                .map(|it| {
                    if it.local_name == it.exported_name {
                        it.exported_name.clone()
                    } else {
                        format!("{} as {}", it.exported_name, it.local_name)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("import {{ {names} }} from `{}`", i.source)
        }
        ImportKind::Namespace { local_name, .. } => {
            format!("import {local_name} from `{}`", i.source)
        }
    }
}

fn emit_top_const(c: &ConstDecl, ind: usize) -> String {
    let prefix = indent(ind);
    let export = if c.is_exported { "export " } else { "" };
    let ty = c
        .ty
        .as_ref()
        .map(|t| format!(": {}", emit_type(t)))
        .unwrap_or_default();
    format!(
        "{prefix}{export}const {}{ty} = {}",
        c.name,
        emit_expr(&c.value, 0)
    )
}

fn emit_reexport(r: &ReExport) -> String {
    let items = r
        .items
        .iter()
        .map(|it| match &it.alias {
            Some((alias, _)) => format!("{} as {alias}", it.name),
            None => it.name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("export {{ {items} }} from `{}`", r.source)
}

// ── Block / statement emission ────────────────────────────────────────────────

/// Emit a block's contents. Returns a string that STARTS with `\n` and ends before the
/// closing `}`. Callers append `}` themselves.
fn emit_block(block: &Block, ind: usize) -> String {
    if block.stmts.is_empty() {
        return " ".to_string(); // `function f() -> nothing { }` — single space
    }
    let mut out = String::from("\n");
    for stmt in &block.stmts {
        out.push_str(&emit_stmt(stmt, ind));
        out.push('\n');
    }
    out
}

fn emit_stmt(s: &Stmt, ind: usize) -> String {
    let prefix = indent(ind);
    match s {
        Stmt::Let {
            is_const,
            name,
            ty,
            value,
            ..
        } => {
            let kw = if *is_const { "const" } else { "let" };
            let ty_s = ty
                .as_ref()
                .map(|t| format!(": {}", emit_type(t)))
                .unwrap_or_default();
            format!("{prefix}{kw} {name}{ty_s} = {}", emit_expr(value, 0))
        }
        Stmt::Assign { target, value, .. } => {
            format!("{prefix}{target} = {}", emit_expr(value, 0))
        }
        Stmt::Return { value, .. } => {
            let v = value
                .as_ref()
                .map(|v| format!(" {}", emit_expr(v, 0)))
                .unwrap_or_default();
            format!("{prefix}return{v}")
        }
        Stmt::Expr(e) => format!("{prefix}{}", emit_expr(e, 0)),
        Stmt::If { cond, body, .. } => {
            let body_s = emit_block(body, ind + 1);
            format!("{prefix}if ({}) {{{body_s}{prefix}}}", emit_expr(cond, 0))
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            let ind2 = indent(ind + 1);
            let arms_s = arms
                .iter()
                .map(|arm| {
                    let pattern = emit_match_pattern(&arm.pattern);
                    let body = emit_block(&arm.body, ind + 2);
                    format!("{ind2}{pattern} => {{{body}{ind2}}}")
                })
                .collect::<Vec<_>>()
                .join("\n");
            let else_s = else_arm
                .as_ref()
                .map(|b| {
                    let body = emit_block(b, ind + 2);
                    format!("\n{ind2}else => {{{body}{ind2}}}")
                })
                .unwrap_or_default();
            format!(
                "{prefix}if ({}) {{\n{arms_s}{else_s}\n{prefix}}}",
                emit_expr(scrutinee, 0)
            )
        }
        Stmt::While { cond, body, .. } => {
            let body_s = emit_block(body, ind + 1);
            format!("{prefix}while ({}) {{{body_s}{prefix}}}", emit_expr(cond, 0))
        }
        Stmt::For {
            var, iter, body, ..
        } => {
            let body_s = emit_block(body, ind + 1);
            format!(
                "{prefix}for ({var} in {}) {{{body_s}{prefix}}}",
                emit_expr(iter, 0)
            )
        }
        Stmt::FieldAssign { target, value, .. } => {
            format!("{prefix}{} = {}", emit_expr(target, 0), emit_expr(value, 0))
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            format!(
                "{prefix}{}[{}] = {}",
                emit_expr(receiver, 0),
                emit_expr(index, 0),
                emit_expr(value, 0)
            )
        }
    }
}

fn emit_match_pattern(p: &MatchPattern) -> String {
    match &p.kind {
        MatchPatternKind::Value(e) => emit_expr(e, 0),
        MatchPatternKind::Is(TypePath { name, .. }) => format!("is {name}"),
        MatchPatternKind::OptionName(name) => name.clone(),
    }
}

// ── Expression emission ───────────────────────────────────────────────────────

/// Left binding power for binary operators (used for precedence-preserving parenthesization).
fn binop_lbp(op: &BinOpKind) -> u8 {
    match op {
        BinOpKind::Or => 2,
        BinOpKind::And => 4,
        BinOpKind::BitOr => 6,
        BinOpKind::BitXor => 8,
        BinOpKind::BitAnd => 10,
        BinOpKind::EqEq | BinOpKind::NotEq => 12,
        BinOpKind::Lt | BinOpKind::Gt | BinOpKind::LtEq | BinOpKind::GtEq => 14,
        BinOpKind::Shl | BinOpKind::Shr => 16,
        BinOpKind::Add | BinOpKind::Sub => 18,
        BinOpKind::Mul | BinOpKind::Div | BinOpKind::Rem => 20,
    }
}

fn binop_str(op: &BinOpKind) -> &'static str {
    match op {
        BinOpKind::Add => "+",
        BinOpKind::Sub => "-",
        BinOpKind::Mul => "*",
        BinOpKind::Div => "/",
        BinOpKind::Rem => "%",
        BinOpKind::Lt => "<",
        BinOpKind::LtEq => "<=",
        BinOpKind::Gt => ">",
        BinOpKind::GtEq => ">=",
        BinOpKind::EqEq => "==",
        BinOpKind::NotEq => "!=",
        BinOpKind::And => "&&",
        BinOpKind::Or => "||",
        BinOpKind::BitAnd => "&",
        BinOpKind::BitOr => "|",
        BinOpKind::BitXor => "^",
        BinOpKind::Shl => "<<",
        BinOpKind::Shr => ">>",
    }
}

/// Emit an expression. `min_bp` is the minimum binding power needed to avoid adding
/// parentheses — pass `0` at the call site. Internal recursive calls pass the parent's
/// binding power to correctly add parentheses only where semantics require them.
pub fn emit_expr(e: &Expr, min_bp: u8) -> String {
    match e {
        Expr::Ident(n, _) => n.clone(),
        Expr::IntLit(v, _) => v.to_string(),
        Expr::NumberLit(s, _) => s.clone(),
        Expr::BoolLit(b, _) => if *b { "true" } else { "false" }.into(),
        Expr::SelfValue { .. } => "self".into(),
        Expr::NoneLit { .. } => "none".into(),

        Expr::BinOp { op, lhs, rhs, .. } => {
            let bp = binop_lbp(op);
            let op_s = binop_str(op);
            // Left child: same bp is fine (left-associative), needs parens if strictly less
            // Right child: same bp needs parens (a + (b + c) must stay grouped)
            let lhs_s = emit_expr(lhs, bp);
            let rhs_s = emit_expr(rhs, bp + 1);
            let inner = format!("{lhs_s} {op_s} {rhs_s}");
            if bp < min_bp {
                format!("({inner})")
            } else {
                inner
            }
        }

        Expr::UnaryOp { op, operand, .. } => {
            let op_s = match op {
                UnaryOpKind::Neg => "-",
                UnaryOpKind::Not => "!",
                UnaryOpKind::BitNot => "~",
            };
            // Unary has higher precedence than all binops; no need to ever add parens around operand
            // unless it's also a unary or another special case — treat it as highest bp
            format!("{op_s}{}", emit_expr(operand, 255))
        }

        Expr::Call(c) => {
            let args: Vec<String> = c.args.iter().map(|a| emit_expr(a, 0)).collect();
            let callee = emit_expr(&c.callee, 0);
            let type_args = if let Some(ta) = &c.type_args {
                let ts = ta.iter().map(emit_type).collect::<Vec<_>>().join(", ");
                format!("<{ts}>")
            } else {
                String::new()
            };
            let args_str = args.join(", ");
            format!("{callee}{type_args}({args_str})")
        }

        Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            let recv = emit_expr(receiver, 0);
            let a: Vec<String> = args.iter().map(|a| emit_expr(a, 0)).collect();
            format!("{recv}.{method}({})", a.join(", "))
        }

        Expr::FieldAccess { receiver, field, .. } => {
            format!("{}.{field}", emit_expr(receiver, 0))
        }

        Expr::PostfixOp { receiver, op, .. } => {
            let op_s = match op {
                PostfixOpKind::Copy => "copy",
                PostfixOpKind::Freeze => "freeze",
            };
            format!("{}.{op_s}()", emit_expr(receiver, 0))
        }

        Expr::IndexAccess { receiver, index, .. } => {
            format!("{}[{}]", emit_expr(receiver, 0), emit_expr(index, 0))
        }

        Expr::ArrayLit { elements, .. } => {
            let elems: Vec<String> = elements.iter().map(|e| emit_expr(e, 0)).collect();
            let single_line = format!("[{}]", elems.join(", "));
            if fits_on_one_line(&single_line) {
                single_line
            } else {
                format!("[\n{}\n]", elems.iter().map(|e| format!("  {e}")).collect::<Vec<_>>().join(",\n"))
            }
        }

        Expr::MapLit { entries, .. } => {
            let pairs: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{}: {}", emit_expr(k, 0), emit_expr(v, 0)))
                .collect();
            let single_line = format!("{{ {} }}", pairs.join(", "));
            if fits_on_one_line(&single_line) {
                single_line
            } else {
                format!(
                    "{{\n{}\n}}",
                    pairs.iter().map(|p| format!("  {p}")).collect::<Vec<_>>().join(",\n")
                )
            }
        }

        Expr::StructLit { fields, .. } => {
            let fs: Vec<String> = fields
                .iter()
                .map(|f| format!("{}: {}", f.name, emit_expr(&f.value, 0)))
                .collect();
            let single_line = format!("{{ {} }}", fs.join(", "));
            if fits_on_one_line(&single_line) {
                single_line
            } else {
                format!(
                    "{{\n{}\n}}",
                    fs.iter().map(|f| format!("  {f}")).collect::<Vec<_>>().join(",\n")
                )
            }
        }

        // Plain string literal (from lexer internals — not user-facing; handled defensively)
        Expr::StringLit(bytes, _) => format!(
            "`{}`",
            std::str::from_utf8(bytes).expect("lexer guarantees source bytes are valid UTF-8")
        ),

        // Backtick strings: reconstruct from parts — content is opaque and never re-flowed
        Expr::InterpolatedString(parts, _) => {
            let mut s = String::from('`');
            for part in parts {
                match part {
                    StringPart::Lit(bytes, _) => s.push_str(
                        std::str::from_utf8(bytes)
                            .expect("lexer guarantees source bytes are valid UTF-8"),
                    ),
                    StringPart::Expr(expr, _) => {
                        s.push_str(&format!("${{{}}}", emit_expr(expr, 0)))
                    }
                }
            }
            s.push('`');
            s
        }

        Expr::Is { expr, ty, .. } => format!("{} is {}", emit_expr(expr, 0), ty.name),

        Expr::Wait(inner, _) => format!("wait {}", emit_expr(inner, 0)),

        Expr::Background(inner, _) => format!("background {}", emit_expr(inner, 0)),

        Expr::Error(_) => "<ERROR>".into(),
    }
}

// ── Type emission ─────────────────────────────────────────────────────────────

pub fn emit_type(ty: &Type) -> String {
    match ty {
        Type::Nothing => "nothing".into(),
        Type::Int => "int".into(),
        Type::Float => "float".into(),
        Type::Bool => "boolean".into(),
        // precision is always 34 in v0.1 (non-34 parsed with a diagnostic); formatter
        // emits `number` for all precision values — update when M8 permits precision[N].
        Type::Number { .. } => "number".into(),
        Type::Named(n, _) => n.clone(),
        Type::Maybe { inner, .. } => format!("maybe<{}>", emit_type(inner)),
        Type::Generic { name, args, .. } => {
            let a = args.iter().map(emit_type).collect::<Vec<_>>().join(", ");
            format!("{name}<{a}>")
        }
        Type::Union { variants, .. } => {
            variants.iter().map(emit_type).collect::<Vec<_>>().join(" | ")
        }
        Type::Dynamic { contract, .. } => format!("dynamic {contract}"),
        Type::TypeParam { name, .. } => name.clone(),
        Type::ErrorCapable { inner, .. } => format!("{} errors", emit_type(inner)),
        Type::Sensitive(inner) => format!("sensitive {}", emit_type(inner)),
        Type::SelfType { .. } => "Self".into(),
        Type::Range { element, .. } => format!("range<{}>", emit_type(element)),
        Type::AnonShape { fields, .. } => {
            let fs = fields
                .iter()
                .map(|f| format!("{}: {}", f.name, emit_type(&f.ty)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ {fs} }}")
        }
        Type::Error => "<ERROR>".into(),
    }
}

// ── Comment-aware emission ─────────────────────────────────────────
//
// These functions thread a `CommentContext` through the AST walk, attaching
// `//` comments at statement, item, and field boundaries.
//
// Architecture:
// - Module items get leading+floating comments before them and inline comments
//   after them (on the same source line as the item's opening token).
// - Block statements get leading+inline comments the same way.
// - Shape fields get inline comments.
// - Doc comments are emitted from the AST fields (`FunctionDecl.doc`,
//   `ShapeDecl.doc`, `FieldDecl.doc`) — NOT from the trivia vec — to avoid
//   double-emission.

/// Emit a complete module with comments interleaved from the `CommentContext`.
///
/// Returns a string ending with exactly one trailing newline.
pub fn emit_module_with_comments(module: &Module, ctx: &CommentContext<'_>) -> String {
    if module.items.is_empty() {
        return emit_floating_comments(ctx, 0, ctx.source.len());
    }

    let mut out = String::new();
    let mut prev_end: usize = 0;

    for item in &module.items {
        let item_start = item_span_start(item);

        // Collect ALL comments in [prev_end, item_start), including doc-comments.
        // Doc comments are emitted via trivia (in source-byte order) rather than
        // via the AST's f.doc/s.doc fields to preserve ordering when a `//` comment
        // and a `///` comment appear in mixed order, and to preserve "split doc blocks"
        // (two `///` blocks separated by a blank line).
        let (leading, floating) = ctx.between(prev_end, item_start);

        // Blank-line separator between items
        if !out.is_empty() {
            out.push('\n');
        }

        // Floating comments: a blank line BEFORE and AFTER the comment group (canonical form).
        // This ensures the floating classification is stable on re-format runs (idempotent).
        if !floating.is_empty() {
            for c in &floating {
                out.push_str(&normalize_comment_text(&c.text));
                out.push('\n');
            }
            // Blank line after the floating group — keeps ≥1 blank between comment and item
            out.push('\n');
        }

        // Leading comments (directly above item, no blank between comment and item)
        for c in &leading {
            out.push_str(&normalize_comment_text(&c.text));
            out.push('\n');
        }

        // Item itself
        let item_text = emit_item_with_comments(item, 0, ctx);
        out.push_str(&item_text);
        out.push('\n');

        prev_end = item_span_end_for_comments(item, ctx);
    }

    // Any trailing comments after the last item
    let last_item_end = prev_end;
    let trailing = emit_floating_comments(ctx, last_item_end, ctx.source.len());
    if !trailing.is_empty() {
        out.push_str(&trailing);
    }

    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn emit_floating_comments(ctx: &CommentContext<'_>, from: usize, to: usize) -> String {
    let mut out = String::new();
    for c in ctx.comments.iter().filter(|c| {
        c.span.start >= from && c.span.start < to && !CommentContext::is_doc_comment(c)
    }) {
        out.push_str(&normalize_comment_text(&c.text));
        out.push('\n');
    }
    out
}

fn normalize_comment_text(text: &str) -> String {
    // Strip trailing \r (CRLF input normalization)
    text.trim_end_matches('\r').to_string()
}

fn item_span_start(item: &Item) -> usize {
    match item {
        Item::Function(f) => f.span.start,
        Item::ShapeDecl(s) => s.span.start,
        Item::ImportDecl(i) => i.span.start,
        Item::ConstDecl(c) => c.span.start,
        Item::ReExport(r) => r.span.start,
        Item::OptionsDecl(o) => o.span.start,
    }
}

fn item_span_end_for_comments(item: &Item, ctx: &CommentContext<'_>) -> usize {
    // Use body.span.end for functions to avoid the FunctionDecl.span.end pointing
    // to the next token after `}`.
    match item {
        Item::Function(f) => f.body.span.end,
        Item::ShapeDecl(s) => s.span.end,
        Item::ImportDecl(i) => ctx.source[i.span.start..]
            .find('\n')
            .map(|x| i.span.start + x)
            .unwrap_or(ctx.source.len()),
        Item::ConstDecl(c) => ctx.source[c.span.start..]
            .find('\n')
            .map(|x| c.span.start + x)
            .unwrap_or(ctx.source.len()),
        Item::ReExport(r) => r.span.end,
        Item::OptionsDecl(o) => o.span.end,
    }
}

fn emit_item_with_comments(item: &Item, ind: usize, ctx: &CommentContext<'_>) -> String {
    match item {
        Item::Function(f) => emit_function_with_comments(f, ind, ctx),
        Item::ShapeDecl(s) => emit_shape_with_comments(s, ind, ctx),
        // Non-body items: use the plain emitter (comments handled at module level)
        other => emit_item(other, ind),
    }
}

fn emit_function_with_comments(f: &FunctionDecl, ind: usize, ctx: &CommentContext<'_>) -> String {
    let prefix = indent(ind);
    let export = if f.is_exported { "export " } else { "" };

    // Doc comments are emitted via trivia at the module level (in source order),
    // NOT here — so we can preserve mixed `///` + `//` ordering and split doc blocks.
    let mut out = String::new();
    let params: Vec<String> = f.params.iter().map(emit_param).collect();
    let ret = emit_type(&f.return_type);
    // errors suffix is already carried by Type::ErrorCapable in f.return_type; do not double-emit
    let errors = "";
    let generics = emit_generics(&f.generics);

    let sig_single = format!(
        "{prefix}{export}function {name}{generics}({params}) -> {ret}{errors}",
        name = f.name,
        params = params.join(", "),
    );
    let sig = if fits_on_one_line(&format!("{sig_single} {{")) {
        format!("{sig_single} {{")
    } else {
        let ind2 = indent(ind + 1);
        let params_ml = params
            .iter()
            .map(|p| format!("{ind2}{p}"))
            .collect::<Vec<_>>()
            .join(",\n");
        format!("{prefix}{export}function {name}{generics}(\n{params_ml}\n{prefix}) -> {ret}{errors} {{",
            name = f.name)
    };

    // Use the function's span start as the block anchor — any comment on a line after
    // the `function` keyword line is eligible as a leading comment for the first statement.
    // Avoids rfind('{') which can land on `{` characters inside comments.
    let body = emit_block_with_comments(&f.body, ind + 1, ctx, f.span.start);
    out.push_str(&format!("{sig}{body}{prefix}}}"));
    out
}

fn emit_shape_with_comments(s: &ShapeDecl, ind: usize, ctx: &CommentContext<'_>) -> String {
    let prefix = indent(ind);
    let export = if s.is_exported { "export " } else { "" };
    let base = if s.is_base { "base " } else { "" };
    let generics = emit_generics(&s.generics);

    if let Some(alias) = &s.alias_ty {
        return format!("{prefix}{export}shape {}{generics} = {}", s.name, emit_type(alias));
    }

    // Doc comments emitted via trivia at module level — not here.
    let mut out = String::new();
    let extends = s
        .extends
        .as_ref()
        .map(|(n, _)| format!(" extends {n}"))
        .unwrap_or_default();
    let follows = if s.follows.is_empty() {
        String::new()
    } else {
        let ns = s.follows.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ");
        format!(" follows {ns}")
    };

    out.push_str(&format!(
        "{prefix}{export}{base}shape {}{generics}{extends}{follows} {{\n",
        s.name
    ));

    for field in &s.fields {
        // Inline comment on the field's source line
        let inline = ctx
            .inline_comment_after(field.span.start)
            .unwrap_or_default();
        let field_text = emit_field(field, ind + 1);
        out.push_str(&format!("{field_text}{inline}\n"));
    }

    for sig in &s.contract_sigs {
        out.push_str(&emit_contract_sig(sig, ind + 1));
        out.push('\n');
    }

    out.push_str(&format!("{prefix}}}"));
    out
}

/// Emit a block with comment-aware statement rendering.
///
/// `block_open_pos`: the byte position of the opening `{` (used to anchor
/// the leading-comment window for the first statement).
fn emit_block_with_comments(
    block: &Block,
    ind: usize,
    ctx: &CommentContext<'_>,
    block_open_pos: usize,
) -> String {
    if block.stmts.is_empty() {
        return " ".to_string();
    }

    let mut out = String::from("\n");
    let mut prev_end = block_open_pos;

    for stmt in &block.stmts {
        let stmt_start = stmt_span_start(stmt);
        let prefix = indent(ind);

        // Leading/floating comments for this statement (include all kinds, no doc-comment filter)
        let (leading, floating) = ctx.between(prev_end, stmt_start);

        // Floating comments (there's a blank line before this stmt — emit them first)
        for c in &floating {
            out.push_str(&format!("{prefix}{}\n", normalize_comment_text(&c.text)));
        }

        // Leading comments (directly above stmt)
        for c in &leading {
            out.push_str(&format!("{prefix}{}\n", normalize_comment_text(&c.text)));
        }

        // Inline comment on the same source line as this stmt
        let inline = ctx.inline_comment_after(stmt_start).unwrap_or_default();

        // Emit statement
        let stmt_text = emit_stmt_with_inline(stmt, ind, ctx, &inline);
        out.push_str(&stmt_text);
        out.push('\n');

        // Update prev_end:
        // - Compound stmts (if/while/for/match): advance to the closing `}` position so that
        //   comments INSIDE the body are not re-claimed as leading comments for the next stmt.
        // - Simple stmts: advance to end of first source line (span.end may cross lines for
        //   stmts ending in InterpolatedString).
        prev_end = match stmt {
            Stmt::If { body, .. } | Stmt::While { body, .. } | Stmt::For { body, .. } => {
                body.span.end
            }
            Stmt::Match { arms, else_arm, .. } => {
                if let Some(b) = else_arm {
                    b.span.end
                } else if let Some(a) = arms.last() {
                    a.body.span.end
                } else {
                    ctx.source[stmt_start..].find('\n').map(|i| stmt_start + i).unwrap_or(ctx.source.len())
                }
            }
            _ => ctx.source[stmt_start..]
                .find('\n')
                .map(|i| stmt_start + i)
                .unwrap_or(ctx.source.len()),
        };
    }
    out
}

fn stmt_span_start(s: &Stmt) -> usize {
    match s {
        Stmt::Let { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::If { span, .. }
        | Stmt::Match { span, .. }
        | Stmt::While { span, .. }
        | Stmt::For { span, .. }
        | Stmt::FieldAssign { span, .. }
        | Stmt::IndexAssign { span, .. } => span.start,
        Stmt::Expr(e) => e.span().start,
    }
}

/// Emit a statement, appending an optional inline comment (already formatted as `  // text`).
///
/// When the full line (code + inline comment) would exceed `LINE_WIDTH`, the inline comment
/// is moved to its own line directly above the statement at the same indent. This preserves
/// the comment's locality without extending the line past the width limit.
fn emit_stmt_with_inline(s: &Stmt, ind: usize, ctx: &CommentContext<'_>, inline: &str) -> String {
    use crate::render::LINE_WIDTH;
    let prefix = indent(ind);

    /// Helper: if `code + inline` exceeds LINE_WIDTH, move inline comment above code.
    fn place(prefix: &str, code: String, inline: &str) -> String {
        if inline.is_empty() {
            return code;
        }
        if code.len() + inline.len() > LINE_WIDTH {
            // Inline loses meaning when the line is too long — move above
            let comment = inline.trim_start();
            format!("{prefix}{comment}\n{code}")
        } else {
            format!("{code}{inline}")
        }
    }

    match s {
        Stmt::Let { is_const, name, ty, value, .. } => {
            let kw = if *is_const { "const" } else { "let" };
            let ty_s = ty.as_ref().map(|t| format!(": {}", emit_type(t))).unwrap_or_default();
            let code = format!("{prefix}{kw} {name}{ty_s} = {}", emit_expr(value, 0));
            place(&prefix, code, inline)
        }
        Stmt::Assign { target, value, .. } => {
            let code = format!("{prefix}{target} = {}", emit_expr(value, 0));
            place(&prefix, code, inline)
        }
        Stmt::Return { value, .. } => {
            let v = value.as_ref().map(|v| format!(" {}", emit_expr(v, 0))).unwrap_or_default();
            let code = format!("{prefix}return{v}");
            place(&prefix, code, inline)
        }
        Stmt::Expr(e) => {
            let code = format!("{prefix}{}", emit_expr(e, 0));
            place(&prefix, code, inline)
        }
        Stmt::If { cond, body, span, .. } => {
            // Use the `if` keyword position as block anchor (safe — no rfind that can hit
            // `{` inside comment text).
            let body_s = emit_block_with_comments(body, ind + 1, ctx, span.start);
            format!("{prefix}if ({}) {{{body_s}{prefix}}}", emit_expr(cond, 0))
        }
        Stmt::Match { scrutinee, arms, else_arm, span, .. } => {
            let ind2 = indent(ind + 1);
            let arms_s = arms
                .iter()
                .map(|arm| {
                    let pattern = emit_match_pattern(&arm.pattern);
                    let body = emit_block_with_comments(&arm.body, ind + 2, ctx, arm.arrow_span.start);
                    format!("{ind2}{pattern} => {{{body}{ind2}}}")
                })
                .collect::<Vec<_>>()
                .join("\n");
            let else_s = else_arm.as_ref().map(|b| {
                let body = emit_block_with_comments(b, ind + 2, ctx, span.start);
                format!("\n{ind2}else => {{{body}{ind2}}}")
            }).unwrap_or_default();
            format!("{prefix}if ({}) {{\n{arms_s}{else_s}\n{prefix}}}", emit_expr(scrutinee, 0))
        }
        Stmt::While { cond, body, span, .. } => {
            let body_s = emit_block_with_comments(body, ind + 1, ctx, span.start);
            format!("{prefix}while ({}) {{{body_s}{prefix}}}", emit_expr(cond, 0))
        }
        Stmt::For { var, iter, body, span, .. } => {
            let body_s = emit_block_with_comments(body, ind + 1, ctx, span.start);
            format!("{prefix}for ({var} in {}) {{{body_s}{prefix}}}", emit_expr(iter, 0))
        }
        Stmt::FieldAssign { target, value, .. } => {
            format!("{prefix}{} = {}{inline}", emit_expr(target, 0), emit_expr(value, 0))
        }
        Stmt::IndexAssign { receiver, index, value, .. } => {
            format!(
                "{prefix}{}[{}] = {}{}",
                emit_expr(receiver, 0),
                emit_expr(index, 0),
                emit_expr(value, 0),
                inline
            )
        }
    }
}
