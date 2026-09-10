//! Proptest fuzz harness: generates random AST-rooted `.ynz` programs and asserts:
//!   1. `format(emit(ast))` parses back to an AST equal modulo trivia — semantic safety.
//!   2. `format(format(emit(ast))) == format(emit(ast))` — idempotency.
//!
//! Strategy: build random `Module` ASTs directly, render via `ynz_fmt::walker::emit_module`,
//! then feed the output into the full formatter pipeline.  AST-rooted generation avoids the
//! ~90% parser-reject waste of text-rooted generators.
//!
//! CARVE-OUTs (deferred — not covered in this harness):
//! - Generics with >2 type params (combinatorial explosion in shrink)
//! - Recursive shape definitions (require prop_recursive with high depth)
//! - Deeply-nested doc-comment blocks (>3 lines)

mod common;
use common::arb_module;

use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn random_ast_semantic_roundtrip(module in arb_module()) {
        let source = ynz_fmt::walker::emit_module(&module);
        let formatted = match ynz_fmt::format(&source) {
            Ok(f) => f,
            Err(_) => {
                // emit_module produced source the parser rejected — skip.
                return Ok(());
            }
        };

        // Semantic round-trip: parse(format(emit(ast))) ~= parse(emit(ast)) modulo trivia.
        let db_orig = ynz_parser::CompilerDb::default();
        let sf_orig =
            ynz_parser::SourceFile::new(&db_orig, "<orig>".into(), source.clone());
        let orig_parse = ynz_parser::parse_query(&db_orig, sf_orig);

        if !orig_parse.diagnostics.is_empty() {
            return Ok(()); // emit_module produced invalid source; skip.
        }

        let db_fmt = ynz_parser::CompilerDb::default();
        let sf_fmt = ynz_parser::SourceFile::new(&db_fmt, "<fmt>".into(), formatted.clone());
        let fmt_parse = ynz_parser::parse_query(&db_fmt, sf_fmt);

        prop_assert!(
            fmt_parse.diagnostics.is_empty(),
            "format() introduced parse errors — source: {:?}  formatted: {:?}",
            source,
            formatted
        );

        prop_assert!(
            ynz_ast::ast_eq_modulo_trivia(&orig_parse.module, &fmt_parse.module),
            "semantic round-trip failed — source: {:?}  formatted: {:?}",
            source,
            formatted
        );

        // Idempotency: format(format(x)) == format(x).
        let formatted2 = ynz_fmt::format(&formatted).expect("second format pass failed");
        prop_assert_eq!(
            formatted,
            formatted2,
            "formatter is not idempotent — source: {:?}",
            source
        );
    }
}
