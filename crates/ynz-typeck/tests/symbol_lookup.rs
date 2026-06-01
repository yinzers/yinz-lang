// Symbol-lookup helper tests — go-to-def / find-refs / rename foundations.
//
// Tests load Yinz source inline or from the pirates-roster fixture project so
// the helpers work against realistic multi-file code.

use ynz_parser::{CompilerDb, SourceFile, SourceFileRegistry};
use ynz_typeck::{
    cross_file_reference_count_estimate, def_site_for_offset, references_for_offset,
    rename_locations, resolve_symbol_at, RenameError, SymbolKind,
};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build a single-file CompilerDb + SourceFile.
fn single_file(path: &str, src: &str) -> (CompilerDb, SourceFile) {
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, path.to_string(), src.to_string());
    db.register_source(sf);
    (db, sf)
}

/// Create a temp project dir with a `yinz.toml` and two files.
///
/// Files are WRITTEN to disk so `resolve_module_path`'s `candidate.exists()` check
/// passes and `std::fs::canonicalize` returns consistent paths.
///
/// Returns `(db, main_sf, _temp_dir)` — caller must keep `_temp_dir` alive until the
/// db is dropped (tempdir is cleaned up on drop).
fn two_files_on_disk(
    services_name: &str,
    services_src: &str,
    main_src: &str,
) -> (CompilerDb, SourceFile, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "ynz_symbol_lookup_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(dir.join("services")).expect("create dirs");
    std::fs::write(dir.join("yinz.toml"), "[project]\nname = \"test\"\n").unwrap();

    let dep_path = dir.join("services").join(format!("{services_name}.ynz"));
    std::fs::write(&dep_path, services_src).expect("write dep");

    let main_path = dir.join("entrypoint.ynz");
    std::fs::write(&main_path, main_src).expect("write main");

    let dep_path_str = dep_path.canonicalize().unwrap().display().to_string();
    let main_path_str = main_path.canonicalize().unwrap().display().to_string();

    let mut db = CompilerDb::default();
    let dep_sf = SourceFile::new(&db, dep_path_str, services_src.to_string());
    db.register_source(dep_sf);
    let main_sf = SourceFile::new(&db, main_path_str, main_src.to_string());
    db.register_source(main_sf);

    (db, main_sf, dir)
}

/// Find the byte offset of the first occurrence of `needle` in `src`.
#[allow(dead_code)]
fn offset_of(src: &str, needle: &str) -> usize {
    src.find(needle)
        .unwrap_or_else(|| panic!("needle {needle:?} not in source"))
}

// ─────────────────────────────────────────────────────────────────────────────
// resolve_symbol_at — same-file lookups
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_resolve_symbol_at_same_file_function_returns_function_kind() {
    let src = r#"
function greet(name: string) -> string {
  return name
}
function entrypoint() -> nothing {
  let msg = greet(`hello`)
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    // Offset on the `greet` call inside entrypoint.
    let call_offset = src.find("greet(`hello`)").unwrap();
    let sym = resolve_symbol_at(&db, sf, call_offset).expect("should resolve");
    assert_eq!(sym.kind, SymbolKind::Function);
    assert_eq!(sym.name, "greet");
}

#[test]
fn test_resolve_symbol_at_same_file_shape_returns_shape_kind() {
    let src = r#"
shape Player {
  name: string
  health: int
}
function entrypoint() -> nothing {
  let p: Player = { name: `Pat`, health: 100 }
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    // Offset on `Player` in the type annotation.
    let offset = src.rfind("Player").unwrap(); // the one in the let binding
    let sym = resolve_symbol_at(&db, sf, offset).expect("should resolve shape");
    assert_eq!(sym.kind, SymbolKind::Shape);
    assert_eq!(sym.name, "Player");
}

#[test]
fn test_resolve_symbol_at_local_let_returns_local_let_kind() {
    let src = r#"
function entrypoint() -> nothing {
  let count = 5
  let doubled = count + count
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    // Offset on second `count` (the use in `doubled = count + count`).
    let use_offset = src.rfind("count + count").unwrap();
    let sym = resolve_symbol_at(&db, sf, use_offset).expect("should resolve local");
    assert_eq!(sym.kind, SymbolKind::LocalLet);
    assert_eq!(sym.name, "count");
}

#[test]
fn test_resolve_symbol_at_local_const_returns_local_const_kind() {
    let src = r#"
function entrypoint() -> nothing {
  const limit = 10
  let x = limit
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    let offset = src.rfind("limit").unwrap(); // use in `let x = limit`
    let sym = resolve_symbol_at(&db, sf, offset).expect("should resolve const");
    assert_eq!(sym.kind, SymbolKind::LocalConst);
    assert_eq!(sym.name, "limit");
}

#[test]
fn test_resolve_symbol_at_parameter_returns_parameter_kind() {
    let src = r#"
function add(a: int, b: int) -> int {
  return a + b
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    // `a` in `return a + b`
    let offset = src.rfind("return a").unwrap() + "return ".len();
    let sym = resolve_symbol_at(&db, sf, offset).expect("should resolve param");
    assert_eq!(sym.kind, SymbolKind::Parameter);
    assert_eq!(sym.name, "a");
}

#[test]
fn test_resolve_symbol_at_options_type_returns_options_kind() {
    let src = r#"
options Status { active, inactive, banned }
function entrypoint() -> nothing {
  let s: Status = Status.active
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    let offset = src.rfind("Status").unwrap(); // last `Status` in `Status.active`
    let sym = resolve_symbol_at(&db, sf, offset).expect("should resolve options");
    assert_eq!(sym.kind, SymbolKind::Options);
    assert_eq!(sym.name, "Status");
}

// ─────────────────────────────────────────────────────────────────────────────
// resolve_symbol_at — SymbolLookupError variants
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_resolve_symbol_at_out_of_bounds_returns_none() {
    let src = "function entrypoint() -> nothing {}";
    let (db, sf) = single_file("test.ynz", src);
    // Way past end of file.
    assert!(resolve_symbol_at(&db, sf, src.len() + 100).is_none());
}

#[test]
fn test_resolve_symbol_at_whitespace_offset_returns_none() {
    let src = "function entrypoint() -> nothing {  }";
    let (db, sf) = single_file("test.ynz", src);
    // The spaces inside the braces.
    let offset = src.find("{  }").unwrap() + 1;
    assert!(resolve_symbol_at(&db, sf, offset).is_none());
}

#[test]
fn test_resolve_symbol_at_integer_literal_returns_none() {
    let src = r#"
function entrypoint() -> nothing {
  let x = 42
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    let offset = src.find("42").unwrap();
    assert!(resolve_symbol_at(&db, sf, offset).is_none());
}

// ─────────────────────────────────────────────────────────────────────────────
// def_site_for_offset
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_def_site_for_offset_same_file_function() {
    let src = r#"
function greet() -> string { return `hi` }
function entrypoint() -> nothing {
  let msg = greet()
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    let call_offset = src.rfind("greet()").unwrap();
    let (_, span) = def_site_for_offset(&db, sf, call_offset).expect("must find def");
    // Span should be inside the declaration, which is near the top of the file.
    assert!(span.start < 20, "decl span should be near top");
}

#[test]
fn test_def_site_for_offset_whitespace_returns_none() {
    let src = "function entrypoint() -> nothing { }";
    let (db, sf) = single_file("test.ynz", src);
    assert!(def_site_for_offset(&db, sf, 0).is_none()); // 'f' in 'function' is a keyword, not resolvable
}

// ─────────────────────────────────────────────────────────────────────────────
// cross-file lookups
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_resolve_symbol_at_cross_file_imported_function() {
    let dep_src = "export function newPirate(name: string) -> string {\n  return name\n}\n";
    let main_src = "import { newPirate } from `services/players`\nfunction entrypoint() -> nothing {\n  let p = newPirate(`Roberto`)\n}\n";
    let (db, main_sf, _dir) = two_files_on_disk("players", dep_src, main_src);

    let call_offset = main_src.rfind("newPirate(`Roberto`)").unwrap();
    let sym = resolve_symbol_at(&db, main_sf, call_offset).expect("should resolve cross-file fn");
    assert_eq!(sym.kind, SymbolKind::Function);
    assert_eq!(sym.name, "newPirate");
    assert!(
        sym.origin_file != main_sf,
        "origin should be dep file, not main file"
    );
}

#[test]
fn test_resolve_symbol_at_cross_file_imported_shape() {
    let dep_src = "export shape Pirate {\n  name: string\n  careerHits: int\n}\n";
    let main_src = "import { Pirate } from `services/players`\nfunction entrypoint() -> nothing {\n  let p: Pirate = { name: `Roberto`, careerHits: 3000 }\n}\n";
    let (db, main_sf, _dir) = two_files_on_disk("players", dep_src, main_src);

    let type_offset = main_src.rfind(": Pirate").unwrap() + 2;
    let sym = resolve_symbol_at(&db, main_sf, type_offset).expect("should resolve shape");
    assert_eq!(sym.kind, SymbolKind::Shape);
    assert!(
        sym.origin_file != main_sf,
        "origin should be dep file, not main file"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// references_for_offset
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_references_for_offset_same_file_include_decl_false_excludes_decl() {
    let src = r#"
function greet() -> string { return `hi` }
function entrypoint() -> nothing {
  let a = greet()
  let b = greet()
  let c = greet()
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    let decl_offset = src.find("function greet").unwrap() + "function ".len();
    let refs = references_for_offset(&db, sf, decl_offset, false);
    // 3 calls, not the declaration.
    assert_eq!(refs.len(), 3, "expected 3 call sites, got {}", refs.len());
}

#[test]
fn test_references_for_offset_include_decl_true_includes_decl() {
    let src = r#"
function greet() -> string { return `hi` }
function entrypoint() -> nothing {
  let a = greet()
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    let decl_offset = src.find("function greet").unwrap() + "function ".len();
    let refs = references_for_offset(&db, sf, decl_offset, true);
    assert!(
        refs.len() >= 2,
        "expected at least decl + 1 use, got {}",
        refs.len()
    );
}

#[test]
fn test_references_for_offset_whitespace_returns_empty() {
    let src = "function entrypoint() -> nothing {   }";
    let (db, sf) = single_file("test.ynz", src);
    let refs = references_for_offset(&db, sf, src.find("{   }").unwrap() + 2, false);
    assert!(refs.is_empty());
}

#[test]
fn test_references_for_offset_symbol_used_in_multiple_files() {
    let dep_src = "export function announce(name: string) -> string {\n  return name\n}\n";
    let main_src = "import { announce } from `services/players`\nfunction entrypoint() -> nothing {\n  let a = announce(`Roberto`)\n  let b = announce(`Willie`)\n}\n";
    let (db, main_sf, _dir) = two_files_on_disk("players", dep_src, main_src);
    let call_offset = main_src.find("announce(`Roberto`)").unwrap();
    let refs = references_for_offset(&db, main_sf, call_offset, false);
    // Both calls in main file.
    assert!(
        refs.len() >= 2,
        "expected at least 2 refs, got {}",
        refs.len()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Shadowing — adversarial per plan-reviewer
// ─────────────────────────────────────────────────────────────────────────────

// WHY: protects against silent-corruption rename — without per-site resolution,
// `references_for_offset` on the outer `foo` returns the inner shadow's use-site
// too (name-match), causing rename to corrupt the inner binding. The test asserts
// EXACT counts and SPAN DISJOINTNESS so a naive name-match implementation fails.
#[test]
fn test_references_shadowing_inner_binding_does_not_contaminate_outer() {
    // outer `let foo = 1` — used in `let bar = foo + 1`
    // inner `let foo = 2` (shadow) — used in `let x = foo` inside the if-block
    let src = "function f() -> nothing {\n  let foo = 1\n  let bar = foo + 1\n  if (true) {\n    let foo = 2\n    let x = foo\n  }\n}\n";
    let (db, sf) = single_file("test.ynz", src);

    let outer_offset = src.find("let foo = 1").unwrap() + "let ".len();
    let inner_offset = src.find("let foo = 2").unwrap() + "let ".len();

    let outer_refs = references_for_offset(&db, sf, outer_offset, false);
    let inner_refs = references_for_offset(&db, sf, inner_offset, false);

    // Outer `foo` → only the use in `bar = foo + 1` (1 site; not the inner shadow's use).
    assert_eq!(
        outer_refs.len(),
        1,
        "outer foo: expected 1 use-site, got {}",
        outer_refs.len()
    );
    // Inner `foo` → only the use in `let x = foo` (1 site; not the outer binding's use).
    assert_eq!(
        inner_refs.len(),
        1,
        "inner foo: expected 1 use-site, got {}",
        inner_refs.len()
    );
    // Spans must be disjoint (they refer to different source positions).
    let outer_span = &outer_refs[0].1;
    let inner_span = &inner_refs[0].1;
    assert!(
        outer_span.end <= inner_span.start || inner_span.end <= outer_span.start,
        "outer and inner use-sites must be disjoint: outer={outer_span:?} inner={inner_span:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// rename_locations
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_rename_locations_local_let_returns_ok_with_locations() {
    let src = r#"
function entrypoint() -> nothing {
  let count = 5
  let doubled = count + count
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    let decl_offset = src.find("let count").unwrap() + "let ".len();
    let locs =
        rename_locations(&db, sf, decl_offset, "total".to_string()).expect("rename should succeed");
    assert!(!locs.is_empty(), "must return at least one location");
}

#[test]
fn test_rename_locations_rejects_reserved_keyword_let() {
    let src = r#"
function entrypoint() -> nothing {
  let count = 5
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    let offset = src.find("let count").unwrap() + "let ".len();
    let err = rename_locations(&db, sf, offset, "let".to_string())
        .err()
        .expect("renaming to keyword must fail");
    assert!(
        matches!(err, RenameError::NewNameIsReservedKeyword(ref k) if k == "let"),
        "expected NewNameIsReservedKeyword(let), got {err:?}"
    );
}

#[test]
fn test_rename_locations_rejects_banned_jargon_class() {
    let src = r#"
function entrypoint() -> nothing {
  let count = 5
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    let offset = src.find("let count").unwrap() + "let ".len();
    let err = rename_locations(&db, sf, offset, "class".to_string())
        .err()
        .expect("renaming to banned jargon must fail");
    assert!(
        matches!(err, RenameError::NewNameIsBannedJargon(..)),
        "expected NewNameIsBannedJargon, got {err:?}"
    );
}

#[test]
fn test_rename_locations_rejects_invalid_identifier_starts_with_digit() {
    let src = r#"
function entrypoint() -> nothing {
  let count = 5
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    let offset = src.find("let count").unwrap() + "let ".len();
    let err = rename_locations(&db, sf, offset, "123foo".to_string())
        .err()
        .expect("invalid identifier must fail");
    assert!(
        matches!(err, RenameError::NewNameInvalidIdentifier(..)),
        "expected NewNameInvalidIdentifier, got {err:?}"
    );
}

#[test]
fn test_rename_locations_rejects_empty_name() {
    let src = r#"
function entrypoint() -> nothing {
  let count = 5
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    let offset = src.find("let count").unwrap() + "let ".len();
    let err = rename_locations(&db, sf, offset, "".to_string())
        .err()
        .expect("empty name must fail");
    assert!(matches!(err, RenameError::NewNameInvalidIdentifier(..)));
}

#[test]
fn test_rename_locations_rejects_imported_symbol_at_use_site() {
    let dep_src = "export function greet() -> string { return `hi` }\n";
    let main_src = "import { greet } from `services/players`\nfunction entrypoint() -> nothing {\n  let msg = greet()\n}\n";
    let (db, main_sf, _dir) = two_files_on_disk("players", dep_src, main_src);

    let call_offset = main_src.find("greet()").unwrap();
    let err = rename_locations(&db, main_sf, call_offset, "sayHello".to_string())
        .err()
        .expect("imported symbol rename must fail");
    assert!(
        matches!(err, RenameError::CannotRenameImportedSymbolInThisFile(..)),
        "expected CannotRenameImportedSymbolInThisFile, got {err:?}"
    );
}

#[test]
fn test_rename_locations_at_non_symbol_returns_not_a_renameable() {
    let src = r#"function entrypoint() -> nothing { }"#;
    let (db, sf) = single_file("test.ynz", src);
    // Offset 0 = 'f' in the `function` keyword — not a resolvable identifier.
    let err = rename_locations(&db, sf, 0, "foo".to_string())
        .err()
        .expect("non-symbol must fail");
    assert!(
        matches!(err, RenameError::NotARenameable),
        "expected NotARenameable, got {err:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// cross_file_reference_count_estimate — Phase-1-owned salsa query
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_cross_file_reference_count_estimate_local_returns_one() {
    let src = r#"
function entrypoint() -> nothing {
  let x = 5
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    let offset = src.find("let x").unwrap() + "let ".len();
    // Local binding — only one file can reference it.
    let estimate = cross_file_reference_count_estimate(&db, sf, offset);
    assert_eq!(estimate, 1);
}

#[test]
fn test_cross_file_reference_count_estimate_exported_symbol_counts_importers() {
    let dep_src = "export function greet() -> string { return `hi` }\n";
    let main_src = "import { greet } from `services/players`\nfunction entrypoint() -> nothing {\n  let msg = greet()\n}\n";
    let (db, _main_sf, _dir) = two_files_on_disk("players", dep_src, main_src);

    // Get the dep SourceFile to measure from the declaration.
    let all_paths = db.all_source_paths();
    let dep_sf = all_paths
        .iter()
        .filter_map(|p| db.source_by_path(p))
        .find(|sf| sf.text(&db).starts_with("export function greet"))
        .expect("dep file must be in db");

    let decl_offset = dep_src.find("function greet").unwrap() + "function ".len();
    let estimate = cross_file_reference_count_estimate(&db, dep_sf, decl_offset);
    // At minimum: the dep file itself + the main file (which imports greet).
    assert!(estimate >= 2, "expected >= 2, got {estimate}");
}

#[test]
fn test_cross_file_reference_count_estimate_completes_fast() {
    // Estimate must return within 5ms (ExportTable read only — no AST walk).
    let dep_src = "export function greet() -> string { return `hi` }\n";
    let main_src = "import { greet } from `services/players`\nfunction entrypoint() -> nothing { let msg = greet() }\n";
    let (db, _main_sf, _dir) = two_files_on_disk("players", dep_src, main_src);

    let all_paths = db.all_source_paths();
    let dep_sf = all_paths
        .iter()
        .filter_map(|p| db.source_by_path(p))
        .find(|sf| sf.text(&db).starts_with("export function greet"))
        .expect("dep file must be in db");

    let start = std::time::Instant::now();
    let offset = dep_src.find("function greet").unwrap() + "function ".len();
    let _ = cross_file_reference_count_estimate(&db, dep_sf, offset);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 5,
        "estimate took {}ms, expected < 5ms",
        elapsed.as_millis()
    );
}
