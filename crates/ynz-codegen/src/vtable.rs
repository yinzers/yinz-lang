use std::collections::HashMap;

use inkwell::{module::Module, values::GlobalValue, AddressSpace};
use ynz_typeck::ShapeTable;

/// Key: (concrete_shape_name, contract_name).
pub type VtableMap<'ctx> = HashMap<(String, String), GlobalValue<'ctx>>;

/// Emit one vtable global per (concrete shape, contract) pair used in the module.
///
/// A vtable is an array of function pointers ordered by the contract's
/// `contract_sigs` declaration order. The concrete shape provides one standalone
/// function per method slot (resolved via UFCS: method name matches function name).
///
/// Must be called AFTER all functions have been forward-declared so that their
/// LLVM values exist for the initialiser.
///
/// Vtable naming: `vtable_<ShapeName>_<ContractName>` (private linkage).
pub fn emit_vtable_globals<'ctx>(
    module: &Module<'ctx>,
    shape_table: &ShapeTable,
) -> VtableMap<'ctx> {
    let ptr = module.get_context().ptr_type(AddressSpace::default());
    let mut vtables = HashMap::new();

    // Iterate in sorted (shape_name, contract_name) order: this loop's iteration order is
    // GLOBAL EMISSION ORDER for the vtable globals in the module's IR text, and
    // `shape_table.shapes` is a per-process-seeded HashMap — unsorted iteration made any
    // ≥2-vtable module's binary/IR flap run-to-run (v0.3-M7 R10 determinism fix, FRAGO 013;
    // DCE-masked in optimized builds, live under `--no-optimize`; the imported-fns twin
    // lives in emit.rs Pass 0.25, the mono-table twin in typeck's BTreeMap).
    let mut pairs: Vec<(&String, &String)> = shape_table
        .shapes
        .iter()
        .flat_map(|(shape_name, shape_def)| shape_def.follows.iter().map(move |c| (shape_name, c)))
        .collect();
    pairs.sort();

    for (shape_name, contract_name) in pairs {
        let Some(contract_def) = shape_table.get(contract_name) else {
            continue;
        };

        // Build the array of function pointers — one per contract method slot.
        let fn_ptrs: Vec<_> = contract_def
            .contract_sigs
            .iter()
            .map(|sig| {
                module
                    .get_function(&sig.name)
                    .map(|f| f.as_global_value().as_pointer_value())
                    .unwrap_or_else(|| ptr.const_null())
            })
            .collect();

        let arr_ty = ptr.array_type(fn_ptrs.len() as u32);
        let arr_val = ptr.const_array(&fn_ptrs);

        let vtable_name = format!("vtable_{}_{}", shape_name, contract_name);
        let g = module.add_global(arr_ty, Some(AddressSpace::default()), &vtable_name);
        g.set_initializer(&arr_val);
        g.set_constant(true);
        g.set_linkage(inkwell::module::Linkage::Private);

        vtables.insert((shape_name.clone(), contract_name.clone()), g);
    }

    vtables
}

/// Find the slot index of `method_name` in `contract_name`'s method list.
pub fn method_slot(
    shape_table: &ShapeTable,
    contract_name: &str,
    method_name: &str,
) -> Option<u32> {
    let contract = shape_table.get(contract_name)?;
    contract
        .contract_sigs
        .iter()
        .position(|s| s.name == method_name)
        .map(|i| i as u32)
}
