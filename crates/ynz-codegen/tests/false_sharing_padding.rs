// False-sharing auto-padding layout proof (v0.3-M4 Phase 4).
//
// WHY: the padding transform's whole value is WHERE fields land in memory — a wrong
// offset silently forfeits the cache-line isolation (or worse, disagrees with the
// frame-slot sizing measured from the same types). These tests measure the emitted
// LLVM struct types with the REAL target's data layout (`TargetData::offset_of_element`
// / `get_abi_size`) and pin the invariants:
//   - padded shape: field i at byte offset 64·i, ABI size 64·n — every field on its
//     own 64-byte cache line;
//   - unpadded shape: natural (declaration-order, no padding slots) layout — proving
//     the transform keys on the padded SET, not on shape-ness.

use std::collections::HashSet;

use inkwell::context::Context;
use ynz_codegen::shape_types::{emit_shape_types, CACHE_LINE_BYTES};
use ynz_codegen::state_machine::default_target_machine;
use ynz_diagnostics::SourceSpan;
use ynz_typeck::shapes::{FieldDef, ShapeDef};
use ynz_typeck::{ShapeTable, Type};

fn span() -> SourceSpan {
    SourceSpan::new("test.ynz", 0, 0)
}

fn field(name: &str, ty: Type) -> FieldDef {
    FieldDef {
        name: name.to_string(),
        ty,
        is_hidden: false,
        is_inherited: false,
        defined_at: span(),
    }
}

/// A shape covering every field payload class llvm_field_type emits:
/// i64 (int), f64 (float), i1 (bool), i128 (number), ptr (string).
fn tally_shape_table() -> ShapeTable {
    let mut table = ShapeTable::empty();
    table.shapes.insert(
        "Tally".to_string(),
        ShapeDef {
            name: "Tally".to_string(),
            is_base: false,
            extends: None,
            follows: vec![],
            fields: vec![
                field("hits", Type::Int),
                field("rate", Type::Float),
                field("active", Type::Bool),
                field("avg", Type::Number { precision: 34 }),
                field("label", Type::String),
            ],
            contract_sigs: vec![],
            defined_at: span(),
        },
    );
    table
}

/// Measure (per-element byte offsets, total ABI size) for a named shape's struct type.
fn measure(padded: &HashSet<String>) -> (Vec<u64>, u64) {
    let machine = default_target_machine().expect("target machine");
    let ctx = Context::create();
    let module = ctx.create_module("padding_test");
    module.set_triple(&machine.get_triple());
    module.set_data_layout(&machine.get_target_data().get_data_layout());
    let dl_owned = module.get_data_layout();
    let dl_str = dl_owned.as_str().to_str().unwrap_or("");
    let target_data = inkwell::targets::TargetData::create(dl_str);

    let table = tally_shape_table();
    let shape_types = emit_shape_types(&ctx, &table, padded);
    let struct_ty = shape_types.get("Tally").expect("Tally struct type");

    let n = struct_ty.count_fields();
    let offsets: Vec<u64> = (0..n)
        .map(|i| {
            target_data
                .offset_of_element(&struct_ty, i)
                .expect("element offset")
        })
        .collect();
    (offsets, target_data.get_abi_size(&struct_ty))
}

#[test]
fn padded_shape_fields_land_on_distinct_cache_lines() {
    // WHY: this is the exit-criteria proof "padded field offsets verified on real
    // 64-byte cache lines" — each field starts exactly at 64·i on the REAL target's
    // data layout, so no two fields can share a cache line (payloads are ≤ 16 bytes).
    let padded: HashSet<String> = HashSet::from(["Tally".to_string()]);
    let (offsets, size) = measure(&padded);

    assert_eq!(
        offsets.len(),
        5,
        "one element per declared field (no index shift)"
    );
    for (i, off) in offsets.iter().enumerate() {
        assert_eq!(
            *off,
            (i as u64) * u64::from(CACHE_LINE_BYTES),
            "padded field {i} must start at byte 64·{i}; offsets: {offsets:?}"
        );
    }
    assert_eq!(
        size,
        5 * u64::from(CACHE_LINE_BYTES),
        "padded struct ABI size must be 64·n so whole-struct copies and frame slots carry the padding"
    );
}

#[test]
fn unpadded_shape_keeps_natural_layout() {
    // WHY: the --no-auto-parallel gating and the module-visibility decline both depend
    // on "not in the padded set" producing the GENUINELY unpadded layout — not merely
    // a differently-padded one. Natural layout for {i64,f64,i1,i128,ptr} on x86_64:
    // 0, 8, 16, 32 (i128 aligns to 16), 48 — total 56 (size rounds to align 16 → 64...
    // measured exactly below, not assumed).
    let padded: HashSet<String> = HashSet::new();
    let (offsets, size) = measure(&padded);

    assert_eq!(offsets.len(), 5);
    assert_eq!(offsets[0], 0, "field 0 at natural offset");
    assert_eq!(offsets[1], 8, "f64 packs right after i64 — no padding slot");
    assert_eq!(offsets[2], 16, "i1 at 16 — no padding slot");
    assert_eq!(offsets[3], 32, "i128 aligns to 16 → 32, not a 64-multiple");
    assert_eq!(offsets[4], 48, "ptr after i128 — no padding slot");
    assert!(
        size < 5 * u64::from(CACHE_LINE_BYTES),
        "unpadded struct must be strictly smaller than the padded form; got {size}"
    );
    // At least two fields share one 64-byte line in the natural layout — the padded
    // form is a genuine layout change, not a relabel.
    assert_eq!(
        offsets[0] / 64,
        offsets[1] / 64,
        "sanity: natural layout co-locates fields 0 and 1 on one cache line"
    );
}
