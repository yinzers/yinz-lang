// WHY: tests for `ownership_call_site_hints` (the muted ownership hints at call sites and at
// stores). One unit; each case is one call syntax or store form that must reach the hint:
//
// - Bug 2.11: `player.heal(20)` (UFCS / MethodCall) and `heal(player, 20)` (free function / Call)
//   are the same call and must hint the same ownership kind; generic functions must hint too (the
//   signature lookup used to stop before `generic_fn_table`).
// - v0.3-M3b Phase 2: `background f(x)` hints the inferred `give` (x unused after) or `copy`.
// - v0.3-M9 C5a: storing a named heap value hands it over, and the muted hint at the store says
//   where it went (`handed to ..`); it reads the one store record
//   (`TypedModule::stores_handed_over`), and storing bits (a copy) records nothing.
//
// Hint wording beyond the ownership modifier and hint position are not asserted (Low tier); a
// position is used only to say at which store site a hint sits.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::ownership_call_site_hints;

fn single_file(discriminator: &str, src: &str) -> (CompilerDb, SourceFile) {
    let path = format!("/tmp/ynz_ownership_ufcs_{discriminator}.ynz");
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, path, src.to_string());
    db.register_source(sf);
    (db, sf)
}

/// How many hints with this ownership modifier (`lend`, `share`, `give`, `copy`) the program gets.
fn hint_count_with_modifier(discriminator: &str, src: &str, modifier: &str) -> usize {
    let (db, sf) = single_file(discriminator, src);
    ownership_call_site_hints(&db, sf)
        .iter()
        .filter(|h| h.modifier == modifier)
        .count()
}

/// The store hints `source` gets, as (text before the hint's position, hint text) pairs.
fn store_hints(discriminator: &str, source: &str) -> Vec<(String, String)> {
    let (db, file) = single_file(discriminator, source);
    ownership_call_site_hints(&db, file)
        .into_iter()
        .filter(|hint| hint.modifier.starts_with("handed to"))
        .map(|hint| {
            let before = source[..hint.position].lines().last().unwrap_or("").trim();
            (before.to_string(), hint.modifier)
        })
        .collect()
}

fn assert_one_store_hint(discriminator: &str, source: &str, line_up_to_value: &str, hint: &str) {
    let found = store_hints(discriminator, source);
    assert_eq!(
        found,
        vec![(line_up_to_value.to_string(), hint.to_string())],
        "{source}"
    );
}

macro_rules! ownership_hint_cases {
    ($($case:ident: $body:block)+) => {
        $(
            // bug: the call-site ownership hints omit or mislabel the effective ownership of an argument (UFCS receiver, generic callee, background spawn, store handover) | tier: Medium
            #[test]
            fn $case() $body
        )+
    };
}

ownership_hint_cases! {
    // Baseline: the free-function form already worked before the UFCS fix.
    free_fn_call_emits_lend_hint: {
        let src = r#"
shape Player { health: Int }
function heal(lend self: Player, amount: Int) -> Nothing { }

function entrypoint() -> Nothing {
    let player: Player = { health: 100 }
    heal(player, 20)
}
"#;
        assert_eq!(hint_count_with_modifier("free_fn_lend", src, "lend"), 1);
    }

    // Before the fix a MethodCall fell through the `if let Expr::Call` guard: no hint for UFCS.
    ufcs_method_call_emits_lend_hint_on_receiver: {
        let src = r#"
shape Player { health: Int }
function heal(lend self: Player, amount: Int) -> Nothing { }

function entrypoint() -> Nothing {
    let player: Player = { health: 100 }
    player.heal(20)
}
"#;
        assert_eq!(hint_count_with_modifier("ufcs_lend", src, "lend"), 1);
    }

    // The receiver is parameter 0 and the other arguments are parameters 1..; both get their own
    // modifier, read from the signature and not hardcoded.
    ufcs_receiver_and_extra_arg_each_get_their_own_hint: {
        let src = r#"
shape Player { health: Int }
shape Weapon { damage: Int }
function equip(share self: Player, lend w: Weapon) -> Nothing { }

function entrypoint() -> Nothing {
    let player: Player = { health: 100 }
    let sword: Weapon = { damage: 10 }
    player.equip(sword)
}
"#;
        assert_eq!(hint_count_with_modifier("ufcs_arg", src, "share"), 1);
        assert_eq!(hint_count_with_modifier("ufcs_arg", src, "lend"), 1);
    }

    // The signature lookup stopped at `sig_table` + `imported` and never queried
    // `generic_fn_table`, so generic calls silently dropped to the no-hint fallback.
    generic_fn_lend_params_emit_lend_hints: {
        let src = r#"
function swap<T>(lend a: T, lend b: T) -> Nothing { }

function entrypoint() -> Nothing {
    let x: Int = 1
    let y: Int = 2
    swap(x, y)
}
"#;
        assert_eq!(hint_count_with_modifier("generic_lend", src, "lend"), 2);
    }

    background_unused_after_spawn_emits_give_hint: {
        let src = r#"
function worker(id: Int) -> Nothing { }

function entrypoint() -> Nothing {
    let taskId: Int = 42
    background worker(taskId)
}
"#;
        assert_eq!(hint_count_with_modifier("bg_give", src, "give"), 1);
    }

    // Inferring `give` here would be a use-after-move; the hint proves the safe direction.
    background_used_after_spawn_emits_copy_hint: {
        let src = r#"
function worker(id: Int) -> Nothing { }

function entrypoint() -> Nothing {
    let taskId: Int = 42
    background worker(taskId)
    print(taskId.toString())
}
"#;
        assert_eq!(hint_count_with_modifier("bg_copy", src, "copy"), 1);
    }

    // An unresolvable MethodCall callee must produce no hint, not a panic.
    unresolved_method_call_does_not_panic: {
        let src = r#"
shape Gadget { charge: Int }

function entrypoint() -> Nothing {
    let g: Gadget = { charge: 50 }
    g.unknownMethod()
}
"#;
        let (db, sf) = single_file("unresolved_method", src);
        let _ = ownership_call_site_hints(&db, sf);
    }

    container_add_hints_where_the_value_went: {
        assert_one_store_hint(
            "add",
            "function entrypoint() -> Nothing {
  let numbers: Array<Int> = [1]
  let list: Array<Array<Int>> = []
  list.add(numbers)
}",
            "list.add(numbers",
            "handed to `list`",
        );
    }

    field_store_hints_where_the_value_went: {
        assert_one_store_hint(
            "field",
            "shape Bag { items: Array<Int> }
function entrypoint() -> Nothing {
  let numbers: Array<Int> = [1]
  let bag: Bag = { items: [0] }
  bag.items = numbers
}",
            "bag.items = numbers",
            "handed to `bag.items`",
        );
    }

    let_hints_where_the_value_went: {
        assert_one_store_hint(
            "let",
            "function entrypoint() -> Nothing {
  let numbers: Array<Int> = [1]
  let owner: Array<Int> = numbers
  print(owner.count().toString())
}",
            "let owner: Array<Int> = numbers",
            "handed to `owner`",
        );
    }

    literal_part_hints_where_the_value_went: {
        assert_one_store_hint(
            "literal",
            "function entrypoint() -> Nothing {
  let numbers: Array<Int> = [1]
  let rows: Array<Array<Int>> = [numbers]
  print(rows.count().toString())
}",
            "let rows: Array<Array<Int>> = [numbers",
            "handed to the literal `[numbers]`",
        );
    }

    storing_bits_hints_nothing: {
        let found = store_hints(
            "bits",
            "function entrypoint() -> Nothing {
  let original: Int = 1
  let list: Array<Int> = []
  list.add(original)
  let copied: Int = original
  print(copied.toString())
}",
        );
        assert_eq!(found, Vec::<(String, String)>::new());
    }
}
