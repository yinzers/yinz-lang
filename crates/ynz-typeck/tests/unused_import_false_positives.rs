// WHY: Regression tests for the six false-positive unused-import sites. An import
// used ONLY via one of these AST positions was incorrectly flagged "imported but
// never used" because the corresponding check path never called
// `referenced_names.insert`. These tests fail before the fix and pass after,
// proving the insert was the missing link.
//
// Also covers the control case (genuinely-unused import still warns) and the
// adversarial dual-pattern case (two uses of one name don't mask a sibling unused import).

use ynz_diagnostics::DiagnosticKind;
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::check_query;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Create a unique temporary directory for this test run.
fn make_temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ynz_unused_import_{tag}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(dir.join("yinz.toml"), "[project]\nname = \"test\"\n").expect("write yinz.toml");
    dir
}

/// Build a two-file db: dep_name.ynz exports `dep_src`; entrypoint.ynz contains `main_src`.
/// Returns (db, entrypoint_sf, dir) — caller drops dir to clean up.
fn two_file_db(
    dep_name: &str,
    dep_src: &str,
    main_src: &str,
) -> (CompilerDb, SourceFile, std::path::PathBuf) {
    let dir = make_temp_dir(dep_name);

    let dep_path = dir.join(format!("{dep_name}.ynz"));
    let main_path = dir.join("entrypoint.ynz");

    std::fs::write(&dep_path, dep_src).expect("write dep");
    std::fs::write(&main_path, main_src).expect("write main");

    let mut db = CompilerDb::default();
    let sf_dep = SourceFile::new(&db, dep_path.display().to_string(), dep_src.to_string());
    let sf_main = SourceFile::new(&db, main_path.display().to_string(), main_src.to_string());
    db.register_source(sf_dep);
    db.register_source(sf_main);

    (db, sf_main, dir)
}

/// Assert that the given name does NOT appear in an UnusedImport warning.
fn assert_no_unused_import_warning(db: &CompilerDb, sf: SourceFile, name: &str) {
    let output = check_query(db, sf);
    let spurious: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(
                &d.kind,
                Some(DiagnosticKind::UnusedImport { name: n }) if n == name
            )
        })
        .collect();
    assert!(
        spurious.is_empty(),
        "Expected NO UnusedImport warning for `{name}`, but got one.\n\
         All diagnostics: {:#?}",
        output.diagnostics
    );
}

/// Assert that the given name DOES appear in an UnusedImport warning (control test).
fn assert_has_unused_import_warning(db: &CompilerDb, sf: SourceFile, name: &str) {
    let output = check_query(db, sf);
    let found: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(
                &d.kind,
                Some(DiagnosticKind::UnusedImport { name: n }) if n == name
            )
        })
        .collect();
    assert!(
        !found.is_empty(),
        "Expected an UnusedImport warning for `{name}`, but none was emitted.\n\
         All diagnostics: {:#?}",
        output.diagnostics
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 1 — options-variant access (`Timeframe.fiveMinute`)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn options_variant_access_does_not_warn_unused_import() {
    // WHY: `check_options_value` returned early without inserting into
    // `referenced_names`, so `Timeframe.fiveMinute` was counted as unused even
    // though it's the exact call site that references the type. This is the
    // user-reported repro that opened the bug class.
    let dep = r#"export options Timeframe { fiveMinute: `Five Minute`, daily: `Daily` }"#;
    let main = r#"
import { Timeframe } from `timeframes`
function entrypoint() -> nothing {
    print(Timeframe.fiveMinute.toString())
}
"#;
    let (db, sf, dir) = two_file_db("timeframes", dep, main);
    assert_no_unused_import_warning(&db, sf, "Timeframe");
    let _ = std::fs::remove_dir_all(dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 2.1 — `is`-narrowing (`if (s is Circle)`)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn is_narrowing_does_not_warn_unused_import() {
    // WHY: `check_is_expr` validated type_path.name against the union variants
    // but never inserted it into referenced_names. An import used exclusively
    // via `is TypeName` was false-positive flagged as unused.
    let dep = r#"
export shape Circle { radius: int }
export shape Square { side: int }
export shape Polygon = Circle | Square
"#;
    let main = r#"
import { Circle } from `shapes`
import { Polygon } from `shapes`
function area(s: Polygon) -> int {
    if (s is Circle) {
        return 3
    }
    return 0
}
function entrypoint() -> nothing { }
"#;
    let (db, sf, dir) = two_file_db("shapes", dep, main);
    assert_no_unused_import_warning(&db, sf, "Circle");
    let _ = std::fs::remove_dir_all(dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 2.2 — `extends` parent / `follows` contract
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn follows_contract_does_not_warn_unused_import() {
    // WHY: `check_module` skipped `Item::ShapeDecl` entirely, so an imported
    // contract used only in `shape X follows Contract` was never inserted into
    // referenced_names and was incorrectly flagged as unused.
    let dep = r#"
export shape Damageable {
    takeDamage(lend self, amount: int) -> nothing
}
"#;
    let main = r#"
import { Damageable } from `contracts`
shape Player follows Damageable {
    name: string
    health: int
}
function takeDamage(lend self: Player, amount: int) -> nothing {
    self.health = self.health - amount
}
function entrypoint() -> nothing { }
"#;
    let (db, sf, dir) = two_file_db("contracts", dep, main);
    assert_no_unused_import_warning(&db, sf, "Damageable");
    let _ = std::fs::remove_dir_all(dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 2.3 — shape field type annotation (`tf: Timeframe`)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn shape_field_type_does_not_warn_unused_import() {
    // WHY: field-type resolution runs in `shapes.rs` before the check pass and
    // has no access to `referenced_names`. `check_module` skipped ShapeDecl
    // entirely, so an import used only as a field type annotation was never
    // tracked and was false-positive flagged.
    let dep = r#"export options Status { active: `Active`, inactive: `Inactive` }"#;
    let main = r#"
import { Status } from `statuses`
shape Bar {
    status: Status
    value: int
}
function entrypoint() -> nothing { }
"#;
    let (db, sf, dir) = two_file_db("statuses", dep, main);
    assert_no_unused_import_warning(&db, sf, "Status");
    let _ = std::fs::remove_dir_all(dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 2.3 (continued) — module-level `const` with imported type
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn module_const_type_annotation_does_not_warn_unused_import() {
    // WHY: `Item::ConstDecl` was folded into a combined arm that did nothing,
    // so `export const DEFAULT: Status = Status.active` never registered `Status`
    // as referenced even though it appears in both the type annotation and the
    // initializer expression. Top-level consts must be exported in Yinz.
    let dep = r#"export options Status { active: `Active`, inactive: `Inactive` }"#;
    let main = r#"
import { Status } from `statuses`
export const DEFAULT: Status = Status.active
function entrypoint() -> nothing { }
"#;
    let (db, sf, dir) = two_file_db("statuses", dep, main);
    assert_no_unused_import_warning(&db, sf, "Status");
    let _ = std::fs::remove_dir_all(dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 2.4 — `dynamic Contract`
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn dynamic_contract_does_not_warn_unused_import() {
    // WHY: the `AstType::Dynamic` branch in `ast_type_to_type` returned
    // `Type::Dynamic { contract }` without inserting `contract` into
    // referenced_names. An import used only as `dynamic Foo` in a type
    // annotation was thus incorrectly flagged as unused.
    let dep = r#"
export shape Damageable {
    takeDamage(lend self, amount: int) -> nothing
}
"#;
    let main = r#"
import { Damageable } from `contracts`
function attackAll(targets: array<dynamic Damageable>) -> nothing {
    for (t in targets) {
        t.takeDamage(10)
    }
}
function entrypoint() -> nothing { }
"#;
    let (db, sf, dir) = two_file_db("contracts", dep, main);
    assert_no_unused_import_warning(&db, sf, "Damageable");
    let _ = std::fs::remove_dir_all(dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 2.5 — generic type position (`Container<int>`)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn concrete_shape_as_field_type_does_not_warn_unused_import() {
    // WHY: `check_module` previously skipped `Item::ShapeDecl` entirely, so an
    // import used exclusively as a field type annotation in another shape was never
    // walked by the check pass and thus not recorded in referenced_names. The fix
    // adds a ShapeDecl walk in check_module that calls ast_type_to_type on each
    // field type (non-generic shapes only — generic shapes have TypeParam
    // placeholders that are not in scope at module level).
    //
    // Cross-file generic shape imports (`shape Container<T>`) are not yet supported
    // by the import resolver (GenericShapeTable is file-local only). The Bug 2.5
    // insert (check.rs `generic_shape_table.contains` branch) is present for
    // future-proofing when that infrastructure ships.
    let dep = r#"export options Priority { high: `High`, low: `Low` }"#;
    let main = r#"
import { Priority } from `priorities`
shape Task {
    name: string
    priority: Priority
}
function entrypoint() -> nothing { }
"#;
    let (db, sf, dir) = two_file_db("priorities", dep, main);
    assert_no_unused_import_warning(&db, sf, "Priority");
    let _ = std::fs::remove_dir_all(dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Control test — genuinely-unused import STILL warns
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn genuinely_unused_import_still_warns() {
    // WHY: the six inserts above must not over-suppress. An import that is truly
    // never referenced anywhere must still produce an UnusedImport warning.
    // This test guards against accidentally blanket-suppressing the diagnostic.
    let dep = r#"export options Status { active: `Active`, inactive: `Inactive` }"#;
    let main = r#"
import { Status } from `statuses`
function entrypoint() -> nothing {
    print(`hello`)
}
"#;
    let (db, sf, dir) = two_file_db("statuses", dep, main);
    assert_has_unused_import_warning(&db, sf, "Status");
    let _ = std::fs::remove_dir_all(dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Step 7 adversarial — dual-used name does not mask a sibling unused import
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn dual_pattern_use_does_not_mask_sibling_unused_import() {
    // WHY: an import used via TWO distinct patterns (extends + field type) must
    // not cause a double-insert that corrupts the referenced_names set and
    // accidentally suppresses a warning for a genuinely-unused sibling import.
    // Concretely: `Entity` is used via `extends Entity` AND as a field type;
    // `UnusedThing` is imported but never referenced. After the fix, `Entity`
    // must NOT be flagged, and `UnusedThing` MUST still be flagged.
    let dep = r#"
export shape Entity { name: string health: int }
export options Rarity { common: `Common`, rare: `Rare` }
"#;
    let main = r#"
import { Entity } from `world`
import { Rarity } from `world`
shape Warrior extends Entity {
    weapon: string
    baseEntity: Entity
}
function entrypoint() -> nothing { }
"#;
    let (db, sf, dir) = two_file_db("world", dep, main);
    // Entity is used (extends + field type): no warning.
    assert_no_unused_import_warning(&db, sf, "Entity");
    // Rarity is never referenced: must still warn.
    assert_has_unused_import_warning(&db, sf, "Rarity");
    let _ = std::fs::remove_dir_all(dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Round-2 regression (i) — module-level const with imported type annotation and
// initializer: import tracked, no new diagnostic vs HEAD baseline.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn module_const_imported_type_tracked_no_new_diagnostic() {
    // WHY: the round-1 ConstDecl arm called infer_expr + ast_type_to_type which
    // are diagnostic-emitting paths. For a const with a valid type and initializer
    // that references an imported options type, this must (a) NOT flag the imported
    // type as unused, and (b) emit NO new diagnostic that was absent before the fix.
    // The diagnostic-free walkers (collect_referenced_names_in_*) satisfy both.
    let dep = r#"export options Priority { high: `High`, low: `Low` }"#;
    let main = r#"
import { Priority } from `priorities`
export const DEFAULT_PRIORITY: Priority = Priority.low
function entrypoint() -> nothing { }
"#;
    let (db, sf, dir) = two_file_db("priorities", dep, main);
    let output = ynz_typeck::check_query(&db, sf);
    // The imported type must not be flagged as unused.
    assert_no_unused_import_warning(&db, sf, "Priority");
    // No diagnostic of any kind must be newly introduced by the Phase 0 fix.
    let unexpected: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| {
            !matches!(
                &d.kind,
                Some(ynz_diagnostics::DiagnosticKind::UnusedImport { .. })
            )
        })
        .collect();
    assert!(
        unexpected.is_empty(),
        "Phase 0 const walk introduced unexpected diagnostics: {:#?}",
        unexpected
    );
    let _ = std::fs::remove_dir_all(dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Round-2 regression (ii) — concrete imported type used only as a field inside
// a generic shape is not flagged unused, and no spurious TypeParam diagnostic.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn generic_shape_concrete_field_import_tracked_no_type_param_diagnostic() {
    // WHY: the round-1 ShapeDecl arm guarded the field walk with
    // `if s.generics.is_empty()`, which skipped the ENTIRE field loop for generic
    // shapes. A concrete imported type used only as a field inside a generic shape
    // (e.g. `shape Box<T> { value: T, meta: ImportedMeta }`) was never tracked and
    // was incorrectly flagged as unused. The fix uses collect_referenced_names_in_ast_type
    // with a type_params set that skips bare `T` while still recording `ImportedMeta`.
    //
    // This test also guards that NO "T is not a known type" diagnostic appears,
    // proving the TypeParam-skip in the diagnostic-free walker works correctly.
    let dep = r#"export options Status { ok: `Ok`, err: `Err` }"#;
    let main = r#"
import { Status } from `statuses`
shape Wrapper<T> {
    value: T
    status: Status
}
function entrypoint() -> nothing { }
"#;
    let (db, sf, dir) = two_file_db("statuses", dep, main);
    let output = ynz_typeck::check_query(&db, sf);
    // The concrete imported type used as a field in the generic shape must not warn.
    assert_no_unused_import_warning(&db, sf, "Status");
    // No "T is not a known type" (NotDefined) diagnostic may appear — that would
    // mean the TypeParam-skip in the walker failed.
    let not_defined_diags: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| matches!(&d.kind, Some(ynz_diagnostics::DiagnosticKind::NotDefined)))
        .collect();
    assert!(
        not_defined_diags.is_empty(),
        "Unexpected NotDefined diagnostics (TypeParam-skip failed): {:#?}",
        not_defined_diags
    );
    let _ = std::fs::remove_dir_all(dir);
}

// ─────────────────────────────────────────────────────────────────────────────
// Round-2 regression (iii) — const with undefined type: no new diagnostic vs HEAD.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn module_const_undefined_type_emits_no_new_diagnostic_vs_head() {
    // WHY: the round-1 ConstDecl arm called ast_type_to_type which emits a
    // NotDefined diagnostic for `const X: NonexistentType = 5`. That turned on
    // type-checking for module-level consts that HEAD never checked — a verdict
    // change that violates the Safety invariant ("no change to type-checking
    // verdicts"). The diagnostic-free walker never calls ast_type_to_type, so
    // `const X: NonexistentType = 5` emits ZERO diagnostics from this path,
    // matching HEAD's behavior.
    //
    // This test has NO imports, so no UnusedImport warnings appear. It asserts
    // the total diagnostic count is 0, proving the const walk adds nothing.
    let main = r#"
export const MAGIC: UndefinedType = 42
function entrypoint() -> nothing { }
"#;
    // Single-file db: the dep file is a no-op placeholder.
    let (db, sf, dir) = two_file_db("placeholder", "function placeholder() -> nothing {}", main);
    let output = ynz_typeck::check_query(&db, sf);
    // Phase 0's diagnostic-free walker must not introduce any diagnostic for this
    // const — matching HEAD's zero-diagnostic behavior for module-level consts.
    assert!(
        output.diagnostics.is_empty(),
        "Phase 0 const walk emitted unexpected diagnostics for a const with undefined type \
         (verdict change detected — HEAD emits 0, this path emits >0): {:#?}",
        output.diagnostics
    );
    let _ = std::fs::remove_dir_all(dir);
}
