use std::collections::{HashMap, HashSet};

use inkwell::{
    context::Context,
    types::{BasicType, BasicTypeEnum, StructType},
    AddressSpace,
};
use ynz_typeck::{ShapeTable, Type};

/// One x86_64/aarch64 cache line — the false-sharing isolation unit
/// (IMP-no-function-coloring "False Sharing Auto-Padding", locked pre-v0.3).
pub const CACHE_LINE_BYTES: u32 = 64;

/// LLVM struct types emitted for each Yinz shape + the fat-pointer struct for
/// `dynamic Foo` values.
pub struct ShapeLlvmTypes<'ctx> {
    /// One struct type per named shape.
    pub named: HashMap<String, StructType<'ctx>>,
    /// Fat-pointer type `{ ptr data, ptr vtable }` shared by all `dynamic Foo` values.
    pub fat_ptr: StructType<'ctx>,
}

impl<'ctx> ShapeLlvmTypes<'ctx> {
    pub fn get(&self, name: &str) -> Option<StructType<'ctx>> {
        self.named.get(name).copied()
    }
}

/// Emit one LLVM struct type per shape in the shape table, plus the fat-pointer type.
///
/// Field order matches ShapeDef::fields (inherited fields first, then own fields).
/// Non-primitive field types (string, nested shapes, dynamic) are stored as opaque
/// pointers. Numbers are stored as i128 (16 bytes, BID encoding) to match the
/// existing decimal128 representation.
///
/// # False-sharing auto-padding (v0.3-M4 Phase 4)
///
/// Shapes named in `padded_shapes` — THE authoritative padded set derived once by
/// `ynz_typeck::false_sharing` from the `background`-boundary analysis (never re-derived
/// here) — get 64-byte cache-line field isolation: each field's element type is wrapped
/// as `{ T, [pad x i8] }` sized to exactly [`CACHE_LINE_BYTES`], so field `i` sits at
/// byte offset `64·i` and the struct's ABI size is `64·n`.
///
/// The wrapper design keeps the LOGICAL field index equal to the LLVM element index
/// (no index remapping at any of the GEP sites), and — with opaque pointers — a GEP to
/// element `i` is byte-identical to a pointer to the inner `T`, so every existing
/// field load/store is layout-correct unchanged. Whole-struct copies (`size_of()` +
/// memcpy / struct load-store) and frame-slot sizing (`frame_layouts_query` measures
/// these same types) grow with the padding automatically.
///
/// Cache-line isolation argument: every Yinz field payload is ≤ 16 bytes (i64/f64/ptr
/// = 8, i1 = 1, i128 = 16) and heap shape allocations come from `ynz_alloc` (malloc,
/// ≥ 16-byte aligned), so a field at offset `64·i` spans bytes `[B+64i, B+64i+16)` with
/// `B mod 64 ∈ {0,16,32,48}` — always within ONE 64-byte line, and distinct fields land
/// on distinct lines. Stack literals for padded shapes additionally get their alloca
/// alignment forced to 64 at the `lower_struct_lit` site.
pub fn emit_shape_types<'ctx>(
    ctx: &'ctx Context,
    shape_table: &ShapeTable,
    padded_shapes: &HashSet<String>,
) -> ShapeLlvmTypes<'ctx> {
    let ptr = ctx.ptr_type(AddressSpace::default());
    let fat_ptr = ctx.struct_type(&[ptr.into(), ptr.into()], false);

    let mut named = HashMap::new();
    for (shape_name, shape_def) in &shape_table.shapes {
        let padded = padded_shapes.contains(shape_name);
        let field_types: Vec<BasicTypeEnum<'ctx>> = shape_def
            .fields
            .iter()
            .map(|f| {
                let t = llvm_field_type(ctx, &f.ty);
                if padded {
                    pad_field_to_cache_line(ctx, t)
                } else {
                    t
                }
            })
            .collect();
        let struct_ty = ctx.struct_type(&field_types, false);
        named.insert(shape_name.clone(), struct_ty);
    }

    ShapeLlvmTypes { named, fat_ptr }
}

/// Wrap a field's LLVM type as `{ T, [pad x i8] }` so the element occupies exactly one
/// [`CACHE_LINE_BYTES`] slot. The inner `T` starts at offset 0 of the wrapper, so a GEP
/// to the wrapper IS a pointer to `T` (opaque pointers) — no load/store site changes.
fn pad_field_to_cache_line<'ctx>(
    ctx: &'ctx Context,
    t: BasicTypeEnum<'ctx>,
) -> BasicTypeEnum<'ctx> {
    let payload_bytes: u32 = match t {
        BasicTypeEnum::IntType(it) => it.get_bit_width().div_ceil(8),
        BasicTypeEnum::FloatType(_) => 8,
        BasicTypeEnum::PointerType(_) => 8,
        // No other field payload type is emitted by llvm_field_type today; a new one
        // must be classified here so its pad keeps the 64-byte slot exact.
        other => panic!("pad_field_to_cache_line: unsupported field payload type {other:?}"),
    };
    assert!(
        payload_bytes <= CACHE_LINE_BYTES,
        "field payload ({payload_bytes} bytes) exceeds one cache line — \
         the per-field 64-byte slot invariant no longer holds"
    );
    let pad = CACHE_LINE_BYTES - payload_bytes;
    ctx.struct_type(&[t, ctx.i8_type().array_type(pad).into()], false)
        .as_basic_type_enum()
}

/// Map a field's typeck type to its LLVM type inside a struct.
///
/// Primitives are stored by value. Everything else (strings, nested shapes,
/// dynamic values) is stored as an opaque pointer.
pub fn llvm_field_type<'ctx>(ctx: &'ctx Context, ty: &Type) -> BasicTypeEnum<'ctx> {
    let ptr = ctx.ptr_type(AddressSpace::default());
    match ty {
        Type::Int => ctx.i64_type().into(),
        Type::Float => ctx.f64_type().into(),
        Type::Bool => ctx.bool_type().into(),
        // N ≤ 34: decimal128 stored as raw i128 bits (BID encoding).
        // N > 34: bignum stored as a pointer to the heap decimal string.
        Type::Number { precision } if *precision <= 34 => ctx.i128_type().into(),
        Type::Number { .. } => ptr.into(),
        // String, Shape, Dynamic, and all else: opaque pointer.
        _ => ptr.into(),
    }
}
