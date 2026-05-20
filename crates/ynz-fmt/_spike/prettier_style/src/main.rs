/// Prettier-style Yinz spike — full AST reflow.
///
/// Discards ALL original whitespace and reconstructs output purely from the AST.
/// Comment placement by byte-position matching on the original source text.
///
/// Decision criterion: same AST → same output, always. Trivially idempotent.
use std::{env, fs, process};

use ynz_ast::nodes::{StringPart, *};
use ynz_parser::{parse_query, CompilerDb, SourceFile};

const LINE_WIDTH: usize = 100;

// ── Comment extraction ──────────────────────────────────────────────────────

fn extract_comments(source: &str) -> Vec<(usize, String)> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        // Skip backtick strings entirely (content is opaque — no comments inside)
        if bytes[i] == b'`' {
            i += 1;
            let mut depth = 0usize;
            while i < bytes.len() {
                if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == b'}' && depth > 0 {
                    depth -= 1;
                    i += 1;
                } else if bytes[i] == b'`' && depth == 0 {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // Doc-comment `///` — skip (handled by AST)
        if i + 2 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' && bytes[i + 2] == b'/'
        {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // Line comment `//`
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            let start = i;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            out.push((start, source[start..i].to_string()));
            continue;
        }
        i += 1;
    }
    out
}

fn line_of(source: &str, byte: usize) -> usize {
    source[..byte.min(source.len())].bytes().filter(|&b| b == b'\n').count()
}

// ── Type emission ───────────────────────────────────────────────────────────

fn emit_type(ty: &Type) -> String {
    match ty {
        Type::Nothing => "nothing".into(),
        Type::Int => "int".into(),
        Type::Float => "float".into(),
        Type::Bool => "boolean".into(),
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
        _ => "unknown".into(),
    }
}

// ── Expr emission ───────────────────────────────────────────────────────────

fn emit_expr(e: &Expr) -> String {
    match e {
        Expr::Ident(n, _) => n.clone(),
        Expr::IntLit(v, _) => v.to_string(),
        Expr::NumberLit(s, _) => s.clone(),
        Expr::BoolLit(b, _) => if *b { "true" } else { "false" }.into(),
        Expr::SelfValue { .. } => "self".into(),
        Expr::NoneLit { .. } => "none".into(),
        Expr::BinOp { op, lhs, rhs, .. } => {
            let op_s = match op {
                BinOpKind::Add => "+", BinOpKind::Sub => "-",
                BinOpKind::Mul => "*", BinOpKind::Div => "/", BinOpKind::Rem => "%",
                BinOpKind::Lt => "<", BinOpKind::LtEq => "<=",
                BinOpKind::Gt => ">", BinOpKind::GtEq => ">=",
                BinOpKind::EqEq => "==", BinOpKind::NotEq => "!=",
                BinOpKind::And => "&&", BinOpKind::Or => "||",
                BinOpKind::BitAnd => "&", BinOpKind::BitOr => "|",
                BinOpKind::BitXor => "^", BinOpKind::Shl => "<<", BinOpKind::Shr => ">>",
            };
            format!("{} {op_s} {}", emit_expr(lhs), emit_expr(rhs))
        }
        Expr::UnaryOp { op, operand, .. } => {
            let op_s = match op {
                UnaryOpKind::Neg => "-", UnaryOpKind::Not => "!", UnaryOpKind::BitNot => "~",
            };
            format!("{op_s}{}", emit_expr(operand))
        }
        Expr::Call(c) => {
            let args = c.args.iter().map(emit_expr).collect::<Vec<_>>().join(", ");
            format!("{}({args})", emit_expr(&c.callee))
        }
        Expr::MethodCall { receiver, method, args, .. } => {
            let a = args.iter().map(emit_expr).collect::<Vec<_>>().join(", ");
            format!("{}.{method}({a})", emit_expr(receiver))
        }
        Expr::FieldAccess { receiver, field, .. } => {
            format!("{}.{field}", emit_expr(receiver))
        }
        Expr::PostfixOp { receiver, op, .. } => {
            let op_s = match op { PostfixOpKind::Copy => "copy", PostfixOpKind::Freeze => "freeze" };
            format!("{}.{op_s}()", emit_expr(receiver))
        }
        Expr::IndexAccess { receiver, index, .. } => {
            format!("{}[{}]", emit_expr(receiver), emit_expr(index))
        }
        Expr::ArrayLit { elements, .. } => {
            let e = elements.iter().map(emit_expr).collect::<Vec<_>>().join(", ");
            format!("[{e}]")
        }
        Expr::StructLit { fields, .. } => {
            let f = fields.iter()
                .map(|f| format!("{}: {}", f.name, emit_expr(&f.value)))
                .collect::<Vec<_>>().join(", ");
            format!("{{ {f} }}")
        }
        // Plain string literal (internal — not user-facing in Yinz; handle defensively)
        Expr::StringLit(bytes, _) => {
            format!("`{}`", String::from_utf8_lossy(bytes))
        }
        // Backtick strings: reconstruct from parts to include the backtick delimiters
        Expr::InterpolatedString(parts, _) => {
            let mut s = String::from('`');
            for part in parts {
                match part {
                    StringPart::Lit(bytes, _) => s.push_str(&String::from_utf8_lossy(bytes)),
                    StringPart::Expr(expr, _) => {
                        s.push_str(&format!("${{{}}}", emit_expr(expr)))
                    }
                }
            }
            s.push('`');
            s
        }
        Expr::Is { expr, ty, .. } => format!("{} is {}", emit_expr(expr), ty.name),
        Expr::Wait(inner, _) => format!("wait {}", emit_expr(inner)),
        Expr::Background(inner, _) => format!("background {}", emit_expr(inner)),
        _ => {
            // Unknown node: emit error marker so we know coverage is incomplete
            "<UNHANDLED_EXPR>".into()
        }
    }
}

// ── Statement emission ──────────────────────────────────────────────────────

fn emit_stmt(s: &Stmt, indent: usize, comments: &[(usize, String)], src: &str) -> String {
    let ind = "  ".repeat(indent);
    let node_start = match s {
        Stmt::Let { span, .. } | Stmt::Assign { span, .. } | Stmt::Return { span, .. }
        | Stmt::If { span, .. } | Stmt::While { span, .. } | Stmt::For { span, .. }
        | Stmt::Match { span, .. } | Stmt::FieldAssign { span, .. }
        | Stmt::IndexAssign { span, .. } => span.start,
        Stmt::Expr(e) => e.span().start,
    };

    // Find the actual end of the source line where the statement starts.
    // Using node_end for line detection is wrong for InterpolatedString (its span.end
    // points past the closing backtick to the start of the next token on the next line).
    let code_line_end = src[node_start..].find('\n').map(|i| node_start + i).unwrap_or(src.len());

    let inline = comments.iter().find(|(s, _)| {
        *s >= node_start && *s < code_line_end
    }).map(|(_, t)| format!("  {t}")).unwrap_or_default();

    let code = match s {
        Stmt::Let { is_const, name, ty, value, .. } => {
            let kw = if *is_const { "const" } else { "let" };
            let ty_s = ty.as_ref().map(|t| format!(": {}", emit_type(t))).unwrap_or_default();
            format!("{ind}{kw} {name}{ty_s} = {}{inline}", emit_expr(value))
        }
        Stmt::Assign { target, value, .. } => {
            format!("{ind}{target} = {}{inline}", emit_expr(value))
        }
        Stmt::Return { value, .. } => {
            let v = value.as_ref().map(|v| format!(" {}", emit_expr(v))).unwrap_or_default();
            format!("{ind}return{v}{inline}")
        }
        Stmt::Expr(e) => format!("{ind}{}{inline}", emit_expr(e)),
        Stmt::If { cond, body, .. } => {
            let brace = src[..body.span.start].rfind('{').unwrap_or(0);
            let body_s = emit_block(&body.stmts, indent + 1, comments, src, brace);
            format!("{ind}if ({}) {{{body_s}{ind}}}", emit_expr(cond))
        }
        Stmt::While { cond, body, .. } => {
            let brace = src[..body.span.start].rfind('{').unwrap_or(0);
            let body_s = emit_block(&body.stmts, indent + 1, comments, src, brace);
            format!("{ind}while ({}) {{{body_s}{ind}}}", emit_expr(cond))
        }
        Stmt::For { var, iter, body, .. } => {
            let brace = src[..body.span.start].rfind('{').unwrap_or(0);
            let body_s = emit_block(&body.stmts, indent + 1, comments, src, brace);
            format!("{ind}for ({var} in {}) {{{body_s}{ind}}}", emit_expr(iter))
        }
        Stmt::FieldAssign { target, value, .. } => {
            format!("{ind}{} = {}{inline}", emit_expr(target), emit_expr(value))
        }
        _ => format!("{ind}<UNHANDLED_STMT>"),
    };
    code
}

fn emit_block(stmts: &[Stmt], indent: usize, comments: &[(usize, String)], src: &str, block_start: usize) -> String {
    if stmts.is_empty() {
        return " ".into();
    }
    let mut out = String::from("\n");
    let mut prev_end = block_start;

    for stmt in stmts {
        let stmt_start = match stmt {
            Stmt::Let { span, .. } | Stmt::Assign { span, .. } | Stmt::Return { span, .. }
            | Stmt::If { span, .. } | Stmt::While { span, .. } | Stmt::For { span, .. }
            | Stmt::Match { span, .. } | Stmt::FieldAssign { span, .. }
            | Stmt::IndexAssign { span, .. } => span.start,
            Stmt::Expr(e) => e.span().start,
        };
        // End of the stmt's actual source line — used to update prev_end correctly.
        // Using span.end is WRONG for stmts whose value is an InterpolatedString: the
        // InterpolatedString's span.end crosses into the next line (it points to the
        // start of the next token). Using the \n position keeps prev_end on the stmt's line.
        let stmt_line_end = src[stmt_start..].find('\n').map(|i| stmt_start + i).unwrap_or(src.len());
        let stmt_line = line_of(src, stmt_start);
        let prev_line = line_of(src, prev_end);

        // Emit leading comments for this stmt
        let ind = "  ".repeat(indent);
        for (cstart, ctext) in comments.iter() {
            let cline = line_of(src, *cstart);
            if *cstart < stmt_start && cline < stmt_line && cline + 3 > stmt_line && cline > prev_line {
                out.push_str(&format!("{ind}{ctext}\n"));
            }
        }

        out.push_str(&emit_stmt(stmt, indent, comments, src));
        out.push('\n');
        prev_end = stmt_line_end;
    }
    out
}

// ── Item emission ───────────────────────────────────────────────────────────

fn emit_param(p: &Param) -> String {
    let own = match &p.ownership {
        Some(OwnershipModifier::Share) => "share ",
        Some(OwnershipModifier::Lend) => "lend ",
        Some(OwnershipModifier::Give) => "give ",
        None => "",
    };
    format!("{own}{}: {}", p.name, emit_type(&p.ty))
}

fn emit_function(f: &FunctionDecl, comments: &[(usize, String)], src: &str) -> String {
    let params: Vec<String> = f.params.iter().map(emit_param).collect();
    let ret = emit_type(&f.return_type);
    let errors = if f.errors_capable { " errors" } else { "" };

    // PRETTIER-STYLE: always check rendered length, break if > LINE_WIDTH
    let sig_single = format!("function {}({}) -> {}{ret}{errors} {{", f.name, params.join(", "), "");
    let sig = if sig_single.len() <= LINE_WIDTH {
        format!("function {}({}) -> {ret}{errors} {{", f.name, params.join(", "))
    } else {
        let ind2 = "  ";
        let params_ml = params.iter().map(|p| format!("{ind2}{p}")).collect::<Vec<_>>().join(",\n");
        format!("function {}(\n{params_ml}\n) -> {ret}{errors} {{", f.name)
    };

    // block.span.start points to the first token INSIDE the block (after {).
    // Scan backwards to find the actual { position — that's the anchor for leading-comment detection.
    let brace_pos = src[..f.body.span.start].rfind('{').unwrap_or(0);
    let body = emit_block(&f.body.stmts, 1, comments, src, brace_pos);
    format!("{sig}{body}}}")
}

fn emit_shape(s: &ShapeDecl, comments: &[(usize, String)], src: &str) -> String {
    if let Some(alias) = &s.alias_ty {
        return format!("shape {} = {}", s.name, emit_type(alias));
    }
    let mut out = format!("shape {} {{\n", s.name);
    for field in &s.fields {
        let field_line_end = src[field.span.start..].find('\n').map(|i| field.span.start + i).unwrap_or(src.len());
        let inline = comments.iter().find(|(cs, _)| {
            *cs >= field.span.start && *cs < field_line_end
        }).map(|(_, t)| format!("  {t}")).unwrap_or_default();
        out.push_str(&format!("  {}: {}{inline}\n", field.name, emit_type(&field.ty)));
    }
    out.push('}');
    out
}

// ── Module emission ─────────────────────────────────────────────────────────

fn format_source(source: &str) -> String {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, "<spike>".into(), source.into());
    let output = parse_query(&db, sf);
    if !output.diagnostics.is_empty() {
        eprintln!("parse errors in input — cannot format");
        return source.to_string();
    }

    let comments = extract_comments(source);
    let module = &output.module;
    let mut out = String::new();
    let mut prev_end: usize = 0;

    for item in &module.items {
        let item_start = match item {
            Item::Function(f) => f.span.start,
            Item::ShapeDecl(s) => s.span.start,
            Item::ImportDecl(i) => i.span.start,
            Item::ConstDecl(c) => c.span.start,
            Item::ReExport(r) => r.span.start,
            Item::OptionsDecl(o) => o.span.start,
        };

        // Collect leading comments for this item (comments between prev item and this item)
        let leading: Vec<&str> = comments.iter()
            .filter(|(cs, _)| {
                let cline = line_of(source, *cs);
                let item_line = line_of(source, item_start);
                let prev_line = line_of(source, prev_end);
                *cs < item_start && cline < item_line && cline >= prev_line
            })
            .map(|(_, ct)| ct.as_str())
            .collect();

        // Blank-line separator comes BEFORE the comment+item block (canonical: blank line
        // separates items; leading comments attach directly to their item with no blank between).
        if !out.is_empty() {
            out.push('\n');
        }

        // Leading comments with no blank between them and the item they describe
        for ct in &leading {
            out.push_str(ct);
            out.push('\n');
        }

        let item_s = match item {
            Item::Function(f) => emit_function(f, &comments, source),
            Item::ShapeDecl(s) => emit_shape(s, &comments, source),
            Item::ImportDecl(i) => {
                // Preserve as original source bytes (imports are simple)
                source.get(i.span.start..i.span.end).unwrap_or("").trim().to_string()
            }
            Item::ConstDecl(c) => {
                format!("const {}: {} = {}", c.name, emit_type(c.ty.as_ref().unwrap_or(&Type::Error)), emit_expr(&c.value))
            }
            _ => source.get(match item {
                Item::ReExport(r) => r.span.start..r.span.end,
                Item::OptionsDecl(o) => o.span.start..o.span.end,
                _ => 0..0,
            }).unwrap_or("").trim().to_string(),
        };

        out.push_str(&item_s);
        out.push('\n');

        // Use body.span.end (the } position) for functions, NOT f.span.end.
        // FunctionDecl.span.end is set to the NEXT token after }, causing prev_line
        // to jump past any inter-function leading comments.
        prev_end = match item {
            Item::Function(f) => f.body.span.end,
            Item::ShapeDecl(s) => s.span.end,
            Item::ImportDecl(i) => i.span.end,
            Item::ConstDecl(c) => c.span.end,
            Item::ReExport(r) => r.span.end,
            Item::OptionsDecl(o) => o.span.end,
        };
    }

    // Trailing newline
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: ynz-fmt-spike-prettier <file.ynz>");
        process::exit(2);
    }
    let path = &args[1];
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => { eprintln!("error reading {path}: {e}"); process::exit(2); }
    };
    print!("{}", format_source(&source));
}
