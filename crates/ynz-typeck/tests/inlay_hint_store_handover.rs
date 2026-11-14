// WHY: v0.3-M9 C5a (Tier: Medium, presentation of a High-tier decision): storing a named heap
// value hands it over, and the muted hint at the store says where the value went
// (`a` + ` handed to `list``), so the later "was stored in" error is no surprise. The hint reads
// the one store record (`TypedModule::stores_handed_over`), so one test per store form pins
// that every form records, and one pins that a store of bits (a copy) records nothing.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::ownership_call_site_hints;

/// The hints `source` gets, as (text before the hint's position, hint text) pairs.
fn store_hints(discriminator: &str, source: &str) -> Vec<(String, String)> {
    let path = format!("/tmp/ynz_store_handover_{discriminator}.ynz");
    let mut db = CompilerDb::default();
    let file = SourceFile::new(&db, path, source.to_string());
    db.register_source(file);
    ownership_call_site_hints(&db, file)
        .into_iter()
        .filter(|hint| hint.modifier.starts_with("handed to"))
        .map(|hint| {
            let before = source[..hint.position].lines().last().unwrap_or("").trim();
            (before.to_string(), hint.modifier)
        })
        .collect()
}

fn assert_one_hint(discriminator: &str, source: &str, line_up_to_value: &str, hint: &str) {
    let found = store_hints(discriminator, source);
    assert_eq!(
        found,
        vec![(line_up_to_value.to_string(), hint.to_string())],
        "{source}"
    );
}

#[test]
fn container_add_hints_where_the_value_went() {
    assert_one_hint(
        "add",
        "function entrypoint() -> nothing {
  let a: array<int> = [1]
  let list: array<array<int>> = []
  list.add(a)
}",
        "list.add(a",
        "handed to `list`",
    );
}

#[test]
fn field_store_hints_where_the_value_went() {
    assert_one_hint(
        "field",
        "shape Bag { items: array<int> }
function entrypoint() -> nothing {
  let a: array<int> = [1]
  let bag: Bag = { items: [0] }
  bag.items = a
}",
        "bag.items = a",
        "handed to `bag.items`",
    );
}

#[test]
fn let_hints_where_the_value_went() {
    assert_one_hint(
        "let",
        "function entrypoint() -> nothing {
  let a: array<int> = [1]
  let b: array<int> = a
  print(b.count().toString())
}",
        "let b: array<int> = a",
        "handed to `b`",
    );
}

#[test]
fn literal_part_hints_where_the_value_went() {
    assert_one_hint(
        "literal",
        "function entrypoint() -> nothing {
  let a: array<int> = [1]
  let rows: array<array<int>> = [a]
  print(rows.count().toString())
}",
        "let rows: array<array<int>> = [a",
        "handed to the literal `[a]`",
    );
}

#[test]
fn storing_bits_hints_nothing() {
    let found = store_hints(
        "bits",
        "function entrypoint() -> nothing {
  let n: int = 1
  let list: array<int> = []
  list.add(n)
  let m: int = n
  print(m.toString())
}",
    );
    assert_eq!(found, Vec::<(String, String)>::new());
}
