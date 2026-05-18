use std::collections::HashMap;

use inkwell::{
    context::Context,
    types::{BasicTypeEnum, StructType},
    AddressSpace,
};
use ynz_typeck::{ShapeTable, Type};

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
pub fn emit_shape_types<'ctx>(
    ctx: &'ctx Context,
    shape_table: &ShapeTable,
) -> ShapeLlvmTypes<'ctx> {
    let ptr = ctx.ptr_type(AddressSpace::default());
    let fat_ptr = ctx.struct_type(&[ptr.into(), ptr.into()], false);

    let mut named = HashMap::new();
    for (shape_name, shape_def) in &shape_table.shapes {
        let field_types: Vec<BasicTypeEnum<'ctx>> = shape_def
            .fields
            .iter()
            .map(|f| llvm_field_type(ctx, &f.ty))
            .collect();
        let struct_ty = ctx.struct_type(&field_types, false);
        named.insert(shape_name.clone(), struct_ty);
    }

    ShapeLlvmTypes { named, fat_ptr }
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
        // Number is stored as raw i128 bits (BID encoding) to match the existing
        // decimal128 ABI: load/store copies the 16 bytes, no extra indirection.
        Type::Number { .. } => ctx.i128_type().into(),
        // String, Shape, Dynamic, and all else: opaque pointer.
        _ => ptr.into(),
    }
}
