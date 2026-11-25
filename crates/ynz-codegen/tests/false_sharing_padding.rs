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
use ynz_codegen::state_machine::{default_target_machine, PipelineConfig};
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
    let machine = default_target_machine(None, PipelineConfig::o0()).expect("target machine");
    let ctx = Context::create();
    let module = ctx.create_module("padding_test");
    module.set_triple(&machine.get_triple());
    module.set_data_layout(&machine.get_target_data().get_data_layout());
    let dl_owned = module.get_data_layout();
    let dl_str = dl_owned.as_str().to_str().unwrap_or("");
    let target_data = inkwell::targets::TargetData::create(dl_str);

    let table = tally_shape_table();
    let shape_types =
        emit_shape_types(&ctx, &table, padded, &target_data).expect("shape types emit");
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

/// One named test per case so the runner names the failing case. Each case lists the byte offset
/// of every field and the struct's ABI size on the real target's data layout.
macro_rules! shape_layout_case {
    ($test_name:ident, padded: $padded:expr, offsets: $offsets:expr, abi_size: $abi_size:expr) => {
        // bug: the padding transform keys on something other than the padded set (or puts a field off its own cache line), so two hot fields share a line, or an unpadded shape grows, or frame slots are sized for a layout the shape does not have | tier: Medium
        #[test]
        fn $test_name() {
            let padded: HashSet<String> = $padded;
            let (offsets, abi_size) = measure(&padded);
            assert_eq!(offsets, $offsets);
            assert_eq!(abi_size, $abi_size);
        }
    };
}

// Case bug: a padded shape's field i is not at byte 64 * i, so fields share a cache line.
shape_layout_case!(
    padded_shape_fields_land_on_distinct_cache_lines,
    padded: HashSet::from(["Tally".to_string()]),
    offsets: vec![0, 64, 128, 192, 256],
    abi_size: 5 * u64::from(CACHE_LINE_BYTES)
);
// Case bug: a shape outside the padded set is padded anyway (or laid out with padding slots).
shape_layout_case!(
    unpadded_shape_keeps_natural_layout,
    padded: HashSet::new(),
    offsets: vec![0, 8, 16, 32, 48],
    abi_size: 64
);
