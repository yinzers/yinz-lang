// Which AST positions count as a use of an import: an import used only at one of them must
// not get the "imported but never used" warning, and a genuinely unused sibling must still get it.

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

/// Check `main_source` against a dependency file and assert, for each name, whether the
/// `UnusedImport` warning was (`flagged`) or was not (`not_flagged`) emitted.
fn assert_unused_import_warnings(
    dependency_name: &str,
    dependency_source: &str,
    main_source: &str,
    not_flagged: &[&str],
    flagged: &[&str],
) {
    let (db, sf, dir) = two_file_db(dependency_name, dependency_source, main_source);
    for name in not_flagged {
        assert_no_unused_import_warning(&db, sf, name);
    }
    for name in flagged {
        assert_has_unused_import_warning(&db, sf, name);
    }
    let _ = std::fs::remove_dir_all(dir);
}

macro_rules! unused_import_cases {
    ($($case:ident: $dependency_name:literal, $dependency:expr, $main:expr, not_flagged: $not_flagged:expr, flagged: $flagged:expr;)+) => {
        $(
            // bug: typeck warns "imported but never used" for an import used only in one AST position (the check path never recorded the name), or masks a genuinely unused sibling import | tier: Medium
            #[test]
            fn $case() {
                assert_unused_import_warnings($dependency_name, $dependency, $main, &$not_flagged, &$flagged);
            }
        )+
    };
}

// Medium cases: each is a different AST position where an import is referenced (the check path
// for that position is separate), plus the control that a sibling genuinely unused import still
// warns.
unused_import_cases! {
    an_options_variant_access_counts_as_a_use: "timeframes", r#"export options Timeframe { fiveMinute: `Five Minute`, daily: `Daily` }"#, r#"
import { Timeframe } from `timeframes`
function entrypoint() -> Nothing {
    print(Timeframe.fiveMinute.toString())
}
"#, not_flagged: ["Timeframe"], flagged: [];

    an_is_narrowing_counts_as_a_use: "shapes", r#"
export shape Circle { radius: Int }
export shape Square { side: Int }
export shape Polygon = Circle | Square
"#, r#"
import { Circle } from `shapes`
import { Polygon } from `shapes`
function area(s: Polygon) -> Int {
    if (s is Circle) {
        return 3
    }
    return 0
}
function entrypoint() -> Nothing { }
"#, not_flagged: ["Circle"], flagged: [];

    a_follows_contract_counts_as_a_use: "contracts", r#"
export shape Damageable {
    takeDamage(lend self, amount: Int) -> Nothing
}
"#, r#"
import { Damageable } from `contracts`
shape Player follows Damageable {
    name: String
    health: Int
}
function takeDamage(lend self: Player, amount: Int) -> Nothing {
    self.health = self.health - amount
}
function entrypoint() -> Nothing { }
"#, not_flagged: ["Damageable"], flagged: [];

    an_extends_parent_counts_as_a_use: "world", r#"
export shape Entity {
    name: String
    health: Int
}
"#, r#"
import { Entity } from `world`
shape Warrior extends Entity {
    weapon: String
}
function entrypoint() -> Nothing { }
"#, not_flagged: ["Entity"], flagged: [];

    a_shape_field_type_counts_as_a_use: "statuses", r#"export options Status { active: `Active`, inactive: `Inactive` }"#, r#"
import { Status } from `statuses`
shape Bar {
    status: Status
    value: Int
}
function entrypoint() -> Nothing { }
"#, not_flagged: ["Status"], flagged: [];

    a_module_const_type_and_initializer_count_as_a_use: "statuses", r#"export options Status { active: `Active`, inactive: `Inactive` }"#, r#"
import { Status } from `statuses`
export const DEFAULT: Status = Status.active
function entrypoint() -> Nothing { }
"#, not_flagged: ["Status"], flagged: [];

    a_dynamic_contract_counts_as_a_use: "contracts", r#"
export shape Damageable {
    takeDamage(lend self, amount: Int) -> Nothing
}
"#, r#"
import { Damageable } from `contracts`
function attackAll(targets: Array<dynamic Damageable>) -> Nothing {
    for (t in targets) {
        t.takeDamage(10)
    }
}
function entrypoint() -> Nothing { }
"#, not_flagged: ["Damageable"], flagged: [];

    a_concrete_field_of_a_generic_shape_counts_as_a_use: "statuses", r#"export options Status { ok: `Ok`, err: `Err` }"#, r#"
import { Status } from `statuses`
shape Wrapper<T> {
    value: T
    status: Status
}
function entrypoint() -> Nothing { }
"#, not_flagged: ["Status"], flagged: [];

    a_union_alias_member_shape_counts_as_a_use: "pgh_events", r#"
export shape SouthSideEvent { name: String }
export shape StripeDistrictEvent { name: String }
"#, r#"
import { SouthSideEvent } from `pgh_events`
import { StripeDistrictEvent } from `pgh_events`
shape PghEvent = SouthSideEvent | StripeDistrictEvent
function entrypoint() -> Nothing { }
"#, not_flagged: ["SouthSideEvent", "StripeDistrictEvent"], flagged: [];

    a_union_alias_member_options_counts_as_a_use: "results", r#"
export shape Scored { points: Int }
export options Grade { high: `High`, low: `Low` }
"#, r#"
import { Scored } from `results`
import { Grade } from `results`
shape Assessment = Scored | Grade
function entrypoint() -> Nothing { }
"#, not_flagged: ["Scored", "Grade"], flagged: [];

    a_name_used_twice_does_not_mask_a_sibling_unused_import: "world", r#"
export shape Entity { name: String health: Int }
export options Rarity { common: `Common`, rare: `Rare` }
"#, r#"
import { Entity } from `world`
import { Rarity } from `world`
shape Warrior extends Entity {
    weapon: String
    baseEntity: Entity
}
function entrypoint() -> Nothing { }
"#, not_flagged: ["Entity"], flagged: ["Rarity"];
}
