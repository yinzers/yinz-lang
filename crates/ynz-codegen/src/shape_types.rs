use std::collections::{HashMap, HashSet};

use inkwell::{
    context::Context,
    targets::TargetData,
    types::{BasicType, BasicTypeEnum, StructType},
    AddressSpace,
};
use ynz_typeck::{ShapeTable, Type};

/// One x86_64/aarch64 cache line — the false-sharing isolation unit
/// (IMP-no-function-coloring "False Sharing Auto-Padding", locked pre-v0.3).
pub const CACHE_LINE_BYTES: u32 = 64;

/// The tag a persisted union slot holds for `none` (a union with a `nothing` variant): no
/// variant has this position. The value form of `none` stays the null pointer; a slot cannot
/// hold a pointer, so the lowering translates at the slot boundary.
pub const UNION_NONE_TAG: u64 = u64::MAX;

/// The in-slot layout of one ENVELOPE union (a union with a shape variant): the ONE answer to
/// "how many bytes is a slot of this union and where is its payload". Computed once, in
/// `emit_shape_types`, from the shapes' ABI sizes; every consumer (array element size, map value
/// size, shape field type, return, staged argument, and C6's release helper) reads it through
/// [`ShapeLlvmTypes::union_layout`] and never re-derives it
/// (`.claude/rules/authoritative-derivation.md`).
///
/// A slot is `{ i64 tag, payload }` laid out like a Rust `enum`: the payload area sits at
/// `payload_offset` and is as large as the largest variant, so a variant's value lives INLINE
/// in the slot it is stored in and a store allocates nothing:
/// - a shape variant: the shape's own bytes (its `shape_abi_sizes` size);
/// - any other variant (`int`, `boolean`, `float`, `string`, array, map, wide `number`): 8
///   bytes, the value's own bits or the heap pointer the store hands over.
///
/// A RECURSIVE union (one of whose variants reaches the union itself through the fields of
/// shapes, so an inline payload would have no finite size) is `boxed`: its slot is
/// `{ i64 tag, i64 word }` and a shape variant's bytes live in a heap cell the slot owns, with
/// `word` the cell pointer. Everything else is inline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnionLayout {
    /// The payload is a heap cell for a shape variant (recursive union).
    pub boxed: bool,
    /// Byte offset of the payload area from the slot start.
    pub payload_offset: u64,
    /// Bytes of the payload area (the largest variant, rounded up to `slot_align`).
    pub payload_size: u64,
    /// Total slot bytes: a multiple of `slot_align`, so array elements stay aligned.
    pub slot_size: u64,
    /// Slot alignment: 8, or 16 when a shape variant holds a decimal128.
    pub slot_align: u64,
    /// `(variant index, shape name)` of each shape variant, in variant order.
    pub shape_variants: Vec<(u64, String)>,
}

/// Canonical key of a union: its resolved variants in order. Two unions are the same slot type
/// exactly when their variant lists are equal.
pub fn union_key(variants: &[Type]) -> String {
    format!("{variants:?}")
}

fn align_up(value: u64, align: u64) -> u64 {
    value.div_ceil(align) * align
}

/// Compute the layout of the union `variants` from the shapes' `(abi size, abi align)` in `abi`.
/// Errors when `variants` has no shape variant (not an envelope union) or when an inline layout
/// needs the ABI of a shape that is not in `abi` yet.
fn compute_union_layout(
    variants: &[Type],
    boxed: bool,
    abi: &HashMap<String, (u64, u64)>,
) -> Result<UnionLayout, String> {
    let shape_variants: Vec<(u64, String)> = variants
        .iter()
        .enumerate()
        .filter_map(|(index, variant)| match variant {
            Type::Shape { name } => Some((index as u64, name.clone())),
            _ => None,
        })
        .collect();
    if shape_variants.is_empty() {
        return Err(format!(
            "union layout: `{variants:?}` has no shape variant, so it is not an envelope union"
        ));
    }
    if boxed {
        return Ok(UnionLayout {
            boxed,
            payload_offset: 8,
            payload_size: 8,
            slot_size: 16,
            slot_align: 8,
            shape_variants,
        });
    }
    let mut payload_bytes = 8u64;
    let mut slot_align = 8u64;
    for (_, name) in &shape_variants {
        let (size, align) = abi
            .get(name)
            .copied()
            .ok_or_else(|| format!("union layout: shape `{name}` has no ABI size yet"))?;
        payload_bytes = payload_bytes.max(size);
        slot_align = slot_align.max(align);
    }
    let payload_offset = slot_align;
    let payload_size = align_up(payload_bytes, slot_align);
    Ok(UnionLayout {
        boxed,
        payload_offset,
        payload_size,
        slot_size: payload_offset + payload_size,
        slot_align,
        shape_variants,
    })
}

/// LLVM struct types emitted for each Yinz shape + the fat-pointer struct for
/// `dynamic Foo` values.
pub struct ShapeLlvmTypes<'ctx> {
    /// One struct type per named shape.
    pub named: HashMap<String, StructType<'ctx>>,
    /// Fat-pointer type `{ ptr data, ptr vtable }` shared by all `dynamic Foo` values.
    pub fat_ptr: StructType<'ctx>,
    /// `(ABI size, ABI alignment)` of every shape, measured on the module's TargetData.
    abi: HashMap<String, (u64, u64)>,
    /// Keys (`union_key`) of the recursive unions, whose slot is boxed.
    boxed_unions: HashSet<String>,
}

impl<'ctx> ShapeLlvmTypes<'ctx> {
    pub fn get(&self, name: &str) -> Option<StructType<'ctx>> {
        self.named.get(name).copied()
    }

    /// The slot layout of the envelope union `variants` (resolved, in order): the one source
    /// for every consumer of a union slot.
    ///
    /// Time: O(v) for v variants  Space: O(v)
    pub fn union_layout(&self, variants: &[Type]) -> Result<UnionLayout, String> {
        compute_union_layout(
            variants,
            self.boxed_unions.contains(&union_key(variants)),
            &self.abi,
        )
    }

    /// The LLVM type of one slot of `layout`: `{ i64 tag, payload }`, the payload area an array
    /// of `i64` (or of `i128` when the slot is 16-aligned) so the struct's alignment and size
    /// are exactly `slot_align` and `slot_size`.
    pub fn union_slot_llvm(&self, ctx: &'ctx Context, layout: &UnionLayout) -> StructType<'ctx> {
        union_slot_struct(ctx, layout)
    }
}

fn union_slot_struct<'ctx>(ctx: &'ctx Context, layout: &UnionLayout) -> StructType<'ctx> {
    let i64_type = ctx.i64_type();
    if layout.boxed {
        return ctx.struct_type(&[i64_type.into(), i64_type.into()], false);
    }
    if layout.slot_align == 16 {
        ctx.struct_type(
            &[
                i64_type.into(),
                i64_type.into(),
                ctx.i128_type()
                    .array_type((layout.payload_size / 16) as u32)
                    .into(),
            ],
            false,
        )
    } else {
        ctx.struct_type(
            &[
                i64_type.into(),
                i64_type.array_type((layout.payload_size / 8) as u32).into(),
            ],
            false,
        )
    }
}

/// The envelope unions among `ty`'s direct fields: the only inline containment between shapes.
fn field_unions(shape_table: &ShapeTable, shape_name: &str) -> Vec<Vec<Type>> {
    shape_table
        .layout_shape(shape_name)
        .map(|shape| {
            shape
                .fields
                .iter()
                .filter(|field| ynz_typeck::check::is_envelope_union(&field.ty))
                .filter_map(|field| match &field.ty {
                    Type::Union { variants } => Some(variants.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Keys of the recursive unions: `U` is recursive when one of its shape variants reaches a
/// shape with a field of the same union `U` through union fields. Boxing exactly these breaks
/// every inline-containment cycle, and the answer does not depend on any traversal order, so
/// every module that sees the same shapes boxes the same unions.
///
/// Time: O(u * (s + f)) for u field unions, s shapes, f fields  Space: O(s)
fn recursive_unions(shape_table: &ShapeTable) -> HashSet<String> {
    let mut recursive = HashSet::new();
    let names = layout_shape_names(shape_table);
    for name in names {
        for variants in field_unions(shape_table, &name) {
            let key = union_key(&variants);
            if recursive.contains(&key) {
                continue;
            }
            let mut stack: Vec<String> = variants
                .iter()
                .filter_map(|variant| match variant {
                    Type::Shape { name } => Some(name.clone()),
                    _ => None,
                })
                .collect();
            let mut visited: HashSet<String> = HashSet::new();
            while let Some(reached) = stack.pop() {
                if !visited.insert(reached.clone()) {
                    continue;
                }
                for inner in field_unions(shape_table, &reached) {
                    if union_key(&inner) == key {
                        recursive.insert(key.clone());
                    }
                    for variant in &inner {
                        if let Type::Shape { name } = variant {
                            stack.push(name.clone());
                        }
                    }
                }
            }
        }
    }
    recursive
}

/// Every shape that gets a layout, in name order: the named shapes and the layout-only ones an
/// imported shape's union fields need. Sorted, so the layout never depends on hash order.
fn layout_shape_names(shape_table: &ShapeTable) -> Vec<String> {
    let mut names: Vec<String> = shape_table
        .shapes
        .keys()
        .chain(shape_table.layout_only.keys())
        .cloned()
        .collect();
    names.sort();
    names.dedup();
    names
}

struct ShapeTypeBuilder<'a, 'ctx> {
    ctx: &'ctx Context,
    shape_table: &'a ShapeTable,
    padded_shapes: &'a HashSet<String>,
    target_data: &'a TargetData,
    boxed_unions: HashSet<String>,
    named: HashMap<String, StructType<'ctx>>,
    abi: HashMap<String, (u64, u64)>,
    in_progress: HashSet<String>,
}

impl<'a, 'ctx> ShapeTypeBuilder<'a, 'ctx> {
    /// Build `name`'s struct type (and, first, those of the shapes its inline union fields
    /// need). Boxing every recursive union leaves the inline dependencies acyclic, so a shape
    /// found in progress is a compiler bug.
    fn build(&mut self, name: &str) -> Result<(), String> {
        if self.named.contains_key(name) {
            return Ok(());
        }
        if !self.in_progress.insert(name.to_string()) {
            return Err(format!(
                "shape layout: `{name}` contains itself inline although every recursive union \
                 is boxed; this is a compiler bug"
            ));
        }
        let shape_def = self
            .shape_table
            .layout_shape(name)
            .ok_or_else(|| format!("shape layout: shape `{name}` is not in the shape table"))?;
        let padded = self.padded_shapes.contains(name);
        let mut field_types: Vec<BasicTypeEnum<'ctx>> = Vec::new();
        for field in &shape_def.fields {
            let field_type = self.field_type(&field.ty)?;
            field_types.push(if padded {
                self.pad_field_to_cache_line(field_type)
            } else {
                field_type
            });
        }
        let struct_ty = self.ctx.struct_type(&field_types, false);
        self.abi.insert(
            name.to_string(),
            (
                self.target_data.get_abi_size(&struct_ty),
                u64::from(self.target_data.get_abi_alignment(&struct_ty)),
            ),
        );
        self.named.insert(name.to_string(), struct_ty);
        self.in_progress.remove(name);
        Ok(())
    }

    /// Map a field's typeck type to its LLVM type inside a struct.
    ///
    /// Primitives are stored by value. An envelope union is stored INLINE as its slot (the
    /// `UnionLayout`). Everything else (strings, nested shapes, dynamic values) is stored as an
    /// opaque pointer.
    ///
    /// Auto-Arc coupling (v0.3-M8 Phase 5): `ynz_typeck::types::arc_shareable` shares a shape
    /// across `background` tasks as ONE byte copy of this struct layout, so its allowlist is
    /// exactly the field kinds stored INLINE here that a byte copy shares soundly (`int`/
    /// `float`/`bool` value bits, plus the `string` pointer the copy path already shares). A
    /// union field stays out of it: its slot may own a heap cell.
    fn field_type(&mut self, ty: &Type) -> Result<BasicTypeEnum<'ctx>, String> {
        let ptr = self.ctx.ptr_type(AddressSpace::default());
        Ok(match ty {
            Type::Int => self.ctx.i64_type().into(),
            Type::Float => self.ctx.f64_type().into(),
            Type::Bool => self.ctx.bool_type().into(),
            // N ≤ 34: decimal128 stored as raw i128 bits (BID encoding).
            // N > 34: bignum stored as a pointer to the heap decimal string.
            Type::Number { precision } if *precision <= 34 => self.ctx.i128_type().into(),
            Type::Number { .. } => ptr.into(),
            Type::Union { variants } if ynz_typeck::check::is_envelope_union(ty) => {
                let boxed = self.boxed_unions.contains(&union_key(variants));
                if !boxed {
                    for variant in variants {
                        if let Type::Shape { name } = variant {
                            self.build(name)?;
                        }
                    }
                }
                let layout = compute_union_layout(variants, boxed, &self.abi)?;
                union_slot_struct(self.ctx, &layout).into()
            }
            // String, Shape, Dynamic, and all else: opaque pointer.
            _ => ptr.into(),
        })
    }

    /// Wrap a field's LLVM type as `{ T, [pad x i8] }` so the element occupies a whole number
    /// of [`CACHE_LINE_BYTES`] lines (exactly one for every field of at most 64 bytes). The
    /// inner `T` starts at offset 0 of the wrapper, so a GEP to the wrapper IS a pointer to `T`
    /// (opaque pointers) — no load/store site changes.
    fn pad_field_to_cache_line(&self, field_type: BasicTypeEnum<'ctx>) -> BasicTypeEnum<'ctx> {
        let payload_bytes = self.target_data.get_abi_size(&field_type);
        let line = u64::from(CACHE_LINE_BYTES);
        let pad = align_up(payload_bytes, line) - payload_bytes;
        self.ctx
            .struct_type(
                &[field_type, self.ctx.i8_type().array_type(pad as u32).into()],
                false,
            )
            .as_basic_type_enum()
    }
}

/// Emit one LLVM struct type per shape in the shape table, plus the fat-pointer type, and the
/// slot layout of every recursive union.
///
/// Field order matches ShapeDef::fields (inherited fields first, then own fields).
/// Non-primitive field types (string, nested shapes, dynamic) are stored as opaque
/// pointers. Numbers are stored as i128 (16 bytes, BID encoding) to match the
/// existing decimal128 representation. An envelope union is stored inline as its slot
/// (`UnionLayout`).
///
/// `target_data` is the module's TargetData: the shapes' ABI sizes feed the union slot sizes, so
/// they are measured here, in dependency order, and kept for `ShapeLlvmTypes::union_layout`.
///
/// # False-sharing auto-padding (v0.3-M4 Phase 4)
///
/// Shapes named in `padded_shapes` — THE authoritative padded set derived once by
/// `ynz_typeck::false_sharing` from the `background`-boundary analysis (never re-derived
/// here) — get cache-line field isolation: each field's element type is wrapped as
/// `{ T, [pad x i8] }` sized to a whole number of [`CACHE_LINE_BYTES`] lines, so field `i` of
/// a field of at most 64 bytes sits at byte offset `64·i` and the struct's ABI size is `64·n`.
///
/// The wrapper design keeps the LOGICAL field index equal to the LLVM element index
/// (no index remapping at any of the GEP sites), and — with opaque pointers — a GEP to
/// element `i` is byte-identical to a pointer to the inner `T`, so every existing
/// field load/store is layout-correct unchanged. Whole-struct copies (`size_of()` +
/// memcpy / struct load-store) and frame-slot sizing (`frame_layouts_query` measures
/// these same types) grow with the padding automatically.
///
/// Cache-line isolation argument: every Yinz field payload is ≤ 16 bytes (i64/f64/ptr
/// = 8, i1 = 1, i128 = 16) except an inline union slot, and heap shape allocations come from
/// `ynz_alloc` (malloc, ≥ 16-byte aligned), so a field at offset `64·i` spans bytes
/// `[B+64i, B+64i+16)` with `B mod 64 ∈ {0,16,32,48}` — always within ONE 64-byte line, and
/// distinct fields land on distinct lines. A union slot larger than a line spans whole lines of
/// its own, so it never shares one with another field. Stack literals for padded shapes
/// additionally get their alloca alignment forced to 64 at the `lower_struct_lit` site.
pub fn emit_shape_types<'ctx>(
    ctx: &'ctx Context,
    shape_table: &ShapeTable,
    padded_shapes: &HashSet<String>,
    target_data: &TargetData,
) -> Result<ShapeLlvmTypes<'ctx>, String> {
    let ptr = ctx.ptr_type(AddressSpace::default());
    let fat_ptr = ctx.struct_type(&[ptr.into(), ptr.into()], false);

    let mut builder = ShapeTypeBuilder {
        ctx,
        shape_table,
        padded_shapes,
        target_data,
        boxed_unions: recursive_unions(shape_table),
        named: HashMap::new(),
        abi: HashMap::new(),
        in_progress: HashSet::new(),
    };
    let names = layout_shape_names(shape_table);
    for name in &names {
        builder.build(name)?;
    }
    Ok(ShapeLlvmTypes {
        named: builder.named,
        fat_ptr,
        abi: builder.abi,
        boxed_unions: builder.boxed_unions,
    })
}

/// The `{ i64 tag, i64 data }` VALUE-form envelope of an envelope union: what a union value
/// points to while it is borrowed inside one frame (`data` is the variant's bits, or the
/// address of a shape variant's bytes). A SLOT of a container, field or return is the
/// [`UnionLayout`] instead.
pub fn union_envelope_type<'ctx>(ctx: &'ctx Context) -> StructType<'ctx> {
    ctx.struct_type(&[ctx.i64_type().into(), ctx.i64_type().into()], false)
}

#[cfg(test)]
mod union_layout_tests {
    use super::*;
    use ynz_diagnostics::SourceSpan;
    use ynz_typeck::shapes::{FieldDef, ShapeDef};

    fn shape(name: &str) -> Type {
        Type::Shape {
            name: name.to_string(),
        }
    }

    fn shape_def(name: &str, fields: Vec<(&str, Type)>) -> ShapeDef {
        let span = SourceSpan {
            file: String::new(),
            start: 0,
            end: 0,
        };
        ShapeDef {
            name: name.to_string(),
            is_base: false,
            extends: None,
            follows: Vec::new(),
            fields: fields
                .into_iter()
                .map(|(field_name, ty)| FieldDef {
                    name: field_name.to_string(),
                    ty,
                    is_hidden: false,
                    is_inherited: false,
                    defined_at: span.clone(),
                })
                .collect(),
            contract_sigs: Vec::new(),
            defined_at: span,
        }
    }

    fn abi(entries: &[(&str, u64, u64)]) -> HashMap<String, (u64, u64)> {
        entries
            .iter()
            .map(|(name, size, align)| (name.to_string(), (*size, *align)))
            .collect()
    }

    // Tier: High (layout is data integrity: every slot size, offset and alignment derives here).
    // Bug caught: a slot smaller than its largest variant overwrites the next array element;
    // a payload offset below the shape's alignment is a misaligned load.
    #[test]
    fn slot_is_the_tag_plus_the_largest_variant_rounded_to_the_alignment() {
        let variants = [shape("Tiny"), shape("Big")];
        let cases = [
            // (Tiny size/align, Big size/align) -> (offset, payload, slot, align)
            ((1, 1), (16, 8), (8, 16, 24, 8)),
            // a shape variant smaller than the 8 bytes a scalar variant needs
            ((1, 1), (1, 1), (8, 8, 16, 8)),
            // a decimal128 field makes a variant 16-aligned: the payload moves to offset 16
            ((1, 1), (32, 16), (16, 32, 48, 16)),
            // a size that is no multiple of the alignment rounds up
            ((12, 4), (20, 4), (8, 24, 32, 8)),
        ];
        for ((tiny_size, tiny_align), (big_size, big_align), expected) in cases {
            let sizes = abi(&[
                ("Tiny", tiny_size, tiny_align),
                ("Big", big_size, big_align),
            ]);
            let layout = compute_union_layout(&variants, false, &sizes).expect("layout");
            let (offset, payload, slot, align) = expected;
            assert_eq!(
                (
                    layout.payload_offset,
                    layout.payload_size,
                    layout.slot_size,
                    layout.slot_align
                ),
                (offset, payload, slot, align),
                "{tiny_size}/{big_size}"
            );
            assert_eq!(
                layout.slot_size % layout.slot_align,
                0,
                "array stride aligned"
            );
            assert!(!layout.boxed);
        }
    }

    #[test]
    fn a_non_shape_variant_needs_eight_payload_bytes_and_a_boxed_slot_is_sixteen() {
        let sizes = abi(&[("Tiny", 1, 1)]);
        let with_int = [shape("Tiny"), Type::Int];
        let inline = compute_union_layout(&with_int, false, &sizes).expect("layout");
        assert_eq!((inline.payload_size, inline.slot_size), (8, 16));
        let boxed = compute_union_layout(&with_int, true, &HashMap::new()).expect("layout");
        assert_eq!(
            (
                boxed.boxed,
                boxed.payload_offset,
                boxed.slot_size,
                boxed.slot_align
            ),
            (true, 8, 16, 8)
        );
        // a union with no shape variant has no envelope: refused, never a silent layout
        assert!(compute_union_layout(&[Type::Int, Type::Nothing], false, &sizes).is_err());
    }

    // Bug caught: an unboxed recursive union has no finite size, and a traversal-order
    // dependent choice would box different unions in two modules that import each other.
    #[test]
    fn recursive_unions_are_exactly_those_that_reach_themselves() {
        let node = Type::Union {
            variants: vec![shape("Node"), Type::Nothing],
        };
        let left = Type::Union {
            variants: vec![shape("Right"), Type::Nothing],
        };
        let right = Type::Union {
            variants: vec![shape("Left"), Type::Nothing],
        };
        let plain = Type::Union {
            variants: vec![shape("Leaf"), Type::Int],
        };
        let mut table = ShapeTable::default();
        for definition in [
            shape_def("Node", vec![("next", node.clone())]),
            shape_def("Left", vec![("other", left.clone())]),
            shape_def("Right", vec![("other", right.clone())]),
            shape_def("Leaf", vec![("count", Type::Int)]),
            shape_def("Holder", vec![("slot", plain.clone())]),
        ] {
            table.shapes.insert(definition.name.clone(), definition);
        }
        let recursive = recursive_unions(&table);
        let key_of = |ty: &Type| match ty {
            Type::Union { variants } => union_key(variants),
            _ => unreachable!(),
        };
        assert!(recursive.contains(&key_of(&node)), "self-recursive");
        assert!(recursive.contains(&key_of(&left)), "mutual: left");
        assert!(recursive.contains(&key_of(&right)), "mutual: right");
        assert!(
            !recursive.contains(&key_of(&plain)),
            "a plain union stays inline"
        );
        assert_eq!(recursive.len(), 3);
    }
}
