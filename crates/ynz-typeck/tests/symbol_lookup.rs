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

/// What `resolve_symbol_at` must return at an offset: the symbol's kind and name, or nothing.
type ExpectedSymbol = Option<(SymbolKind, &'static str)>;

fn assert_resolves(source: &str, offset: impl Fn(&str) -> usize, expected: ExpectedSymbol) {
    let (db, sf) = single_file("test.ynz", source);
    let found = resolve_symbol_at(&db, sf, offset(source));
    let found = found
        .as_ref()
        .map(|symbol| (symbol.kind.clone(), symbol.name.as_str()));
    assert_eq!(found, expected, "{source}");
}

macro_rules! resolve_cases {
    ($($case:ident: $source:expr, $offset:expr => $expected:expr;)+) => {
        $(
            // bug: go-to-def resolves an offset to the wrong symbol kind or name, or resolves one that is no identifier (whitespace, a literal, past the end of the file) | tier: Medium
            #[test]
            fn $case() {
                assert_resolves($source, $offset, $expected);
            }
        )+
    };
}

// Medium cases: one per declaration kind the resolver must tell apart (function, shape, local
// let, local const, parameter, options) and one per non-identifier offset (past the end,
// whitespace, a literal): each is a different lookup path, so none duplicates another.
resolve_cases! {
    resolving_a_same_file_function_call_gives_the_function_kind:
        "\nfunction greet(name: String) -> String {\n  return name\n}\nfunction entrypoint() -> Nothing {\n  let msg = greet(`hello`)\n}\n",
        |src| src.find("greet(`hello`)").unwrap()
        => Some((SymbolKind::Function, "greet"));
    resolving_a_same_file_shape_annotation_gives_the_shape_kind:
        "\nshape Player {\n  name: String\n  health: Int\n}\nfunction entrypoint() -> Nothing {\n  let p: Player = { name: `Pat`, health: 100 }\n}\n",
        |src| src.rfind("Player").unwrap()
        => Some((SymbolKind::Shape, "Player"));
    resolving_a_local_let_use_gives_the_local_let_kind:
        "\nfunction entrypoint() -> Nothing {\n  let count = 5\n  let doubled = count + count\n}\n",
        |src| src.rfind("count + count").unwrap()
        => Some((SymbolKind::LocalLet, "count"));
    resolving_a_local_const_use_gives_the_local_const_kind:
        "\nfunction entrypoint() -> Nothing {\n  const limit = 10\n  let x = limit\n}\n",
        |src| src.rfind("limit").unwrap()
        => Some((SymbolKind::LocalConst, "limit"));
    resolving_a_parameter_use_gives_the_parameter_kind:
        "\nfunction add(a: Int, b: Int) -> Int {\n  return a + b\n}\n",
        |src| src.rfind("return a").unwrap() + "return ".len()
        => Some((SymbolKind::Parameter, "a"));
    resolving_an_options_type_gives_the_options_kind:
        "\noptions Status { active, inactive, banned }\nfunction entrypoint() -> Nothing {\n  let s: Status = Status.active\n}\n",
        |src| src.rfind("Status").unwrap()
        => Some((SymbolKind::Options, "Status"));
    an_offset_past_the_end_of_the_file_resolves_to_nothing:
        "function entrypoint() -> Nothing {}",
        |src| src.len() + 100
        => None;
    a_whitespace_offset_resolves_to_nothing:
        "function entrypoint() -> Nothing {  }",
        |src| src.find("{  }").unwrap() + 1
        => None;
    an_integer_literal_resolves_to_nothing:
        "\nfunction entrypoint() -> Nothing {\n  let x = 42\n}\n",
        |src| src.find("42").unwrap()
        => None;
}

// bug: go-to-def on an imported function resolves to the importing file instead of the file that declares it | tier: Medium
#[test]
fn resolving_an_imported_function_reaches_its_origin_file() {
    let dep_src = "export function newPirate(name: String) -> String {\n  return name\n}\n";
    let main_src = "import { newPirate } from `services/players`\nfunction entrypoint() -> Nothing {\n  let p = newPirate(`Roberto`)\n}\n";
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

// bug: find-definition on a call finds no declaration, or one far from where the function is declared | tier: Medium
#[test]
fn a_call_has_its_declaration_as_definition_site() {
    let src = r#"
function greet() -> String { return `hi` }
function entrypoint() -> Nothing {
  let msg = greet()
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    let call_offset = src.rfind("greet()").unwrap();
    let (_, span) = def_site_for_offset(&db, sf, call_offset).expect("must find def");
    // Span should be inside the declaration, which is near the top of the file.
    assert!(span.start < 20, "decl span should be near top");
}

// bug: find-definition returns a site for a keyword, which is no resolvable symbol | tier: Medium
#[test]
fn a_keyword_has_no_definition_site() {
    let src = "function entrypoint() -> Nothing { }";
    let (db, sf) = single_file("test.ynz", src);
    assert!(def_site_for_offset(&db, sf, 0).is_none()); // 'f' in 'function' is a keyword, not resolvable
}

/// How many references `references_for_offset` must return.
enum ExpectedReferences {
    Exactly(usize),
    AtLeast(usize),
}

fn assert_references(
    source: &str,
    offset: impl Fn(&str) -> usize,
    include_declaration: bool,
    expected: ExpectedReferences,
) {
    let (db, sf) = single_file("test.ynz", source);
    let count = references_for_offset(&db, sf, offset(source), include_declaration).len();
    match expected {
        ExpectedReferences::Exactly(exact) => assert_eq!(count, exact, "{source}"),
        ExpectedReferences::AtLeast(least) => {
            assert!(count >= least, "expected at least {least}, got {count}")
        }
    }
}

macro_rules! references_cases {
    ($($case:ident: $source:expr, $offset:expr, $include_declaration:expr => $expected:expr;)+) => {
        $(
            // bug: find-references returns the wrong set (the declaration when excluded, none when included, any at a whitespace offset) | tier: Medium
            #[test]
            fn $case() {
                assert_references($source, $offset, $include_declaration, $expected);
            }
        )+
    };
}

const THREE_CALLS: &str = r#"
function greet() -> String { return `hi` }
function entrypoint() -> Nothing {
  let a = greet()
  let b = greet()
  let c = greet()
}
"#;

// Medium cases: the include-declaration switch both ways (the boundary of the one flag) and the
// no-symbol offset.
references_cases! {
    references_without_the_declaration_are_the_three_calls:
        THREE_CALLS, |src| src.find("function greet").unwrap() + "function ".len(), false
        => ExpectedReferences::Exactly(3);
    references_with_the_declaration_include_it:
        THREE_CALLS, |src| src.find("function greet").unwrap() + "function ".len(), true
        => ExpectedReferences::AtLeast(4);
    references_at_a_whitespace_offset_are_empty:
        "function entrypoint() -> Nothing {   }", |src| src.find("{   }").unwrap() + 2, false
        => ExpectedReferences::Exactly(0);
}

// bug: find-references on an imported function misses the calls in the importing file | tier: Medium
#[test]
fn references_of_an_imported_function_are_found_in_the_importing_file() {
    let dep_src = "export function announce(name: String) -> String {\n  return name\n}\n";
    let main_src = "import { announce } from `services/players`\nfunction entrypoint() -> Nothing {\n  let a = announce(`Roberto`)\n  let b = announce(`Willie`)\n}\n";
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

// bug: find-references on an outer binding also returns the use of a shadowing inner binding of the same name, so a rename corrupts the inner binding | tier: Medium
#[test]
fn references_of_a_shadowed_binding_do_not_contaminate_each_other() {
    // outer `let foo = 1` — used in `let bar = foo + 1`
    // inner `let foo = 2` (shadow) — used in `let x = foo` inside the if-block
    let src = "function f() -> Nothing {\n  let foo = 1\n  let bar = foo + 1\n  if (true) {\n    let foo = 2\n    let x = foo\n  }\n}\n";
    let (db, sf) = single_file("test.ynz", src);

    let outer_offset = src.find("let foo = 1").unwrap() + "let ".len();
    let inner_offset = src.find("let foo = 2").unwrap() + "let ".len();

    let outer_refs = references_for_offset(&db, sf, outer_offset, false);
    let inner_refs = references_for_offset(&db, sf, inner_offset, false);

    // Each binding has exactly its own one use site, and the two sites are disjoint.
    assert_eq!(outer_refs.len(), 1, "outer foo: {}", outer_refs.len());
    assert_eq!(inner_refs.len(), 1, "inner foo: {}", inner_refs.len());
    let outer_span = &outer_refs[0].1;
    let inner_span = &inner_refs[0].1;
    assert!(
        outer_span.end <= inner_span.start || inner_span.end <= outer_span.start,
        "outer and inner use-sites must be disjoint: outer={outer_span:?} inner={inner_span:?}"
    );
}

/// What `rename_locations` must answer for a rename of `count` in the program below.
enum ExpectedRename {
    Locations,
    ReservedKeyword,
    BannedJargon,
    InvalidIdentifier,
    NotARenameable,
}

const RENAME_PROGRAM: &str = r#"
function entrypoint() -> Nothing {
  let count = 5
  let doubled = count + count
}
"#;

fn assert_rename(offset: impl Fn(&str) -> usize, new_name: &str, expected: ExpectedRename) {
    let (db, sf) = single_file("test.ynz", RENAME_PROGRAM);
    let result = rename_locations(&db, sf, offset(RENAME_PROGRAM), new_name.to_string());
    let result = result.map(|locations| locations.len());
    match (expected, result) {
        (ExpectedRename::Locations, Ok(location_count)) => {
            assert!(location_count > 0, "must return at least one location")
        }
        (ExpectedRename::ReservedKeyword, Err(RenameError::NewNameIsReservedKeyword(keyword))) => {
            assert_eq!(keyword, new_name)
        }
        (ExpectedRename::BannedJargon, Err(RenameError::NewNameIsBannedJargon(..)))
        | (ExpectedRename::InvalidIdentifier, Err(RenameError::NewNameInvalidIdentifier(..)))
        | (ExpectedRename::NotARenameable, Err(RenameError::NotARenameable)) => {}
        (_, other) => panic!("unexpected rename result for {new_name:?}: {other:?}"),
    }
}

macro_rules! rename_cases {
    ($($case:ident: $offset:expr, $new_name:literal => $expected:expr;)+) => {
        $(
            // bug: rename accepts a reserved keyword, banned jargon or an invalid identifier as the new name (or refuses a valid one, or renames at an offset that is no symbol) | tier: Medium
            #[test]
            fn $case() {
                assert_rename($offset, $new_name, $expected);
            }
        )+
    };
}

// Medium cases: the valid rename plus one per refusal the caller handles differently.
rename_cases! {
    renaming_a_local_let_returns_its_locations:
        |src| src.find("let count").unwrap() + "let ".len(), "total" => ExpectedRename::Locations;
    renaming_to_a_reserved_keyword_is_refused:
        |src| src.find("let count").unwrap() + "let ".len(), "let" => ExpectedRename::ReservedKeyword;
    renaming_to_banned_jargon_is_refused:
        |src| src.find("let count").unwrap() + "let ".len(), "class" => ExpectedRename::BannedJargon;
    renaming_to_a_name_starting_with_a_digit_is_refused:
        |src| src.find("let count").unwrap() + "let ".len(), "123foo" => ExpectedRename::InvalidIdentifier;
    renaming_to_an_empty_name_is_refused:
        |src| src.find("let count").unwrap() + "let ".len(), "" => ExpectedRename::InvalidIdentifier;
    renaming_at_a_keyword_offset_is_not_a_renameable:
        |_src| 0, "foo" => ExpectedRename::NotARenameable;
}

// bug: rename accepts an imported symbol at its use site, rewriting only this file and leaving the declaring file out of step | tier: Medium
#[test]
fn renaming_an_imported_symbol_at_its_use_site_is_refused() {
    let dep_src = "export function greet() -> String { return `hi` }\n";
    let main_src = "import { greet } from `services/players`\nfunction entrypoint() -> Nothing {\n  let msg = greet()\n}\n";
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

// bug: the cross-file reference estimate for a local binding is not 1 (a local can only be referenced from its own file) | tier: Medium
#[test]
fn a_local_binding_has_a_cross_file_estimate_of_one() {
    let src = r#"
function entrypoint() -> Nothing {
  let x = 5
}
"#;
    let (db, sf) = single_file("test.ynz", src);
    let offset = src.find("let x").unwrap() + "let ".len();
    assert_eq!(cross_file_reference_count_estimate(&db, sf, offset), 1);
}

// bug: the cross-file reference estimate of an exported symbol leaves out the files that import it | tier: Medium
#[test]
fn an_exported_symbol_has_a_cross_file_estimate_counting_its_importers() {
    let dep_src = "export function greet() -> String { return `hi` }\n";
    let main_src = "import { greet } from `services/players`\nfunction entrypoint() -> Nothing {\n  let msg = greet()\n}\n";
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
