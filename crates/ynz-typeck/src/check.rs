use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::{
    BinOpKind, Block, CallExpr, Expr, FunctionDecl, Item, MatchArm, MatchPatternKind, Module,
    OwnershipModifier, PostfixOpKind, Stmt, StructLitField, Type as AstType, UnaryOpKind,
};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

use crate::{
    builtins::{
        array_method_is_mutating, array_method_return, fixed_method_is_mutating,
        fixed_method_return, map_method_is_mutating, map_method_return, maybe_method_return,
        sensitive_method_return, string_method_return,
    },
    generics::{
        apply_substitution, unify_param, GenericFnSig, GenericFnTable, GenericShapeTable,
        MonoSignature, MonomorphizationTable, Substitution,
    },
    intrinsics::PrimitiveIntrinsicTable,
    options_table::{collect_options, OptionsTable},
    return_paths::analyze_return_paths,
    scope::{Scope, ScopeEntry},
    shapes::ShapeTable,
    signatures::SignatureTable,
    suspension_source::is_base_suspension_intrinsic,
    types::{type_name, Type},
};

/// Inferred ownership for a plain-ident argument at a `background` call site.
///
/// `OwnershipModifier` (from ynz-ast) covers `Share / Lend / Give` — the three
/// modifiers that appear in function signatures.  `background` inference additionally
/// needs `Copy` (the argument is cloned because the caller reads the binding again
/// after the spawn).  Rather than extend the AST enum (which is shared across all
/// compiler passes), we keep this typeck-local enum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BgOwnership {
    /// Transfer ownership to the background task — the caller does not read the
    /// binding again after this spawn.
    Give,
    /// Copy the value into the background task — the caller reads the binding after
    /// the spawn, so the task needs its own independent copy.
    Copy,
    /// v0.3-M4: share the underlying channel with the background task (refcounted alias —
    /// `ynz_channel_share`). Channels are the sanctioned cross-task conduit: BOTH sides must
    /// operate on the SAME bounded buffer (that is the whole point of a channel), so neither
    /// `give` (caller loses its end) nor `copy` (two disconnected buffers) is correct.
    Channel,
}

/// The type-annotated view of a module.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedModule {
    pub module: Module,
    /// Per-expression types keyed by `(span.start, span.end)`.
    ///
    /// Keying by start alone causes collisions when a BinOp's span.start
    /// equals its leftmost child's span.start — the parent overwrites the child.
    /// The full `(start, end)` pair is unique per expression node.
    pub expr_types: std::collections::HashMap<(usize, usize), Type>,
    /// Compiler-inferred ownership for each plain-ident argument at a
    /// `background` call site, keyed by `(arg_span.start, arg_span.end)`.
    ///
    /// Only plain `Expr::Ident` arguments are recorded here — arguments already
    /// written as `x.give` or `x.copy()` are handled by the postfix-op path and
    /// are NOT re-inferred (explicit always wins over inferred).
    ///
    /// Used by `ownership_call_site_hints` to emit the inferred modifier as a
    /// muted-text hint at the call site.
    pub background_arg_inferred_ownership: std::collections::HashMap<(usize, usize), BgOwnership>,
}

/// Run the M5 type checker over all function bodies.
///
/// Returns the typed module, monomorphization table, accumulated diagnostics, and the
/// set of names that were resolved from the signature table or shape table during the
/// check pass. The caller (`check_query`) uses this to detect unused imports.
// Each argument is a distinct, required compiler table (signatures, shapes, generics,
// intrinsics, imported options, imported fn names); they don't bundle into a meaningful
// context type for a single-call-site entry point. Matches the convention used across
// emit.rs / ynz-watch for top-level entry functions.
#[allow(clippy::too_many_arguments)]
pub fn check(
    module: &Module,
    sig_table: &SignatureTable,
    shape_table: &ShapeTable,
    generic_fn_table: &GenericFnTable,
    generic_shape_table: &GenericShapeTable,
    intrinsics: &PrimitiveIntrinsicTable,
    imported_options: &std::collections::HashMap<String, crate::options_table::OptionsEntry>,
) -> (
    TypedModule,
    MonomorphizationTable,
    DiagnosticBucket,
    HashSet<String>,
) {
    let mut diags = DiagnosticBucket::new();
    let mut options_table = collect_options(module, &mut diags);
    // Merge imported options so function bodies can use cross-file options types.
    for (name, entry) in imported_options {
        options_table
            .options
            .entry(name.clone())
            .or_insert_with(|| entry.clone());
    }

    let mut checker = Checker {
        intrinsics,
        sig_table,
        shape_table,
        generic_fn_table,
        generic_shape_table,
        options_table: &options_table,
        expr_types: HashMap::new(),
        diags,
        scope: Scope::new(),
        current_fn_ret: Type::Nothing,
        current_shape: None,
        type_param_scope: HashMap::new(),
        mono_table: MonomorphizationTable::default(),
        maybe_non_none: HashSet::new(),
        union_narrowed: HashMap::new(),
        union_aliases: collect_union_aliases(module, shape_table),
        errors_success_narrowed: HashSet::new(),
        errors_consumed: HashSet::new(),
        current_fn_errors_capable: false,
        referenced_names: HashSet::new(),
        kernel_mode: false,
        inside_wait: false,
        inside_background: false,
        current_fn_suspends: false,
        bg_inferred: HashMap::new(),
        conduit_root_spans: HashSet::new(),
        derivable_conduits: HashSet::new(),
        channel_returning_fns: crate::may_block::collect_channel_returning_fns(module),
    };
    checker.check_module(module);
    let typed = TypedModule {
        module: module.clone(),
        expr_types: checker.expr_types,
        background_arg_inferred_ownership: checker.bg_inferred,
    };
    (
        typed,
        checker.mono_table,
        checker.diags,
        checker.referenced_names,
    )
}

/// Like `check` but with kernel-mode enabled.
///
/// In kernel mode, `wait` and `background` are compile errors because the
/// thread-pool runtime does not run in kernel mode. This function is used in tests;
/// the `--kernel` build mode arrives in a later version.
#[allow(clippy::too_many_arguments)]
pub fn check_with_kernel_mode(
    module: &Module,
    sig_table: &SignatureTable,
    shape_table: &ShapeTable,
    generic_fn_table: &GenericFnTable,
    generic_shape_table: &GenericShapeTable,
    intrinsics: &PrimitiveIntrinsicTable,
    imported_options: &std::collections::HashMap<String, crate::options_table::OptionsEntry>,
) -> (TypedModule, MonomorphizationTable, DiagnosticBucket) {
    let mut diags = DiagnosticBucket::new();
    let mut options_table = collect_options(module, &mut diags);
    for (name, entry) in imported_options {
        options_table
            .options
            .entry(name.clone())
            .or_insert_with(|| entry.clone());
    }
    let mut checker = Checker {
        intrinsics,
        sig_table,
        shape_table,
        generic_fn_table,
        generic_shape_table,
        options_table: &options_table,
        expr_types: HashMap::new(),
        diags,
        scope: Scope::new(),
        current_fn_ret: Type::Nothing,
        current_shape: None,
        type_param_scope: HashMap::new(),
        mono_table: MonomorphizationTable::default(),
        maybe_non_none: HashSet::new(),
        union_narrowed: HashMap::new(),
        union_aliases: collect_union_aliases(module, shape_table),
        errors_success_narrowed: HashSet::new(),
        errors_consumed: HashSet::new(),
        current_fn_errors_capable: false,
        referenced_names: HashSet::new(),
        kernel_mode: true,
        inside_wait: false,
        inside_background: false,
        current_fn_suspends: false,
        bg_inferred: HashMap::new(),
        conduit_root_spans: HashSet::new(),
        derivable_conduits: HashSet::new(),
        channel_returning_fns: crate::may_block::collect_channel_returning_fns(module),
    };
    checker.check_module(module);
    let typed = TypedModule {
        module: module.clone(),
        expr_types: checker.expr_types,
        background_arg_inferred_ownership: checker.bg_inferred,
    };
    (typed, checker.mono_table, checker.diags)
}

/// Build the `wait_on_non_may_block` warning (registry `[[diagnostic_template]]` kind_name =
/// `"WaitOnNonMayBlockWarning"`) — explicit `wait` on a callee that never suspends.
///
/// Single-sourced here (v0.3-M3g Phase 1) because the two call sites that need it — the
/// CPU-only-intrinsic arm and the transitive-user-fn arm in `check_call` — previously each
/// hand-wrote their own WHAT/WHY text, and the two had drifted to three-way-different wording
/// (this exact hand-duplication pattern caused a v0.3-M3d gate block; the fix here prevents it
/// recurring now that `wait` legality on a CPU-group host is about to change in Phase 2/3).
/// `what_instead_call` is the callee-invocation text each call site renders for its own reason
/// (an intrinsic call site has no accessible argument list at this point, so it renders a plain
/// `...`; the user-fn call site renders the callee's formal parameter names) — kept as a
/// parameter rather than folded into this function so unifying WHAT/WHY does not also force an
/// unrelated behavior change in WHAT-INSTEAD's rendering.
fn wait_on_non_may_block_warning(
    span: SourceSpan,
    callee_name: &str,
    what_instead_call: &str,
) -> Diagnostic {
    Diagnostic::warning(
        span,
        "`wait` on a function that does not suspend — the `wait` has no effect.",
        format!("Remove the `wait` keyword — call `{what_instead_call}` directly."),
        format!(
            "`wait` only has effect when the awaited expression can suspend (calls a may-block \
             intrinsic or another function whose body contains `wait`). Calling a purely \
             CPU-bound callee with explicit `wait` is an ordering barrier — it ends any parallel \
             group that `{callee_name}` could have joined — so removing `wait` lets the compiler \
             overlap this call with adjacent independent calls when that is safe."
        ),
    )
}

struct Checker<'b> {
    // ── Borrowed look-up tables (module-wide, read-only during check) ─────────
    //
    // These tables are built by the signature pre-pass and imported symbol
    // resolution before `check_module` runs.  They never change during a single
    // check pass.
    intrinsics: &'b PrimitiveIntrinsicTable,
    sig_table: &'b SignatureTable,
    shape_table: &'b ShapeTable,
    generic_fn_table: &'b GenericFnTable,
    generic_shape_table: &'b GenericShapeTable,
    options_table: &'b OptionsTable,

    // ── Mutable module-level output ───────────────────────────────────────────
    //
    // These accumulate results across the entire module; they are NOT reset
    // between functions.
    expr_types: HashMap<(usize, usize), Type>,
    diags: DiagnosticBucket,
    /// Accumulated monomorphization entries for all generic call sites in this module.
    mono_table: MonomorphizationTable,
    /// M6: named union type aliases from `shape Shape = Circle | Square` declarations.
    /// Maps alias name → resolved union type. Populated before check_module runs.
    union_aliases: HashMap<String, Type>,

    // ── Per-function mutable state ────────────────────────────────────────────
    //
    // Reset at the start of each function body (see `check_function`).
    scope: Scope,
    /// Return type of the function currently being checked.
    current_fn_ret: Type,
    /// Name of the shape whose method we're currently checking (for `self`/`Self` resolution
    /// and hidden-field visibility). `None` when checking a free function.
    current_shape: Option<String>,
    /// Type parameters in scope for the function currently being checked.
    /// Maps type-param name → unit; presence means the name resolves to `TypeParam`.
    type_param_scope: HashMap<String, ()>,
    /// Whether the function currently being checked is itself errors_capable.
    current_fn_errors_capable: bool,
    /// True when building for --kernel mode. `wait` and `background` are rejected
    /// at compile time because the thread-pool runtime does not run in kernel mode.
    /// Defaults to false; set to true only via `check_with_kernel_mode`.
    kernel_mode: bool,

    // ── Flow-sensitive sets (reset per function; persist through if/else branches) ──
    //
    // `maybe_non_none` and `union_narrowed` are intentionally NOT reset between
    // functions — they track narrowing that may span nested scope levels within a
    // single function and are cleared at the start of each new function by
    // `check_function`.  `errors_success_narrowed` and `errors_consumed` are
    // explicitly cleared in `check_function` (see lines ~165-166).
    //
    // Summary of reset points:
    //   errors_success_narrowed — cleared at the start of each function
    //   errors_consumed          — cleared at the start of each function
    //   maybe_non_none           — managed per scope-push/pop inside if-guards
    //   union_narrowed           — managed per is-arm scope
    /// Flow-sensitive tracking: binding names known to be non-none inside an `.exists()` guard.
    maybe_non_none: HashSet<String>,
    /// M6: binding names narrowed to a specific union variant inside an `is`-arm body.
    /// Maps binding name → narrowed type (the specific variant type).
    union_narrowed: HashMap<String, Type>,
    /// Flow-sensitive: binding names known to be in the success state after a
    /// `.failed() == false` check or after auto-propagation fired. These bindings
    /// have narrowed from `ErrorsCapable<T>` to `T`.
    errors_success_narrowed: HashSet<String>,
    /// Bindings that have been consumed by auto-propagation at first use or by
    /// a `.failed()` check. After consumption, calling `.failed()` is a compile
    /// error ("check-after-use").
    errors_consumed: HashSet<String>,
    /// Names that were actually resolved via the signature table or shape table
    /// during this check pass. Used by `check_query` to detect unused imports —
    /// any imported name absent from this set after the pass was never referenced.
    referenced_names: HashSet<String>,

    // ── v0.3-M2 wait-context flags ────────────────────────────────────────────
    //
    // Set to true while recursing inside `Expr::Wait` or `Expr::Background`
    // respectively. Used by call-site checks to distinguish:
    //   - `wait fn()` (inside_wait=true) → correct usage
    //   - `fn()` bare (both false) → known-safe (analysis drove the decision)
    //   - `background fn()` (inside_background=true) → graph cut, not propagated
    //
    // (true, true) is unreachable per parser: `background` is statement-position only;
    // `wait background X()` fails at parse time before typeck runs.
    //
    // Reset to false at the start of each expression; set to true only while the
    // direct inner of the corresponding wrapper is being checked.
    inside_wait: bool,
    inside_background: bool,
    /// True when the function currently being checked has `suspends == true` (transitive
    /// may-block analysis result). Set at the start of each function by `check_function`.
    ///
    /// The can't-infer diagnostic gates on this field: a caller that independently suspends
    /// (reaches an intra-unit `sleep`) AND makes an unanalyzable cross-module or
    /// dynamic-dispatch call gets the clean compile error. A caller that does NOT
    /// independently suspend treats the boundary call as a non-suspending leaf — the
    /// M2-documented under-approximation per `design/no-function-coloring.md:61-67`
    /// (cross-module suspension propagation requires M8 binary package metadata and
    /// ships in M3).
    current_fn_suspends: bool,

    /// Inferred ownership modifier for each plain-ident argument at a `background` call site.
    ///
    /// Populated during `check_stmts` when a `Stmt::Expr(Expr::Background(...))` is
    /// encountered. The key is `(arg_span.start, arg_span.end)`. Accumulates across all
    /// function bodies in the module. Moved into `TypedModule` when the check pass completes.
    ///
    /// Only plain `Expr::Ident` args are recorded — explicit `.give`/`.copy()` postfix
    /// args are handled by the postfix-op path; explicit always wins over inferred.
    bg_inferred: HashMap<(usize, usize), BgOwnership>,

    /// v0.3-M4 Phase 2: spans at which a suspending conduit-method call (`ch.send(v)`,
    /// `ch.receive()`, `h.send(v)`, `h.receive()`) is allowed to appear — the ROOT of a
    /// bare expression statement or a `let` binding's value (optionally under an explicit
    /// `wait`). Set per statement by `check_stmts`/`check_let`; a conduit-method call whose
    /// span is not in this set is in nested-expression position and gets the bind-it-first
    /// teaching error (mirrors the existing sub-expression suspending-call discipline).
    conduit_root_spans: HashSet<(usize, usize)>,

    /// v0.3-M4 conduit-origin discipline: names of bindings in the CURRENT function whose
    /// conduit-ness (channel / background task handle) is syntactically derivable by the
    /// may-block resolver. Seeded from channel-typed params by `check_function` /
    /// `check_generic_function_body`, accumulated in statement order by `check_let` via
    /// THE shared predicate `may_block::let_binds_derivable_conduit` — never a second,
    /// hand-kept-in-sync derivation (authoritative-derivation.md).
    ///
    /// A flat per-function set (no scope popping), exactly matching the resolver's
    /// accumulation, so the two views stay equal on every program typeck accepts.
    /// `check_conduit_method_call` rejects a suspending `.send()`/`.receive()` whose
    /// receiver is not in this set — that rejection is what makes the resolver's
    /// "can never under-approximate what typeck accepted" invariant
    /// (suspension_source.rs) actually hold.
    derivable_conduits: HashSet<String>,

    /// Local functions whose declared return type is `channel<T>` — the SAME collection
    /// `may_block::build_call_graph` uses, via `may_block::collect_channel_returning_fns`.
    channel_returning_fns: HashSet<String>,
}

impl<'b> Checker<'b> {
    fn check_module(&mut self, module: &Module) {
        // `main` existence and signature are validated in `collect_signatures`.
        // Body checking just iterates all functions with the signature table available.
        // P3b: verify follows contracts after both tables are available.
        self.check_follows_contracts();
        for item in &module.items {
            match item {
                Item::Function(f) => self.check_function(f),
                Item::ShapeDecl(s) => {
                    // Walk the shape declaration to record imported names that are used
                    // only in structural positions (field types, extends parent). The
                    // check pass never enters ShapeDecl bodies — shapes are pre-resolved
                    // in shapes.rs — so without this walk those imports were invisible
                    // to referenced_names and incorrectly flagged as unused.
                    //
                    // `follows` contract names are recorded in check_follows_contracts
                    // (the single chokepoint for shapes that have contracts). `extends`
                    // is handled here because check_follows_contracts only visits shapes
                    // with non-empty follows lists.
                    if let Some((parent_name, _)) = &s.extends {
                        self.referenced_names.insert(parent_name.clone());
                    }
                    // Collect type-param names so the walker can skip them. A field
                    // typed `T` in `shape Box<T> { value: T }` is a local placeholder,
                    // not an imported name — walking it with ast_type_to_type would emit
                    // a spurious "T is not a known type" diagnostic. The diagnostic-free
                    // walker skips these names while still tracking concrete imported
                    // types used alongside type params (e.g. `meta: ImportedMeta`).
                    let type_params: HashSet<String> =
                        s.generics.iter().map(|g| g.name.clone()).collect();
                    for field in &s.fields {
                        self.collect_referenced_names_in_ast_type(&field.ty, &type_params);
                    }
                    // Union type alias RHS: `shape PghEvent = SouthSideEvent | StripeDistrictEvent`.
                    // alias_ty is the raw AstType (Union) written in source. Because the check
                    // pass never enters ShapeDecl bodies, these member names are otherwise
                    // invisible to referenced_names — the same gap the field-type walk above
                    // closes for regular shape fields.
                    if let Some(alias) = &s.alias_ty {
                        self.collect_referenced_names_in_ast_type(alias, &type_params);
                    }
                }
                // M6: options declarations are validated and registered by collect_options()
                // which runs before check_module. Nothing to do here.
                Item::OptionsDecl(_) => {}
                Item::ConstDecl(c) => {
                    // Walk the const declaration's type annotation and initializer so
                    // that imports referenced only in module-level const positions are
                    // recorded in referenced_names. Phase 0 tracks reference presence
                    // only — full type-checking of const bodies is out of scope here
                    // (infer_expr emits diagnostics and is reserved for function bodies).
                    // The diagnostic-free walkers below emit zero diagnostics; they only
                    // insert names into referenced_names so the unused-import pass does
                    // not false-positive on a genuinely-used import.
                    let no_type_params: HashSet<String> = HashSet::new();
                    if let Some(ty) = &c.ty {
                        self.collect_referenced_names_in_ast_type(ty, &no_type_params);
                    }
                    self.collect_referenced_names_in_expr(&c.value);
                }
                // M8: import/export declarations — validated by collect_exports/imports
                // which runs before check_module. Function-body typeck is unaffected.
                Item::ImportDecl(_) | Item::ReExport(_) => {}
            }
        }
        lint_repeated_inline_shapes(module, &mut self.diags);
    }

    fn check_function(&mut self, f: &FunctionDecl) {
        if f.return_type == AstType::Error || body_has_error_node(&f.body.stmts) {
            return;
        }

        if !f.generics.is_empty() {
            self.check_generic_function_body(f);
            return;
        }

        // ast_type_to_type resolves ErrorCapable → ErrorsCapable { inner } already.
        let ret_ty = self.ast_type_to_type(&f.return_type);
        self.current_fn_ret = ret_ty.clone();

        // M7 P3a: track whether the current function is errors-capable.
        self.current_fn_errors_capable = f.errors_capable;
        self.errors_success_narrowed.clear();
        self.errors_consumed.clear();
        // Track whether the caller transitively suspends (analysis result). The can't-infer
        // diagnostic gates on this: a function that independently reaches a suspension point
        // (intra-unit `sleep`) AND makes an unanalyzable boundary call gets the error.
        // Functions that do NOT independently suspend treat the boundary call as a
        // non-suspending leaf — the M2 under-approximation per design/no-function-coloring.md:75.
        self.current_fn_suspends = self
            .sig_table
            .fns
            .get(&f.name)
            .map(|s| s.suspends)
            .unwrap_or(false);

        self.scope.push();

        // v0.3-M4 conduit-origin discipline: seed this function's derivable-conduit set from
        // its channel-typed params — the SAME seeding the may-block resolver performs
        // (`fn_contains_conduit_suspension`), via the shared `ast_type_is_channel`.
        self.derivable_conduits.clear();
        for param in &f.params {
            if crate::may_block::ast_type_is_channel(&param.ty) {
                self.derivable_conduits.insert(param.name.clone());
            }
        }

        // Register parameters. If the first param is named `self` and has a Shape type,
        // record the enclosing shape for hidden-field visibility and Self resolution.
        self.current_shape = None;
        for (i, param) in f.params.iter().enumerate() {
            let param_ty = self.ast_type_to_type(&param.ty);
            if i == 0 && param.name == "self" {
                if let Type::Shape { name } = &param_ty {
                    self.current_shape = Some(name.clone());
                }
            }
            self.scope.insert(
                param.name.clone(),
                ScopeEntry {
                    ty: param_ty,
                    is_const: false,
                    is_param: true,
                    param_ownership: param.ownership.clone(),
                    is_loop_var: false,
                    is_consumed: false,
                    defined_at: param.name_span.clone(),
                },
            );
        }

        // Option-B deferral checks: emit clean teaching errors for wait patterns
        // that require the M3 coroutine-locals transform rather than silently
        // no-oping (loop case) or crashing the backend (local-crossing case).
        //
        // Both checks apply whenever the function contains ANY suspension point —
        // explicit (`wait expr` tokens) OR inferred (a bare call to a suspending
        // function, which M2 codegen also lowers as a state-machine step).
        // Without the inferred-suspension arm, a local that crosses a bare
        // `sleeper()` call (no `wait` keyword) slips past typeck and reaches LLVM
        // codegen where the composed frame doesn't back it → SSA dominance failure.
        let has_explicit_waits = block_contains_wait(&f.body);
        let is_suspending_fn = self.current_fn_suspends;

        // Build the suspending-function set once; reused by checks 2 and 3.
        // Contains all user-defined functions whose `suspends` flag is set by the
        // Phase-6 may-block fixpoint. `is_suspending_call` also folds in the base
        // suspension intrinsics (`sleep` etc., via `is_base_suspension_intrinsic`) which
        // are not in sig_table.
        let suspending_fns: std::collections::HashSet<&str> = if is_suspending_fn {
            self.sig_table
                .fns
                .iter()
                .filter_map(|(n, s)| if s.suspends { Some(n.as_str()) } else { None })
                .collect()
        } else {
            std::collections::HashSet::new()
        };

        // `wait` inside `for`/`while`/`match` is a supported, safe position.
        // Frame-backed loop state carries the loop counter and loop-carried locals across
        // each suspension (one ynz_alloc per task tree, sequential iterations), so no
        // positional guard is needed for loop-body waits. Only the checks below (wide-value
        // return; array-shape-runtime-field) remain active.

        // Check WideValueSuspendingReturn: a suspending function whose return type is a
        // wide-inner value that the SM return path cannot correctly handle without a dedicated
        // frame staging slot.
        //
        // Two classes remain rejected:
        //   • `-> Shape errors`   — EC success path stores a shape pointer that points into
        //                           the resume fn's stack frame; staging at FRAME_OFFSET_LOCALS_START
        //                           (offset 32) clobbers child sub-frames. Needs variable-size
        //                           staging (shape size varies) and is also entangled with the
        //                           pre-existing non-suspending shape-return base bug.
        //   • `-> Shape`          — Bare shape return stages bytes at FRAME_OFFSET_LOCALS_START+0
        //                           (offset 32), which is where child sub-frames begin. Writing
        //                           there clobbers the child frame → SIGSEGV at resume.
        //
        // `-> number errors` (decimal128 EC) is NOT rejected: its 16-byte staging slot is
        // allocated at a fixed offset in the composed frame (after own-local slots, before
        // child sub-frames) by build_frame_layouts. The slot lives inside the single composed
        // frame allocation — alloc=1/free=1 invariant preserved.
        //
        // The guard must NOT reject:
        //   `-> number errors`, `-> int errors`, `-> float`, `-> number`, `-> bool`,
        //   `-> string`, `-> array`, `-> map` from suspending functions — all verified clean.
        //
        // Scoped to RETURN TYPE only. Shapes as CROSSING LOCALS still work correctly
        // (frame-embedded via the crossing-local slot machinery, not the return-slot path).
        if !self.kernel_mode && is_suspending_fn {
            let is_wide_return = match &ret_ty {
                // `-> Shape` bare: crashing staging at child-sub-frame region.
                Type::Shape { .. } => true,
                // `-> Shape errors` (shape pointer into stack frame, variable-size staging).
                // `-> number errors` is explicitly NOT rejected — it has a correct implementation.
                Type::ErrorsCapable { inner } => {
                    matches!(inner.as_ref(), Type::Shape { .. })
                }
                _ => false,
            };
            if is_wide_return {
                let (what_instead, type_label) = match &ret_ty {
                    Type::Shape { .. } => {
                        let rendered = type_name(&ret_ty);
                        (
                            format!(
                                "Return the shape's fields individually as primitives, or \
                                 bind `{rendered}` to a crossing local and return a primitive derived from it."
                            ),
                            format!("`{rendered}`"),
                        )
                    }
                    Type::ErrorsCapable { inner } => match inner.as_ref() {
                        Type::Shape { .. } => {
                            let rendered = type_name(inner.as_ref());
                            (
                                "Return the shape's fields individually (e.g. `-> int errors`), \
                                 or compute a primitive result inside the function and return that."
                                    .to_string(),
                                format!("`{rendered} errors`"),
                            )
                        }
                        _ => ("".to_string(), "this type".to_string()),
                    },
                    _ => ("".to_string(), "this type".to_string()),
                };
                self.diags.push(Diagnostic::error(
                    f.span.clone(),
                    format!("A suspending function cannot yet return {type_label} by value."),
                    what_instead,
                    "Returning a shape value from a suspended function needs a variable-size \
                     frame staging slot entangled with the shape-return base fix — that work is \
                     deferred. See design/concurrency.md 'WideValueSuspendingReturn'.",
                ));
            }
        }

        // Check 3: suspending call in a sub-expression position.
        //
        // The codegen compiles each suspending call as its own state-machine step. The
        // three supported direct-statement forms are: `foo()`, `let x = foo()`, and
        // `return foo()` (with or without `wait`). Any suspending call nested deeper —
        // as an operand of `+`/`-`/etc., inside an interpolation `${...}`, as an `if`
        // condition, as an argument to another call, etc. — falls through the codegen
        // switcher to a wrapper path that panics at runtime ("Cannot start a runtime
        // from within a runtime"). Catching it here (typeck) prevents the runtime abort.
        //
        // This guard is permanent (not a temporary M2 limitation): step-by-step style —
        // one operation per line with a named variable — is Yinz's deliberate design
        // (Golden Rule 7). Keeping each suspending call on its own statement also means
        // M3b's auto-parallelization of independent statements works naturally: two
        // `let a = wait fa()` / `let b = wait fb()` lines get parallelized automatically.
        if !self.kernel_mode && is_suspending_fn {
            let violations = suspending_calls_in_subexpr_position(&f.body.stmts, &suspending_fns);
            for (span, callee_name) in violations {
                self.diags.push(Diagnostic::error(
                    span,
                    format!("`{callee_name}` is a suspending call inside a larger expression."),
                    format!(
                        "Give it its own line: `let result = {callee_name}(...)`, \
                         then use `result` in the expression."
                    ),
                    "Yinz compiles each suspending call as its own state-machine step, \
                     and the step-by-step style (one operation per line with a named \
                     variable) is the language's preferred form — it keeps code readable \
                     and enables the compiler to auto-parallelize independent statements.",
                ));
            }
        }

        self.check_stmts(&f.body.stmts);
        self.scope.pop();

        // Checks 1a–1c (run after check_stmts so expr_types is populated — needed for
        // type lookups on identifiers in the for-loop iterator position):

        // Check 1a (StoredRangeWithWait): `let r = range(0,3); for (i in r) { wait }`.
        // The SM range arm in codegen calls `extract_range_bounds(iter)` which requires the
        // iter to be a literal `range(...)` call. A stored range variable reaches a different
        // code path that cannot yet recover the bounds from the frame-backed alloca. Emit a
        // WHAT/WHAT-INSTEAD/WHY error rather than letting codegen ICE.
        // See design/concurrency.md 'StoredRangeWithWait' and registry/features.toml.
        if !self.kernel_mode && has_explicit_waits {
            if let Some(span) = find_stored_range_wait_in_for(&f.body.stmts, &self.expr_types) {
                self.diags.push(Diagnostic::error(
                    span,
                    "a stored range variable cannot yet be the iterator of a `for` loop \
                     that contains a `wait`.",
                    "Inline the range directly in the loop: \
                     `for (i in range(0, n)) { ... }`. \
                     If `n` is a crossing local, it is already frame-backed and the inline \
                     form works without any extra changes.",
                    "The state-machine codegen re-evaluates the iterator expression at each \
                     loop header to reload the range bounds. For a stored range variable the \
                     bounds would need to be read from the range's frame-backed alloca, which \
                     is not yet implemented. See design/concurrency.md 'StoredRangeWithWait'.",
                ));
            }
        }

        // Check 1b (FixedArrayIterWithWait): `for (x in fixed<T>) { wait }`.
        // fixed<T> arrays are stack-allocated in the current resume-function's stack frame.
        // When a `wait` suspends and the resume function returns, the stack frame is freed.
        // The next resume reads the array pointer from the frame slot — a dangling address.
        // See design/concurrency.md 'FixedArrayIterWithWait' and registry/features.toml.
        if !self.kernel_mode && has_explicit_waits {
            if let Some(span) = find_fixed_array_iter_wait_in_for(&f.body.stmts, &self.expr_types) {
                self.diags.push(Diagnostic::error(
                    span,
                    "a `fixed<T>` array cannot be the iterator of a `for` loop that contains a \
                     `wait`.",
                    "Use `array<T>` instead: `let items: array<T> = [...]`. An `array<T>` is \
                     heap-allocated so its pointer survives suspension.",
                    "The elements of `fixed<T>` live on the current resume-function's stack. \
                     When a `wait` suspends and the function returns to the scheduler, that stack \
                     frame is freed. On the next resume the element pointer is stale, producing \
                     undefined behavior. See design/concurrency.md 'FixedArrayIterWithWait'.",
                ));
            }
        }

        // Check 1c (ExpressionIterWithWait): `for (x in makeArray()) { wait }`.
        // A call-expression iterator is re-evaluated by the SM codegen on every loop header
        // visit — once for count check, once for element load. For expressions with side
        // effects, this evaluates N+1 times and breaks the one-alloc-per-task invariant.
        // See design/concurrency.md 'ExpressionIterWithWait' and registry/features.toml.
        if !self.kernel_mode && has_explicit_waits {
            if let Some(span) = find_expr_iter_wait_in_for(&f.body.stmts) {
                self.diags.push(Diagnostic::error(
                    span,
                    "a call-expression iterator cannot yet be used in a `for` loop that \
                     contains a `wait`.",
                    "Bind the collection to a variable first: \
                     `let items = makeArray()` then `for (x in items) { ... }`. \
                     The `items` variable will be frame-backed and survive `wait` correctly.",
                    "The state-machine codegen re-evaluates the iterator expression at each \
                     loop header, which would call the function once per iteration instead of \
                     once total. Binding the collection first ensures it is evaluated exactly \
                     once and the resulting pointer is stored in a stable frame slot. \
                     See design/concurrency.md 'ExpressionIterWithWait'.",
                ));
            }
        }

        // Check 2 (run after check_stmts so expr_types is populated):
        // A crossing local whose shape has a nested-shape field cannot yet cross a `wait`.
        // Shapes with only primitive / heap-stable fields (int, bool, float, number, string,
        // array, map) are frame-embedded inline and work correctly. Shapes with a nested-shape
        // field store an opaque pointer to a stack-allocated struct in their LLVM layout; that
        // pointer becomes invalid after the resume function returns and resumes.
        // Full recursive aggregate frame-embedding ships in a later milestone.
        if !self.kernel_mode && (has_explicit_waits || is_suspending_fn) {
            let param_names_ref: Vec<&str> = f.params.iter().map(|p| p.name.as_str()).collect();
            let crossings = crossing_local_names(
                &f.body.stmts,
                &param_names_ref,
                &suspending_fns,
                &self.expr_types,
            );
            for crossing_name in &crossings {
                // Look up the typeck-resolved type now that expr_types is populated.
                // For let-defined crossing locals, find_crossing_local_typeck_type_in_map
                // returns the RHS expression type. For for-loop vars (no Stmt::Let),
                // it returns None — use find_for_loop_var_type_in_stmts as the fallback
                // so the nested-shape check covers both positions.
                let resolved_ty = find_crossing_local_typeck_type_in_map(
                    &f.body.stmts,
                    crossing_name.as_str(),
                    &self.expr_types,
                )
                .or_else(|| {
                    find_for_loop_var_type_in_stmts(
                        &f.body.stmts,
                        crossing_name.as_str(),
                        &self.expr_types,
                    )
                });
                if let Some(Type::Shape {
                    name: ref shape_name,
                }) = resolved_ty
                {
                    let has_nested_shape = self
                        .shape_table
                        .shapes
                        .get(shape_name.as_str())
                        .is_some_and(|def| {
                            def.fields
                                .iter()
                                .any(|field| matches!(&field.ty, Type::Shape { .. }))
                        });
                    if has_nested_shape {
                        let span = find_crossing_local_span(&f.body.stmts, crossing_name.as_str())
                            .unwrap_or_else(|| f.span.clone());
                        self.diags.push(Diagnostic::error(
                            span,
                            format!(
                                "`{crossing_name}` is a `{shape_name}` value that crosses a \
                                 `wait` — but `{shape_name}` has a nested-shape field, which \
                                 cannot be frame-embedded yet."
                            ),
                            format!(
                                "Restructure `{shape_name}` so all its fields are primitive \
                                 types (int, bool, float, number, string), or flatten the \
                                 nested shape's fields into `{shape_name}` directly."
                            ),
                            "Shapes with nested-shape fields store an internal pointer to a \
                             stack buffer. That pointer becomes invalid after a `wait` suspends \
                             and resumes the function. Full recursive aggregate frame-embedding \
                             ships in a later milestone.",
                        ));
                    }
                }
            }

            // Check 2b (UnsupportedCrossingLocalType): a crossing local whose type cannot
            // yet be correctly frame-backed cannot cross a `wait`.
            //
            // The frame-slot classifier in codegen handles: int, bool, float, number,
            // string, array, map, Shape, ErrorsCapable. Types not in this list either fall
            // into the generic pointer flush/reload path (which calls `ptr_to_int` on the
            // alloca pointer) or have no scalar frame representation at all.
            //
            // Blocked categories and why:
            //   - `union` / `maybe<T>` / `dynamic Contract`: alloca points to a {tag,payload}
            //     struct on the RESUME FUNCTION'S STACK, which is destroyed between suspension
            //     and resume. Reloading the stored address after resume produces UB.
            //   - `fixed<T>` (let binding): fixed arrays are stack-allocated allocas in the
            //     resume function's stack frame. The same dangling-pointer hazard applies.
            //     (For-loop iteration over fixed<T> is caught separately by FixedArrayIterWithWait.)
            //   - MapEntry (for-loop var over a map): map entry vars are NOT in crossing_names
            //     (they are rebound fresh on each body-bb entry). Accessing entry.key/entry.value
            //     after a wait is caught separately by Check 2c (MapEntryFieldAfterWait).
            //
            // We check BOTH the RHS expression type (from expr_types, after check_stmts)
            // AND the resolved annotation type (from the AST `ty` field + union-alias lookup).
            // The annotation type catches union-alias annotations like `let fig: Figure = c`
            // where the RHS resolves to the concrete variant type (e.g. `Circle`), not the
            // union alias type (`Figure`). For for-loop vars (no Stmt::Let annotation), we
            // additionally check the iterator's element type via find_for_loop_var_type_in_stmts.
            // Any unsupported type in any source triggers the guard.
            for crossing_name in &crossings {
                // RHS expression type (catches let-binding inferred types).
                let rhs_ty = find_crossing_local_typeck_type_in_map(
                    &f.body.stmts,
                    crossing_name.as_str(),
                    &self.expr_types,
                );
                // Annotation type, resolved through union aliases (catches explicit annotations).
                let ann_ty =
                    find_let_annotation_type_in_stmts(&f.body.stmts, crossing_name.as_str())
                        .and_then(|ast_ty| self.resolve_type_for_guard(&ast_ty));
                // For-loop variable type: for vars bound by `for (x in iter)`, there is no
                // Stmt::Let so neither rhs_ty nor ann_ty is populated. Derive the element type
                // from the iterator expression via the for-loop scanner.
                let for_var_ty = find_for_loop_var_type_in_stmts(
                    &f.body.stmts,
                    crossing_name.as_str(),
                    &self.expr_types,
                );
                // Pick the first unsupported type from any source — prefer annotation over RHS
                // (it encodes the programmer's intent more precisely for union-aliased vars).
                let effective_ty = [&ann_ty, &rhs_ty, &for_var_ty]
                    .iter()
                    .find_map(|opt| {
                        opt.as_ref().filter(|ty| {
                            matches!(
                                ty,
                                Type::Union { .. }
                                    | Type::Maybe { .. }
                                    | Type::Dynamic { .. }
                                    | Type::BuiltinFixed { .. }
                                    | Type::Range { .. }
                            )
                        })
                    })
                    .cloned();
                if let Some(ty) = effective_ty {
                    let ty_display = type_name(&ty);
                    let (what_instead, why) = match &ty {
                        Type::BuiltinFixed { .. } => (
                            format!(
                                "Declare `{crossing_name}` as `array<T>` instead of `fixed<T>`. \
                                 An `array<T>` is heap-allocated so its pointer survives suspension."
                            ),
                            "fixed<T> arrays are stack-allocated in the resume function's stack \
                             frame. When a `wait` suspends the function and the resume function \
                             returns to the scheduler, that stack frame is freed. On the next \
                             resume, the crossing-local frame slot holds a dangling pointer to the \
                             old stack-allocated array — reading it is undefined behavior. \
                             See design/concurrency.md 'UnsupportedCrossingLocalType'.",
                        ),
                        Type::Range { .. } => (
                            format!(
                                "Inline the range directly in the `for` loop: \
                                 `for ({crossing_name} in range(...))` instead of binding it to \
                                 a `let` first. An inline range expression is reconstructed on \
                                 each resume; a stored range is a stack-allocated value whose \
                                 pointer dangles after suspension. \
                                 See design/concurrency.md 'UnsupportedCrossingLocalType'."
                            ),
                            "A range value is stack-allocated by the codegen; when a `wait` \
                             suspends the function the stack frame is freed, and the crossing-local \
                             frame slot holds a dangling pointer on the next resume. Iterating \
                             that dangling range produces zero iterations (silent wrong output). \
                             See design/concurrency.md 'UnsupportedCrossingLocalType'.",
                        ),
                        _ => (
                            format!(
                                "Extract the inner value before the `wait`, or restructure so \
                                 `{crossing_name}` is not needed after the suspension."
                            ),
                            "The frame-slot save/restore for `union`, `maybe`, and `dynamic` \
                             values is not yet implemented — without it the value would be read \
                             from a stack address that no longer exists after the function resumes. \
                             See design/concurrency.md 'UnsupportedCrossingLocalType'.",
                        ),
                    };
                    let span = find_crossing_local_span(&f.body.stmts, crossing_name.as_str())
                        .unwrap_or_else(|| f.span.clone());
                    self.diags.push(Diagnostic::error(
                        span,
                        format!("a `{ty_display}` value cannot yet cross a `wait`."),
                        what_instead,
                        why,
                    ));
                }
            }

            // Check 2c (MapEntryFieldAfterWait): a `for (entry in map)` loop whose body
            // contains a `wait` and reads `entry.key` or `entry.value` AFTER the wait.
            //
            // The SM map codegen creates a fresh {key, value} entry struct on each body-bb
            // entry from ynz_map_iter_get. When a wait suspends the resume function, the
            // struct alloca lives on the resume function's stack — which is freed when the
            // function returns. On the next resume call, reading entry.key or entry.value
            // through the old stack alloca is a dangling-pointer read (SIGSEGV).
            //
            // The entry loop variable is intentionally NOT added to crossing_names for map
            // loops (it is re-bound fresh on each body-bb entry and needs no frame slot).
            // This guard catches the case where the programmer tries to use entry.* after
            // a wait, which is the only dangerous pattern.
            if let Some(span) = find_map_entry_field_after_wait(&f.body.stmts, &self.expr_types) {
                self.diags.push(Diagnostic::error(
                    span,
                    "a map entry field (`entry.key` or `entry.value`) cannot be read after a \
                     `wait` inside a `for (entry in map)` loop.",
                    "Read `entry.key` and `entry.value` before the `wait` and bind them to \
                     separate `let` bindings — e.g. `let k = entry.key; let v = entry.value` — \
                     then use `k` and `v` after the `wait`. An outer accumulator that does not \
                     read entry fields can cross the `wait` freely.",
                    "A map-iteration loop variable is rebound from the runtime on each \
                     iteration. When a `wait` suspends the function, the entry's key-value data \
                     lives on the resume function's stack, which is freed on suspension. On the \
                     next resume, reading entry fields through the old stack address produces \
                     garbage or a crash. \
                     See design/concurrency.md 'map-entry-fields-after-wait'.",
                ));
            }

            // Check 2d (ArrayShapeRuntimeFieldWithWait): an `array<Shape>` crossing local
            // whose array literal contains at least one struct element with a runtime-computed
            // field value (i.e., not a compile-time integer/bool literal).
            //
            // `array<Shape>` elements are stored as pointers to the shape's LLVM struct alloca.
            // For all-literal elements, the codegen emits LLVM module-level globals (eternal
            // address, stable across suspension). For elements with runtime field values, the
            // codegen falls back to a stack alloca in the constructing function's resume frame —
            // which is freed when the function suspends and returns to the scheduler. On the
            // next resume the element pointers dangle, producing undefined behavior.
            //
            // The interim fix is a clean WHAT/WHAT-INSTEAD/WHY compile error.
            // The permanent fix (by-value element storage) ships in m3c-array-by-value.
            // See design/concurrency.md 'ArrayShapeRuntimeFieldWithWait' and
            // design/future/array-by-value-element-storage.md.
            if let Some((span, crossing_name)) =
                find_array_shape_runtime_field_crossing(&crossings, &f.body.stmts)
            {
                self.diags.push(Diagnostic::error(
                    span,
                    format!(
                        "`{crossing_name}` is an `array<Shape>` whose elements have \
                         runtime-computed field values and cannot yet cross a `wait`."
                    ),
                    "An `array<Shape>` built with computed (non-literal) field values \
                     cannot be used in a function that contains `wait` yet. Two options \
                     that work today:\n\
                     \n\
                     1. Use only plain literal numbers or true/false as field values:\n\
                        let items = [{ id: 1, qty: 10 }]   // all literals — works\n\
                     \n\
                     2. Move the array and all its uses into a separate helper function \
                     that does not contain any `wait`:\n\
                        function buildItems(qty: int) -> array<Item> {\n\
                          return [{ id: 1, qty: qty }]   // no wait here — works\n\
                        }\n\
                     \n\
                     Full support for computed field values in functions that use `wait` \
                     ships soon — see design/concurrency.md 'ArrayShapeRuntimeFieldWithWait'.",
                    "Array elements with computed (non-literal) field values are stored as \
                     references to temporary memory created while the array is built. When the \
                     function pauses at a `wait`, that temporary memory is released — so after \
                     the pause the references point at freed memory and reading them gives wrong \
                     values. Elements whose fields are all simple literal numbers or true/false \
                     are stored in permanent memory and work correctly across a `wait`. Full \
                     support for computed field values across a `wait` ships in a later \
                     milestone. \
                     See design/concurrency.md 'ArrayShapeRuntimeFieldWithWait'.",
                ));
            }

            // Check 3 (shadow detection): a `let` that re-declares a crossing-local name
            // is ambiguous — the same name means two different values across a `wait`. Reject
            // it with a clean WHAT/WHAT-INSTEAD/WHY error.
            //
            // Codegen keys frame slots by NAME: one alloca per name, pre-created in sm_entry.
            // Two bindings with the same name around a suspension share ONE slot — the later
            // write clobbers the earlier value, producing a silent wrong answer. Rejecting is
            // the correct design: two bindings with the same name across a suspension is
            // confusing (Golden Rule 2) AND currently unrepresentable in the frame-slot layout.
            //
            // Two distinct collision shapes are caught here:
            //   (a) Nested shadow: outer `let x` before suspension + inner `let x` in a nested
            //       block (if/while/for/match body) — shadowing inside a nested scope means
            //       codegen generates two writes to the same name-keyed slot.
            //   (b) Top-level redeclaration: outer `let x` before suspension + another `let x`
            //       at the TOP LEVEL of the function body after the suspension — same slot,
            //       different values, guaranteed clobber at the assignment point.
            //
            // Only apply this check when the crossing local has a TOP-LEVEL outer `let`
            // declaration in the function body (one not nested inside any if/while/for/match).
            // An inner-only crossing local (declared solely inside a nested block) cannot be
            // shadowed by a later outer `let` with the same name — the outer `let` is not a
            // crossing local, so there is no alloca ambiguity.
            for crossing_name in &crossings {
                let name_str = crossing_name.as_str();
                if !outer_is_genuine_crossing_local(
                    &f.body.stmts,
                    name_str,
                    &suspending_fns,
                    &self.expr_types,
                ) {
                    // The outer `let target` either doesn't exist, appears after the first
                    // suspension, or has no reads/redeclarations after a top-level suspension
                    // attributable to the outer binding. Shadow detection must not fire — there
                    // is no outer crossing local to protect.
                    continue;
                }
                // Shape (a): nested shadow — inner `let name` inside a nested block.
                if find_shadow_in_stmts(&f.body.stmts, name_str) {
                    let span = find_crossing_local_span(&f.body.stmts, name_str)
                        .unwrap_or_else(|| f.span.clone());
                    self.diags.push(Diagnostic::error(
                        span,
                        format!(
                            "`{crossing_name}` is declared again inside a nested scope, but the \
                             outer `{crossing_name}` crosses a `wait`."
                        ),
                        format!(
                            "Rename the inner binding to something distinct (e.g., \
                             `let inner_{crossing_name} = ...`) so the two values are \
                             unambiguously named across the suspension boundary."
                        ),
                        "Across a `wait`, one name must mean one value. A shadowing `let` inside \
                         a nested scope creates a second binding with the same name — the compiler \
                         cannot tell which value should survive the suspension.",
                    ));
                }
                // Shape (b): top-level redeclaration after suspension — a second `let name`
                // at the top level of the function body, after a suspension point. Both the
                // pre-wait outer binding and the post-wait redeclaration share the same
                // name-keyed frame slot; the redeclaration clobbers the outer value.
                if has_top_level_let_after_suspension(
                    &f.body.stmts,
                    name_str,
                    &suspending_fns,
                    &self.expr_types,
                ) {
                    let span = find_crossing_local_span(&f.body.stmts, name_str)
                        .unwrap_or_else(|| f.span.clone());
                    self.diags.push(Diagnostic::error(
                        span,
                        format!(
                            "`{crossing_name}` is declared before a `wait` and then declared \
                             again at the top level after the `wait`."
                        ),
                        format!(
                            "Rename the second binding to something distinct (e.g., \
                             `let {crossing_name}_after = ...`) so the two values are \
                             unambiguously named across the suspension boundary."
                        ),
                        "Across a `wait`, one name must mean one value. Two top-level `let` \
                         declarations with the same name — one before and one after a `wait` — \
                         share the same frame slot. The second declaration overwrites the first, \
                         producing a silent wrong answer.",
                    ));
                }
            }

            // Check 3b (parameter-shadow detection): a `let` that re-declares a PARAMETER
            // name triggers the same alloca-collision as the let-vs-let case above — parameters
            // are frame-slotted at function entry, so their name occupies a slot from the
            // moment the function is entered. The frame-slot system keys slots by name; a
            // nested `let pname` shares that same slot, and the codegen cannot hold two
            // distinct values under one name simultaneously.
            //
            // Two collision shapes for parameters:
            //   (a) Nested shadow: a `let param_name` inside any nested block (if/while/
            //       for/match body) shares the parameter's name-keyed frame slot. Even a
            //       shadow that does NOT itself cross a `wait` is unsafe: the Part-A
            //       entry-block alloca path would place the inner alloca in sm_entry and
            //       reload_params_from_frame in each continuation state would overwrite
            //       cg.locals[pname] with the inner alloca — corrupting the parameter across
            //       any subsequent suspension. Per design/concurrency.md § ShadowsCrossingLocal
            //       (M3c roadmap), same-name shadows in async functions are rejected until
            //       per-binding-ID slot allocation lands (1–2 sessions, tracked in M3c plan).
            //       Workaround: use a distinct name for the inner binding.
            //   (b) Top-level redeclaration: `let param_name` at the TOP LEVEL of the
            //       function body — same slot as the parameter, guaranteed clobber.
            for param in &f.params {
                let pname = param.name.as_str();
                // Shape (a): any nested `let pname` in any block body. The conservative
                // reject covers all nested shadows regardless of whether the inner binding
                // itself crosses a suspension — the name-keyed frame slot is shared, and
                // the reload path in continuation states cannot distinguish inner from outer.
                // See design/concurrency.md § ShadowsCrossingLocal for the per-binding-ID
                // lifting path (roadmap M3c).
                if param_has_nested_let_shadow(&f.body.stmts, pname) {
                    self.diags.push(Diagnostic::error(
                        param.span.clone(),
                        format!(
                            "`{pname}` is already bound in this function, which suspends at a \
                             `wait`. Re-using the name `{pname}` in a nested scope would share \
                             one frame slot across the suspension."
                        ),
                        format!(
                            "Rename the inner binding to something distinct (e.g., \
                             `let inner_{pname} = ...`). (Full same-name support across a \
                             suspension is tracked — see design/concurrency.md \
                             ShadowsCrossingLocal.)"
                        ),
                        "In a function that suspends at a `wait`, every name maps to one frame \
                         slot. Two bindings sharing a name across a suspension boundary share \
                         that slot — the compiler cannot hold both values simultaneously under \
                         one name.",
                    ));
                }
                // Shape (b): top-level redeclaration of parameter — a `let param_name` at
                // the top level of the function body shares the parameter's frame slot and
                // clobbers it, regardless of whether any read resolves to the parameter or
                // the redeclaration. Only applicable when the function has a suspension
                // (otherwise the parameter is not frame-slotted at all).
                if first_top_level_suspension_idx(&f.body.stmts, &suspending_fns, &self.expr_types)
                    .is_some()
                    && has_top_level_let_in_stmts(&f.body.stmts, pname)
                {
                    self.diags.push(Diagnostic::error(
                        param.span.clone(),
                        format!(
                            "`{pname}` is a parameter that is declared again at the top level \
                             of the function, but `{pname}` crosses a `wait`."
                        ),
                        format!(
                            "Rename the top-level binding to something distinct (e.g., \
                             `let {pname}_val = ...`) so the parameter and the local value \
                             are unambiguously named across the suspension boundary."
                        ),
                        "Across a `wait`, one name must mean one value. A top-level `let` that \
                         re-declares a parameter name shares the parameter's frame slot — the \
                         declaration overwrites the parameter value, producing a silent wrong \
                         answer.",
                    ));
                }
            }
        }

        // Return-path analysis for non-nothing functions.
        // For ErrorsCapable functions, report the inner type name (not "string errors")
        // so the error message reads naturally. Also skip analysis for -> nothing errors
        // (inner is Nothing) since implicit fallthrough is valid for nothing-returning fns.
        let ret_ty_for_analysis = match &ret_ty {
            Type::ErrorsCapable { inner } => *inner.clone(),
            other => other.clone(),
        };
        if ret_ty != Type::Nothing && ret_ty != Type::Error && ret_ty_for_analysis != Type::Nothing
        {
            let analysis = analyze_return_paths(&f.body);
            if !analysis.all_paths_return {
                self.diags.push(Diagnostic::error(
                    f.span.clone(),
                    format!(
                        "`{}` must return a `{}` on every path, but some paths fall off the end without returning.",
                        f.name,
                        type_name(&ret_ty_for_analysis)
                    ),
                    "Add `return value` at the end of the function, or add an `else =>` default arm to any multi-case `if` that needs to return.",
                    "Every path through the function must produce a value. A path that falls off the end produces no value, which is a bug.",
                ));
            }
            for dead_span in analysis.dead_code {
                self.diags.push(Diagnostic::warning(
                    dead_span,
                    "This code will never run.",
                    "Remove the unreachable code, or move the `return` statement after it.",
                    "A `return` statement ends the function immediately. Any code after it in the same block is never reached.",
                ));
            }
        }
    }

    /// Type-check the body of a generic function under its type-parameter scope.
    ///
    /// Generic bodies are checked with TypeParam types in scope. No return-path
    /// analysis at P3a — the generic signature is trusted; codegen verifies per-instantiation.
    fn check_generic_function_body(&mut self, f: &FunctionDecl) {
        // Push type param names into the type_param_scope.
        for gp in &f.generics {
            self.type_param_scope.insert(gp.name.clone(), ());
        }

        let ret_ty = self.ast_type_to_type(&f.return_type);
        self.current_fn_ret = ret_ty;
        self.current_shape = None;

        self.scope.push();
        // v0.3-M4 conduit-origin discipline: same per-function seeding as `check_function`.
        self.derivable_conduits.clear();
        for param in &f.params {
            if crate::may_block::ast_type_is_channel(&param.ty) {
                self.derivable_conduits.insert(param.name.clone());
            }
        }
        for param in &f.params {
            let param_ty = self.ast_type_to_type(&param.ty);
            self.scope.insert(
                param.name.clone(),
                ScopeEntry {
                    ty: param_ty,
                    is_const: false,
                    is_param: true,
                    param_ownership: param.ownership.clone(),
                    is_loop_var: false,
                    is_consumed: false,
                    defined_at: param.name_span.clone(),
                },
            );
        }
        self.check_stmts(&f.body.stmts);
        self.scope.pop();

        // Clear type params when done with this function.
        for gp in &f.generics {
            self.type_param_scope.remove(&gp.name);
        }
    }

    fn check_stmts(&mut self, stmts: &[Stmt]) {
        // Collect early-return narrowing facts: when an `if (!m.exists()) { return }` or
        // `if (!m.exists()) { panic(...) }` is detected, mark `m` as non-none for all
        // subsequent statements in this block.
        let mut early_return_narrowed: Vec<String> = Vec::new();

        // Indexed iteration so that `background` inference can look at stmts[i+1..].
        for (i, stmt) in stmts.iter().enumerate() {
            // Apply any early-return narrowing facts from previous `if (!x.exists()) { return }`.
            for name in &early_return_narrowed {
                self.maybe_non_none.insert(name.clone());
            }

            match stmt {
                Stmt::Expr(expr) => {
                    // v0.3-M4 Phase 2: record the conduit-method root span for this statement
                    // (bare `ch.send(v)` / `ch.receive()` statements are the allowed position).
                    self.record_conduit_root(expr);
                    // Give/copy inference for `background fn(x)` plain-ident arguments.
                    //
                    // Safe direction: if we cannot prove the binding is dead after the spawn,
                    // infer `.copy` (caller keeps original, task gets its own copy). Only infer
                    // `.give` when we can prove the binding is NOT read in any remaining statement.
                    if let Expr::Background(inner, _) = expr {
                        if let Expr::Call(call) = inner.as_ref() {
                            let remaining = &stmts[i + 1..];
                            // Only infer for plain Expr::Ident args — explicit .give/.copy()
                            // postfix args are handled by the postfix-op path; explicit wins.
                            let mut gives: Vec<String> = Vec::new();
                            for arg in &call.args {
                                if let Expr::Ident(name, span) = arg {
                                    // v0.3-M4: a channel argument is SHARED with the task
                                    // (refcounted alias) — both sides must operate on the
                                    // same bounded buffer; neither give nor copy is correct.
                                    let is_channel = self.scope.lookup(name).is_some_and(|e| {
                                        matches!(e.ty, Type::BuiltinChannel { .. })
                                    });
                                    if is_channel {
                                        self.bg_inferred
                                            .insert((span.start, span.end), BgOwnership::Channel);
                                        continue;
                                    }
                                    let used_after = remaining
                                        .iter()
                                        .any(|s| ident_read_in_stmt(s, name.as_str()));
                                    let inferred = if used_after {
                                        BgOwnership::Copy
                                    } else {
                                        BgOwnership::Give
                                    };
                                    self.bg_inferred
                                        .insert((span.start, span.end), inferred.clone());
                                    if inferred == BgOwnership::Give {
                                        gives.push(name.clone());
                                    }
                                }
                            }
                            // Infer_expr runs first (for diagnostics / type registration),
                            // then we consume the .give bindings so any subsequent stmt
                            // that reads the binding triggers the use-after-give error.
                            self.infer_expr(expr, None);
                            for name in &gives {
                                // `ident_read_in_stmt` does not distinguish `const` from `let`,
                                // so a `const` binding not read after spawn would reach this
                                // path. Consuming a `const` would incorrectly block re-reads in
                                // later statements (const values are always live). The
                                // `!entry.is_const` guard is intentionally unreachable via the
                                // give-inference liveness walk for any well-typed program —
                                // it exists as a conservative backstop for future liveness
                                // changes that might re-derive give candidates without the
                                // const distinction.
                                if let Some(entry) = self.scope.lookup(name.as_str()) {
                                    if !entry.is_const && !entry.is_consumed {
                                        self.scope.consume(name.as_str());
                                    }
                                }
                            }
                            continue;
                        }
                    }
                    self.infer_expr(expr, None);
                }
                Stmt::Let {
                    is_const,
                    name,
                    name_span,
                    ty,
                    value,
                    span: _,
                } => {
                    self.check_let(*is_const, name, name_span, ty.as_ref(), value);
                }
                Stmt::Assign {
                    target,
                    target_span,
                    value,
                    span: _,
                } => {
                    // Reassignment invalidates early-return narrowing for the target binding.
                    early_return_narrowed.retain(|n| n != target);
                    self.check_assign(target, target_span, value);
                }
                Stmt::If { cond, body, .. } => {
                    // Detect early-return narrowing: `if (!m.exists()) { <recognized-exit> }`.
                    // After this if, `m` is proven to exist for the rest of the block.
                    let negated_exists = self.extract_negated_exists_binding(cond);
                    let body_always_exits = analyze_return_paths(body).all_paths_return;
                    if !negated_exists.is_empty() && body_always_exits {
                        for name in &negated_exists {
                            early_return_narrowed.push(name.clone());
                        }
                    }
                    self.check_stmt_if(cond, body);
                }
                Stmt::Match {
                    scrutinee,
                    arms,
                    else_arm,
                    ..
                } => {
                    self.check_stmt_match(scrutinee, arms, else_arm.as_ref());
                }
                Stmt::While { cond, body, .. } => {
                    self.check_stmt_while(cond, body);
                }
                Stmt::For {
                    var,
                    var_span,
                    iter,
                    body,
                    ..
                } => {
                    self.check_stmt_for(var, var_span, iter, body);
                }
                Stmt::Return { value, span } => {
                    self.check_stmt_return(value.as_ref(), span);
                }
                Stmt::FieldAssign {
                    target,
                    value,
                    span,
                } => {
                    self.check_field_assign(target, value, span);
                }
                Stmt::IndexAssign {
                    receiver,
                    index,
                    value,
                    span,
                } => {
                    self.check_index_assign(receiver, index, value, span);
                }
            }
        }
        // Clean up early-return narrowing facts when leaving the block.
        for name in &early_return_narrowed {
            self.maybe_non_none.remove(name.as_str());
        }
    }

    fn check_let(
        &mut self,
        is_const: bool,
        name: &str,
        name_span: &SourceSpan,
        annotation: Option<&AstType>,
        value: &Expr,
    ) {
        // v0.3-M4 Phase 2: `let h = background fn(...)` — the background handle-form
        // (lifting the M8 P5 rejection). The binding gets an inferred-only task-handle type
        // supporting `.send()` (into the task's first `channel<T>` parameter) and repeated
        // `.receive()` (message replies + the task's own completion value, typed `T errors`).
        if let Expr::Background(inner, bg_span) = value {
            let handle_ty = self.check_background_handle_spawn(inner, bg_span, annotation, value);
            // v0.3-M4 conduit-origin discipline: a spawn binding is a derivable conduit —
            // mirrors the resolver's `Expr::Background` arm in `let_binds_derivable_conduit`.
            self.derivable_conduits.insert(name.to_string());
            self.scope.insert(
                name.to_string(),
                ScopeEntry {
                    ty: handle_ty,
                    is_const,
                    is_param: false,
                    param_ownership: None,
                    is_loop_var: false,
                    is_consumed: false,
                    defined_at: name_span.clone(),
                },
            );
            return;
        }

        // v0.3-M4 Phase 2: `let x = ch.receive()` / `let r = ch.send(v)` are allowed
        // root positions for suspending conduit-method calls.
        self.record_conduit_root(value);

        let annotated_ty = annotation.map(|t| self.ast_type_to_type(t));
        let value_ty = self.infer_expr(value, annotated_ty.as_ref());

        // M7 P3c: range values are first-class — no restriction on storage.
        // (The M3 restriction is removed here.)

        let binding_ty = if let Some(ann_ty) = &annotated_ty {
            if value_ty == Type::Error || *ann_ty == Type::Error {
                Type::Error
            } else if !types_compatible(ann_ty, &value_ty) {
                self.diags.push(Diagnostic::error(
                    value.span().clone(),
                    format!(
                        "This value is `{}`, but `{}` is declared as `{}`.",
                        type_name(&value_ty),
                        name,
                        type_name(ann_ty)
                    ),
                    format!(
                        "Change the annotation to `{}`, or use a different value.",
                        type_name(&value_ty)
                    ),
                    "The value on the right side must match the type annotation on the left.",
                ));
                Type::Error
            } else if matches!(ann_ty, Type::Union { .. }) {
                // M6: for union type annotations, use the declared union type as the binding type,
                // not the concrete variant. `let s: Shape = circle` → s is Shape, not Circle.
                ann_ty.clone()
            } else {
                // Use the value_ty to preserve size information from ArrayLit inference.
                value_ty
            }
        } else {
            value_ty
        };

        // v0.3-M4 conduit-origin discipline: accumulate derivable conduit bindings with THE
        // shared resolver predicate — statement order and set semantics identical to the
        // may-block resolver's `Stmt::Let` arm, so the two views can never drift
        // (authoritative-derivation.md).
        if crate::may_block::let_binds_derivable_conduit(
            annotation,
            value,
            &self.derivable_conduits,
            &self.channel_returning_fns,
        ) {
            self.derivable_conduits.insert(name.to_string());
        }

        self.scope.insert(
            name.to_string(),
            ScopeEntry {
                ty: binding_ty,
                is_const,
                is_param: false,
                param_ownership: None,
                is_loop_var: false,
                is_consumed: false,
                defined_at: name_span.clone(),
            },
        );
    }

    /// v0.3-M4 Phase 2: typecheck `let h = background fn(...)` and compute the handle type.
    ///
    /// Runs the standard `Expr::Background` checks (kernel gate, must-wrap-a-call, borrow
    /// rejects, large-copy warnings) via `infer_expr`, then derives the handle type from
    /// the spawned callee's signature:
    /// - `result` = the callee's SUCCESS type (the `T` of `-> T errors`, or the plain
    ///   return type) — `.receive()` returns `T errors`;
    /// - `msg_elem` = the element type of the callee's first `channel<T>` parameter
    ///   (the conduit `h.send(v)` feeds), `None` when it has no channel parameter.
    ///
    /// The callee must be a SUSPENDING function: the handle substrate is an independent
    /// joinable Tokio task driving a state-machine frame (never a `CpuJoinHandle` — trap
    /// door 1a). A non-suspending (pure-CPU) callee is rejected with a teaching error;
    /// the registry records the deferral (`background-handle-nonsuspending-callee`).
    fn check_background_handle_spawn(
        &mut self,
        inner: &Expr,
        bg_span: &SourceSpan,
        annotation: Option<&AstType>,
        full_value: &Expr,
    ) -> Type {
        if let Some(ann) = annotation {
            // A task handle has no typeable source annotation in v0.3 — the binding's type
            // is inferred from the spawn expression.
            let _ = self.ast_type_to_type(ann);
            self.diags.push(Diagnostic::error(
                bg_span.clone(),
                "A background task handle has no type annotation — drop the annotation.",
                "Write `let h = background fn(...)` without a type annotation.",
                "The handle's type is figured out automatically from the spawned function's \
                 signature; there is no way to write it in source in v0.3.",
            ));
        }

        // Pre-record inferred ownership for plain-ident args BEFORE the Background arm's
        // large-copy warning reads it: channels are SHARED (refcounted alias — both sides
        // must operate on the same bounded buffer); everything else defaults to Copy (safe
        // direction — the caller keeps its original).
        if let Expr::Call(call) = inner {
            for arg in &call.args {
                if let Expr::Ident(n, span) = arg {
                    let is_channel = self
                        .scope
                        .lookup(n)
                        .is_some_and(|e| matches!(e.ty, Type::BuiltinChannel { .. }));
                    let o = if is_channel {
                        BgOwnership::Channel
                    } else {
                        BgOwnership::Copy
                    };
                    self.bg_inferred.insert((span.start, span.end), o);
                }
            }
        }

        // Standard Background checks (kernel gate, call enforcement, borrow rejects,
        // large-copy warnings). Its Type::Nothing result is replaced below.
        let _ = self.infer_expr(full_value, None);

        // Resolve the spawned callee's signature for the handle type.
        let callee_name = match inner {
            Expr::Call(call) => match &call.callee {
                Expr::Ident(n, _) => Some(n.clone()),
                _ => None,
            },
            _ => None,
        };
        let Some(callee_name) = callee_name else {
            // Non-call / non-ident callee — already diagnosed by the Background arm.
            return Type::Error;
        };
        let Some((suspends, ret, params)) = self
            .sig_table
            .fns
            .get(&callee_name)
            .map(|sig| (sig.suspends, sig.ret.clone(), sig.params.clone()))
        else {
            self.diags.push(Diagnostic::error(
                bg_span.clone(),
                format!(
                    "`{callee_name}` is not a user-defined function — a task handle needs one."
                ),
                "Spawn a function you defined: `let h = background worker(...)`.",
                "The handle form runs a whole function as an independent task and collects \
                 its completion value; built-in operations have nothing to collect.",
            ));
            return Type::Error;
        };
        if !suspends {
            self.diags.push(Diagnostic::error(
                bg_span.clone(),
                format!(
                    "`{callee_name}` never suspends — the handle form needs a suspending function in v0.3."
                ),
                format!(
                    "Either drop the `let` (fire-and-forget `background {callee_name}(...)` works \
                     for CPU-bound functions), or make `{callee_name}` suspending (it suspends if \
                     it uses `wait`, a channel, or calls a function that does)."
                ),
                "A handled task runs on the scheduler's cooperative pool, which drives \
                 functions that can suspend and resume. A function that never suspends runs \
                 on the CPU pool instead, where there is no handle to collect from yet — \
                 that support ships in a later milestone \
                 (`background-handle-nonsuspending-callee` in the feature registry).",
            ));
            return Type::Error;
        }

        let result = match &ret {
            Type::ErrorsCapable { inner } => inner.clone(),
            other => Box::new(other.clone()),
        };
        let msg_elem = params.iter().find_map(|(_, t)| {
            if let Type::BuiltinChannel { elem } = t {
                Some(elem.clone())
            } else {
                None
            }
        });
        let handle_ty = Type::BackgroundHandle { result, msg_elem };
        // Overwrite the Background expression's recorded type (the generic arm stored
        // `nothing`) so codegen and later reads see the handle type.
        let vspan = full_value.span();
        self.expr_types
            .insert((vspan.start, vspan.end), handle_ty.clone());
        handle_ty
    }

    fn check_assign(&mut self, target: &str, target_span: &SourceSpan, value: &Expr) {
        let value_ty = self.infer_expr(value, None);

        match self.scope.lookup(target) {
            None => {
                let mut candidates: Vec<&str> = self.scope.all_names();
                candidates.extend(self.sig_table.all_names());
                self.diags.push(make_not_defined_diag(
                    target,
                    target_span.clone(),
                    &candidates,
                    format!("Declare it first: `let {target} = ...`"),
                    "You can only assign to variables that have been declared with `let`.",
                ));
            }
            Some(entry) if entry.is_param => {
                self.diags.push(Diagnostic::error(
                    target_span.clone(),
                    format!("`{target}` is a parameter — parameters cannot be reassigned."),
                    format!("To work with a modified value, declare a new variable: `let my_{target} = {target}`"),
                    "In Yinz, function parameters are read-only by default. If you need to mutate the value, declare a `let` binding: `let my_name = name` then modify `my_name` instead.",
                ));
            }
            Some(entry) if entry.is_loop_var => {
                self.diags.push(Diagnostic::error(
                    target_span.clone(),
                    format!("`{target}` is the loop variable — it cannot be changed inside the loop body."),
                    format!("Declare a separate variable if you need a counter: `let count = {target}`"),
                    "The loop variable steps through values automatically each iteration. Changing it inside the body would cause confusing behavior.",
                ));
            }
            Some(entry) if entry.is_const => {
                self.diags.push(Diagnostic::error(
                    target_span.clone(),
                    format!(
                        "`{target}` cannot be changed — it was declared with `const`."
                    ),
                    format!(
                        "Change `const {target} = ...` to `let {target} = ...` if you need to reassign it."
                    ),
                    "`const` declares a value that never changes. Use `let` when the value needs to be updated.",
                ));
            }
            Some(entry) => {
                let bound_ty = entry.ty.clone();
                if value_ty != Type::Error && value_ty != bound_ty {
                    self.diags.push(Diagnostic::error(
                        value.span().clone(),
                        format!(
                            "Cannot store a `{}` in `{}` — it holds `{}`.",
                            type_name(&value_ty),
                            target,
                            type_name(&bound_ty)
                        ),
                        format!("The value must be a `{}`.", type_name(&bound_ty)),
                        format!(
                            "`{target}` was declared as `{}`. Storing a `{}` in it would change its type.",
                            type_name(&bound_ty),
                            type_name(&value_ty)
                        ),
                    ));
                }
            }
        }
    }

    fn check_stmt_if(&mut self, cond: &Expr, body: &Block) {
        let cond_ty = self.infer_expr(cond, None);
        if cond_ty != Type::Error && cond_ty != Type::Bool {
            self.diags.push(Diagnostic::error(
                cond.span().clone(),
                format!("The condition of an `if` must be `boolean`, but this is `{}`.", type_name(&cond_ty)),
                "Write a comparison that produces `true` or `false`, e.g. `x > 0`.",
                "`if` branches on whether the condition is `true` or `false`. Any other type cannot be used as a condition.",
            ));
        }

        // Flow-sensitive narrowing: if condition is `m.exists()`, mark `m` as known-non-none
        // inside the if body so `.value` is allowed without another guard.
        let narrowed = self.extract_exists_binding(cond);
        for name in &narrowed {
            self.maybe_non_none.insert(name.clone());
        }

        // M7 P3a: if condition is `x.failed()`, mark `x` as "consumed by failed check"
        // inside the if body. After the block, `x` is narrowed to success.
        let failed_binding = self.extract_failed_binding(cond);
        for name in &failed_binding {
            self.errors_consumed.insert(name.clone());
        }

        self.scope.push();
        self.check_stmts(&body.stmts);
        self.scope.pop();

        // Remove narrowing flags after the block exits.
        for name in &narrowed {
            self.maybe_non_none.remove(name.as_str());
        }

        // M7 P3a: after `if (x.failed()) { ... }`, narrow `x` to success for subsequent code.
        for name in &failed_binding {
            self.errors_success_narrowed.insert(name.clone());
        }
    }

    /// M7 P3a: extract the binding name from a `.failed()` condition.
    ///
    /// Matches `x.failed()` → `vec!["x"]` so the if-body can use error fields
    /// and subsequent code sees `x` narrowed to its success type.
    fn extract_failed_binding(&self, cond: &Expr) -> Vec<String> {
        if let Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } = cond
        {
            if method == "failed" && args.is_empty() {
                if let Expr::Ident(name, _) = receiver.as_ref() {
                    // Only extract when the binding is actually ErrorsCapable.
                    if let Some(entry) = self.scope.lookup(name) {
                        if matches!(entry.ty, Type::ErrorsCapable { .. }) {
                            return vec![name.clone()];
                        }
                    }
                }
            }
        }
        Vec::new()
    }

    /// Extract the binding name from an `.exists()` condition for flow-sensitive narrowing.
    ///
    /// Matches `m.exists()` → `vec!["m"]` so the if-body can use `m.value`.
    /// Any other form returns an empty vec.
    fn extract_exists_binding(&self, cond: &Expr) -> Vec<String> {
        if let Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } = cond
        {
            if method == "exists" && args.is_empty() {
                if let Expr::Ident(name, _) = receiver.as_ref() {
                    return vec![name.clone()];
                }
            }
        }
        Vec::new()
    }

    /// Extract the binding name from a NEGATED `.exists()` condition for early-return narrowing.
    ///
    /// Matches `!m.exists()` → `vec!["m"]` — after this if-block always returns,
    /// `m` is non-none for the rest of the enclosing block.
    fn extract_negated_exists_binding(&self, cond: &Expr) -> Vec<String> {
        if let Expr::UnaryOp {
            op: ynz_ast::nodes::UnaryOpKind::Not,
            operand,
            ..
        } = cond
        {
            return self.extract_exists_binding(operand);
        }
        Vec::new()
    }

    fn check_stmt_match(&mut self, scrutinee: &Expr, arms: &[MatchArm], else_arm: Option<&Block>) {
        let scrutinee_ty = self.infer_expr(scrutinee, None);

        for arm in arms {
            match &arm.pattern.kind {
                MatchPatternKind::Value(pat_expr) => {
                    let pat_ty = self.infer_expr(pat_expr, Some(&scrutinee_ty));
                    if pat_ty != Type::Error
                        && scrutinee_ty != Type::Error
                        && pat_ty != scrutinee_ty
                    {
                        self.diags.push(Diagnostic::error(
                            pat_expr.span().clone(),
                            format!(
                                "This arm pattern is `{}`, but the matched value is `{}`.",
                                type_name(&pat_ty),
                                type_name(&scrutinee_ty)
                            ),
                            format!(
                                "Use a `{}` literal or expression as the pattern.",
                                type_name(&scrutinee_ty)
                            ),
                            "Each arm pattern must have the same type as the value being matched.",
                        ));
                    }
                }
                // Is: M6 union narrowing — validate variant, narrow inside arm body.
                MatchPatternKind::Is(type_path) => {
                    self.check_is_arm_pattern(&scrutinee_ty, type_path, &arm.pattern.span);
                    // Narrowing: inside this arm's body, the scrutinee binding is narrowed.
                    // We push a scope with the narrowed type for the binding if we can identify it.
                    // (Full binding-name extraction is P3b — basic case: scrutinee is a direct Ident)
                    let narrowed_name = simple_ident_name(scrutinee).map(|s| s.to_string());
                    if let Some(ref name) = narrowed_name {
                        self.union_narrowed.insert(
                            name.clone(),
                            Type::Shape {
                                name: type_path.name.clone(),
                            },
                        );
                    }
                    self.scope.push();
                    self.check_stmts(&arm.body.stmts);
                    self.scope.pop();
                    if let Some(ref name) = narrowed_name {
                        self.union_narrowed.remove(name);
                    }
                    continue; // skip the standard scope push/pop below
                }
                // OptionName: M6 options multi-case arm.
                MatchPatternKind::OptionName(variant_name) => {
                    self.check_option_name_arm(&scrutinee_ty, variant_name, &arm.pattern.span);
                }
            }
            self.scope.push();
            self.check_stmts(&arm.body.stmts);
            self.scope.pop();
        }

        // Exhaustiveness check for options multi-case.
        if let Type::Options { name: opts_name } = &scrutinee_ty {
            if let Some(entry) = self.options_table.get(opts_name) {
                let covered: std::collections::HashSet<&str> = arms
                    .iter()
                    .filter_map(|arm| {
                        if let MatchPatternKind::OptionName(v) = &arm.pattern.kind {
                            Some(v.as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                if else_arm.is_none() {
                    let missing: Vec<&str> = entry
                        .variants
                        .iter()
                        .filter(|v| !covered.contains(v.as_str()))
                        .map(String::as_str)
                        .collect();
                    if !missing.is_empty() {
                        self.diags.push(Diagnostic::error(
                            scrutinee.span().clone(),
                            format!(
                                "Non-exhaustive options multi-case — `{}` has {} variants; {} are not handled: {}.",
                                opts_name,
                                entry.variants.len(),
                                missing.len(),
                                missing.join(", ")
                            ),
                            format!("Add the missing arms (e.g. `{} =>`) or add an `else =>` default arm.", missing[0]),
                            "The compiler knows every variant at compile time. A missing arm means some values would silently fall through — likely a bug.",
                        ));
                    }
                }
            }
        }

        // M6: Union exhaustiveness check for `Is` arms.
        if let Type::Union { variants } = &scrutinee_ty {
            let covered: std::collections::HashSet<String> = arms
                .iter()
                .filter_map(|arm| {
                    if let MatchPatternKind::Is(tp) = &arm.pattern.kind {
                        Some(tp.name.clone())
                    } else {
                        None
                    }
                })
                .collect();
            if else_arm.is_none() {
                let missing: Vec<String> = variants
                    .iter()
                    .filter_map(|v| {
                        if let Type::Shape { name } = v {
                            if !covered.contains(name) {
                                Some(name.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect();
                if !missing.is_empty() {
                    self.diags.push(Diagnostic::error(
                        scrutinee.span().clone(),
                        format!(
                            "Non-exhaustive union multi-case — {} variant{} not handled: {}.",
                            missing.len(),
                            if missing.len() == 1 { " is" } else { "s are" },
                            missing.join(", ")
                        ),
                        format!("Add the missing arms (e.g. `is {} =>`) or add an `else =>` default arm.", missing[0]),
                        "The compiler knows every union variant at compile time. A missing arm means some values silently fall through — likely a bug.",
                    ));
                }
            }
        }

        if let Some(else_body) = else_arm {
            self.scope.push();
            self.check_stmts(&else_body.stmts);
            self.scope.pop();
        }
    }

    fn check_stmt_while(&mut self, cond: &Expr, body: &Block) {
        let cond_ty = self.infer_expr(cond, None);
        if cond_ty != Type::Error && cond_ty != Type::Bool {
            self.diags.push(Diagnostic::error(
                cond.span().clone(),
                format!("The condition of a `while` loop must be `boolean`, but this is `{}`.", type_name(&cond_ty)),
                "Write a comparison that produces `true` or `false`, e.g. `x > 0`.",
                "`while` loops until the condition becomes `false`. Any other type cannot be used as a condition.",
            ));
        }
        self.scope.push();
        self.check_stmts(&body.stmts);
        self.scope.pop();
    }

    fn check_stmt_for(&mut self, var: &str, var_span: &SourceSpan, iter: &Expr, body: &Block) {
        let iter_ty = self.infer_expr(iter, None);

        // M7 P3c: Iterable<T> protocol dispatch. Each built-in collection type
        // maps to an element type. User shapes are checked for a next() function
        // matching the Iterable<T> contract.
        let elem_ty = match &iter_ty {
            Type::Range { element, .. } => *element.clone(),
            Type::BuiltinArray { elem } => *elem.clone(),
            Type::BuiltinFixed { elem, .. } => *elem.clone(),
            Type::BuiltinMap { key, val } => Type::MapEntry {
                key: key.clone(),
                val: val.clone(),
            },
            // M7 P3c: string iteration yields one code-point string per step.
            Type::String => Type::String,
            // M7 P3c: user shape iteration — requires a standalone next() function
            // whose return type is maybe<T>. The element type T is extracted from it.
            Type::Shape { name } => self.infer_iterable_element_for_shape(name, iter.span()),
            Type::Error => Type::Error,
            other => {
                self.diags.push(Diagnostic::error(
                    iter.span().clone(),
                    format!("`for` loops over `{}` are not supported.", type_name(other)),
                    "Use `range(...)`, iterate over `array<T>`, `fixed<T>`, `map<K, V>`, `string`, or a shape that follows `Iterable<T>`.",
                    "For custom types, define `function next(lend self: YourShape) -> maybe<T>` to make them iterable.",
                ));
                Type::Error
            }
        };

        self.scope.push();
        self.scope.insert(
            var.to_string(),
            ScopeEntry {
                ty: elem_ty,
                is_const: false,
                is_param: false,
                param_ownership: None,
                is_loop_var: true,
                is_consumed: false,
                defined_at: var_span.clone(),
            },
        );
        self.check_stmts(&body.stmts);
        self.scope.pop();
    }

    /// M7 P3c: look up the element type T for iterating over a user-defined shape.
    ///
    /// A shape is iterable if there is a standalone `next` function in the signature
    /// table whose first parameter is `Shape { name }` and whose return type is
    /// `Maybe { inner: T }`. If no such function exists, emits a diagnostic and
    /// returns `Type::Error`.
    fn infer_iterable_element_for_shape(&mut self, shape_name: &str, span: &SourceSpan) -> Type {
        if let Some(sig) = self.sig_table.fns.get("next") {
            let shape_ty = Type::Shape {
                name: shape_name.to_string(),
            };
            if let Some((_, first_ty)) = sig.params.first() {
                if *first_ty == shape_ty {
                    // next() returns maybe<T> — extract T as the element type.
                    return match &sig.ret {
                        Type::Maybe { inner } => *inner.clone(),
                        other => {
                            self.diags.push(Diagnostic::error(
                                span.clone(),
                                format!("`{shape_name}.next()` must return `maybe<T>` to be iterable, but it returns `{}`.", type_name(other)),
                                format!("Change `function next(lend self: {shape_name}) -> {}` to return `maybe<T>` instead.", type_name(other)),
                                "The `Iterable<T>` contract requires `next(lend self) -> maybe<T>`. When the iterator is exhausted, return `none`.",
                            ));
                            Type::Error
                        }
                    };
                }
            }
        }
        self.diags.push(Diagnostic::error(
            span.clone(),
            format!("`{shape_name}` cannot be iterated — it does not follow `Iterable<T>`."),
            format!("Add `function next(lend self: {shape_name}) -> maybe<T>` to make it iterable."),
            "For a `for` loop to work on a custom shape, the shape needs a standalone `next` function returning `maybe<T>`. When the iterator is done, return `none`.",
        ));
        Type::Error
    }

    fn check_stmt_return(&mut self, value: Option<&Expr>, span: &SourceSpan) {
        let expected = self.current_fn_ret.clone();
        match (value, &expected) {
            (None, Type::Nothing) => {}
            (None, Type::Error) => {}
            (None, ret) => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!(
                        "`return` without a value, but this function must return `{}`.",
                        type_name(ret)
                    ),
                    "Add a return value: `return expr`",
                    "A non-`nothing` function must return a value on every path that exits.",
                ));
            }
            (Some(expr), Type::Nothing) => {
                self.infer_expr(expr, None);
                self.diags.push(Diagnostic::error(
                    expr.span().clone(),
                    "`return` with a value in a `-> nothing` function.",
                    "Remove the value: write `return` with no expression.",
                    "Functions declared `-> nothing` do not produce a value. A `return` inside them ends the function — no value allowed.",
                ));
            }
            (Some(expr), ret) => {
                let val_ty = self.infer_expr(expr, Some(ret));
                if val_ty != Type::Error && *ret != Type::Error && !types_compatible(ret, &val_ty) {
                    // M7 P3a: in an errors-capable function, returning the inner success
                    // type is valid (the auto-propagation machinery wraps it at codegen).
                    let compatible = if let Type::ErrorsCapable { inner } = ret {
                        types_compatible(&val_ty, inner)
                    } else if let Type::ErrorsCapable { inner } = &val_ty {
                        // Returning an ErrorsCapable value from an errors function is also valid.
                        self.current_fn_errors_capable && types_compatible(inner, ret)
                    } else {
                        false
                    };
                    if !compatible {
                        self.diags.push(Diagnostic::error(
                            expr.span().clone(),
                            format!(
                                "`return` produces `{}`, but this function must return `{}`.",
                                type_name(&val_ty),
                                type_name(ret)
                            ),
                            format!("Return a `{}` value instead.", type_name(ret)),
                            format!(
                                "The function's declared return type is `{}`. Every `return` must produce a value of that type.",
                                type_name(ret)
                            ),
                        ));
                    }
                }
            }
        }
    }

    /// Heuristic estimate of conceptual copy cost when the user writes `.copy()`:
    /// 8 bytes per field for shape values (assumes each field is i64-sized).
    ///
    /// This is NOT the C-ABI slot size at the background-spawn site (codegen packs each
    /// arg into a single i64 regardless of the shape's field count). Used only to decide
    /// whether to emit the large-copy warning; not load-bearing for correctness.
    fn estimate_type_size_bytes(&self, ty: &Type) -> usize {
        match ty {
            Type::Shape { name, .. } => {
                if let Some(def) = self.shape_table.shapes.get(name.as_str()) {
                    // Each field = 8 bytes (i64 ABI slot)
                    def.fields.len() * 8
                } else {
                    8
                }
            }
            // Scalar and pointer types: always 8 bytes in the i64 ABI
            _ => 8,
        }
    }

    /// Infer the type of `expr`.
    ///
    /// `hint` is an optional expected type passed from a `let` annotation.
    /// It only affects literal expressions (`IntLit`, `NumberLit`) — it does
    /// not change how compound expressions like `BinOp` are inferred.
    fn infer_expr(&mut self, expr: &Expr, hint: Option<&Type>) -> Type {
        let ty = match expr {
            Expr::StringLit(_, _) => Type::String,
            Expr::Ident(name, span) => self.resolve_ident(name, span),
            Expr::Call(call) => self.check_call(call),
            Expr::Error(span) => {
                self.expr_types.insert((span.start, span.end), Type::Error);
                return Type::Error;
            }

            Expr::IntLit(_, _) => match hint {
                Some(Type::Number { precision: 34 }) => Type::Number { precision: 34 },
                Some(Type::Float) => Type::Float,
                _ => Type::Int,
            },

            Expr::NumberLit(_, _) => match hint {
                Some(Type::Float) => Type::Float,
                // M8 P6: use the annotated precision when a number annotation is present.
                Some(Type::Number { precision }) => Type::Number {
                    precision: *precision,
                },
                _ => Type::Number { precision: 34 },
            },

            Expr::BoolLit(_, _) => Type::Bool,

            Expr::BinOp { op, lhs, rhs, span } => {
                let lhs_ty = self.infer_expr(lhs, None);
                let rhs_ty = self.infer_expr(rhs, None);
                self.check_binop(op, &lhs_ty, &rhs_ty, span)
            }

            Expr::UnaryOp { op, operand, span } => {
                let operand_ty = self.infer_expr(operand, None);
                self.check_unaryop(op, &operand_ty, span)
            }

            Expr::MethodCall {
                receiver,
                method,
                method_span,
                args,
                span: method_call_span,
            } => {
                let receiver_ty = self.infer_expr(receiver, None);
                // EC-specific methods on a let/const-bound ErrorsCapable value inside an
                // errors-capable function: resolve_ident auto-propagates the binding from
                // ErrorsCapable<T> → T (so the compiler can insert early-return IR). That
                // means receiver_ty here is the bare inner type, and check_method_call
                // cannot find .or/.failed/.message etc. on it.
                //
                // Restore the full ErrorsCapable<T> for dispatch when ALL of:
                //   (a) the method is one of the EC-specific set,
                //   (b) the inferred receiver_ty is NOT already ErrorsCapable (was auto-stripped),
                //   (c) the receiver is a bare Ident whose SCOPE ENTRY still carries ErrorsCapable.
                //
                // This is a narrow, targeted fix — it does not affect normal value-context
                // auto-propagation (check_user_fn_call etc.) or non-EC method dispatch.
                const EC_METHODS: &[&str] =
                    &["or", "failed", "message", "suggestions", "trace", "source"];
                let effective_receiver_ty = if !matches!(receiver_ty, Type::ErrorsCapable { .. })
                    && EC_METHODS.contains(&method.as_str())
                {
                    if let Expr::Ident(ident_name, ident_span) = receiver.as_ref() {
                        if let Some(entry) = self.scope.lookup(ident_name) {
                            if matches!(entry.ty, Type::ErrorsCapable { .. }) {
                                // Restore the ErrorsCapable type for EC-method dispatch.
                                // Auto-propagation in resolve_ident already stripped the type
                                // and wrote the bare inner type into expr_types. Overwrite that
                                // entry with the full ErrorsCapable type so codegen reads the
                                // right ABI ({i64,i64} pair) when lowering the EC method call.
                                let ec_ty = entry.ty.clone();
                                self.expr_types
                                    .insert((ident_span.start, ident_span.end), ec_ty.clone());
                                ec_ty
                            } else {
                                receiver_ty
                            }
                        } else {
                            receiver_ty
                        }
                    } else {
                        receiver_ty
                    }
                } else {
                    receiver_ty
                };
                // v0.3-M4 Phase 2: suspending conduit-method surface — `.send()`/`.receive()`
                // on a `channel<T>` value or a background task handle. Dispatched BEFORE the
                // generic method paths so the element-typed argument check and the
                // receiver/statement-position discipline apply.
                if matches!(
                    effective_receiver_ty,
                    Type::BuiltinChannel { .. } | Type::BackgroundHandle { .. }
                ) {
                    self.check_conduit_method_call(
                        &effective_receiver_ty,
                        receiver,
                        method,
                        method_span,
                        args,
                        method_call_span,
                    )
                } else
                // M4 P5: one-arg intrinsic methods (wrapping/saturating arithmetic).
                // Must NOT use `return` here — the match value feeds expr_types.insert below.
                if args.len() == 1 {
                    if let Some((expected_arg_ty, ret_ty)) = self
                        .intrinsics
                        .lookup_method_1arg(&effective_receiver_ty, method)
                    {
                        let expected = expected_arg_ty.clone();
                        let actual = self.infer_expr(&args[0], Some(&expected));
                        if actual != expected && actual != Type::Error {
                            self.diags.push(ynz_diagnostics::Diagnostic::error(
                                args[0].span().clone(),
                                format!("`.{method}()` expects `{}` but got `{}`.", crate::types::type_name(&expected), crate::types::type_name(&actual)),
                                format!("Pass an `{}` value.", crate::types::type_name(&expected)),
                                format!("`.{method}()` is a primitive arithmetic operation that only works on `{}`.", crate::types::type_name(&expected)),
                            ));
                        }
                        ret_ty
                    } else {
                        for arg in args.iter() {
                            self.infer_expr(arg, None);
                        }
                        self.check_method_call(
                            &effective_receiver_ty,
                            Some(receiver),
                            method,
                            method_span,
                        )
                    }
                } else {
                    for arg in args.iter() {
                        self.infer_expr(arg, None);
                    }
                    self.check_method_call(
                        &effective_receiver_ty,
                        Some(receiver),
                        method,
                        method_span,
                    )
                }
            }

            Expr::FieldAccess {
                receiver,
                field,
                field_span,
                ..
            } => {
                // M4 P5: type-attached constants (e.g. `int.max`, `number.epsilon`).
                // Intercept before inferring receiver type to avoid "undefined `int`" error.
                if let Expr::Ident(type_name_str, _) = receiver.as_ref() {
                    if let Some(const_ty) = type_attached_const_type(type_name_str, field) {
                        const_ty
                    } else if self.options_table.contains(type_name_str) {
                        // M6: OptionsValue — `Status.active` where Status is an options type.
                        self.check_options_value(type_name_str, field, field_span)
                    } else {
                        self.infer_field_access(receiver, field, field_span)
                    }
                } else {
                    self.infer_field_access(receiver, field, field_span)
                }
            }
            Expr::StructLit { fields, span } => self.check_struct_lit(fields, hint, span),
            Expr::PostfixOp { receiver, op, span } => self.check_postfix_op(receiver, op, span),
            Expr::SelfValue { span } => match self.scope.lookup("self") {
                Some(entry) => entry.ty.clone(),
                None => {
                    self.diags.push(Diagnostic::error(
                            span.clone(),
                            "`self` can only be used inside a function whose first parameter is named `self`.",
                            "Add `share self: ShapeName` as the first parameter of this function.",
                            "`self` refers to the value the function was called on. It must be declared as the first parameter.",
                        ));
                    Type::Error
                }
            },
            Expr::NoneLit { span } => {
                // M7 P3a: if the hint is ErrorsCapable wrapping Maybe, unwrap to the Maybe type.
                let effective_hint = match hint {
                    Some(Type::ErrorsCapable { inner })
                        if matches!(inner.as_ref(), Type::Maybe { .. }) =>
                    {
                        Some(inner.as_ref())
                    }
                    other => other,
                };
                match effective_hint {
                    Some(Type::Maybe { .. }) => effective_hint.unwrap().clone(),
                    // When hint is Type::Error, an upstream annotation error was already emitted —
                    // suppress the cascade by returning Error silently.
                    Some(Type::Error) => Type::Error,
                    None => {
                        self.diags.push(Diagnostic::error(
                            span.clone(),
                            "Cannot work out which type `none` should be here.",
                            "Annotate the binding: `let x: maybe<int> = none`.",
                            "`none` is the absent value of `maybe<T>` for some T. The compiler needs the annotation to know which T.",
                        ));
                        Type::Error
                    }
                    Some(other) => {
                        let other_name = type_name(other);
                        self.diags.push(Diagnostic::error(
                            span.clone(),
                            format!("`none` cannot be a `{other_name}` value."),
                            "Use `maybe<T>` for optional values: `let x: maybe<int> = none`.",
                            "`none` is only valid as the absent value of `maybe<T>`. It cannot be used where a concrete type is expected.",
                        ));
                        Type::Error
                    }
                }
            }
            Expr::IndexAccess {
                receiver,
                index,
                span,
            } => {
                let recv_ty = self.infer_expr(receiver, None);
                let _idx_ty = self.infer_expr(index, Some(&Type::Int));
                match &recv_ty {
                    Type::BuiltinArray { elem } | Type::BuiltinFixed { elem, .. } => Type::Maybe {
                        inner: elem.clone(),
                    },
                    Type::BuiltinMap { val, .. } => Type::Maybe { inner: val.clone() },
                    // M7 P3b: string bracket access desugars to .get(n) → maybe<string>
                    Type::String => Type::Maybe {
                        inner: Box::new(Type::String),
                    },
                    Type::Error => Type::Error,
                    other => {
                        self.diags.push(Diagnostic::error(
                            span.clone(),
                            format!("`{}` does not support bracket access.", type_name(other)),
                            "Bracket access works on `array<T>`, `fixed<T>`, `map<K, V>`, and `string`.",
                            "Use `.get(index)` on built-in collections or access shape fields with dot notation.",
                        ));
                        Type::Error
                    }
                }
            }
            Expr::ArrayLit { elements, span } => self.check_array_lit(elements, hint, span),
            Expr::MapLit { entries, span } => self.check_map_lit(entries, hint, span),
            // M6: `x is Foo` type-narrowing predicate — returns bool.
            Expr::Is {
                expr: inner,
                ty: type_path,
                span,
            } => self.check_is_expr(inner, type_path, span),
            // M7 P3b: interpolated string — validate that each ${...} expression
            // has a stringifiable type. Primitive types are always valid; shapes
            // require a standalone `toString` function.
            // M8 P4: if ANY interpoland is `sensitive`, the result is `sensitive string`.
            Expr::InterpolatedString(parts, _) => {
                let mut has_sensitive = false;
                for part in parts {
                    if let ynz_ast::nodes::StringPart::Expr(e, span) = part {
                        let part_ty = self.infer_expr(e, None);
                        if matches!(&part_ty, Type::Sensitive { .. }) {
                            has_sensitive = true;
                        } else if !is_stringifiable(&part_ty, self.sig_table) {
                            let type_name_str = type_name(&part_ty);
                            self.diags.push(Diagnostic::error(
                                span.clone(),
                                format!("`{type_name_str}` cannot be used inside a string interpolation."),
                                format!(
                                    "Add a `.toString()` method to `{type_name_str}`: \
                                     `function toString(share self: {type_name_str}) -> string {{ ... }}`"
                                ),
                                "String interpolation calls `.toString()` on each `${{}}` expression. \
                                 Primitive types (int, float, bool, string) work automatically. \
                                 Custom shapes need a standalone `toString` function.",
                            ));
                        }
                    }
                }
                if has_sensitive {
                    Type::Sensitive {
                        inner: Box::new(Type::String),
                    }
                } else {
                    Type::String
                }
            }
            // `wait expr` — kernel-mode rejects `wait` (no scheduler runtime).
            // M2: adds non-call-expression error + may-block warning.
            Expr::Wait(inner, span) => {
                if self.kernel_mode {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        "`wait` is not available in --kernel mode.",
                        "Remove the keyword or build without `--kernel`. Kernel-mode programs run without a scheduler runtime.",
                        "The thread-pool runtime that powers `wait` does not run in kernel mode. See `design/no-runtime-mode.md` for the kernel-mode contract.",
                    ));
                    // Return Type::Error without recursing into the inner expression. Recursing
                    // would visit the inner Expr::Call and trigger the call-dispatch kernel guard,
                    // emitting a second diagnostic for the same site. One diagnostic per site is
                    // the contract — the `wait` rejection already names the cause.
                    return Type::Error;
                }
                // `wait` must be followed by a call expression.
                // `wait background X()` is a parser error (background is statement-position only),
                // so the (inside_wait=true, inside_background=true) corner is unreachable here.
                let is_call = matches!(inner.as_ref(), Expr::Call(_) | Expr::MethodCall { .. });
                if !is_call {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        "`wait` must be followed by a function call.",
                        "Write `wait someFn()` to wait for `someFn` to complete.",
                        "`wait` schedules a suspension point. It only applies to function calls \
                         whose result must be waited for.",
                    ));
                    return self.infer_expr(inner, hint);
                }
                // Set inside_wait context so call dispatch can emit may-block warning.
                let prev_inside_wait = self.inside_wait;
                self.inside_wait = true;
                let result = self.infer_expr(inner, hint);
                self.inside_wait = prev_inside_wait;
                result
            }
            // `background expr` — must be a function call; return type is Nothing
            // (return value is discarded). Ownership rules enforced in check_stmt.
            Expr::Background(inner, span) => {
                // Kernel-mode rejection — background requires the thread-pool runtime.
                if self.kernel_mode {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        "`background` is not available in --kernel mode.",
                        "Remove the keyword or build without `--kernel`. Kernel-mode programs run without a scheduler runtime.",
                        "The thread-pool runtime that powers `background` does not run in kernel mode. See `design/no-runtime-mode.md` for the kernel-mode contract.",
                    ));
                    // Still infer inner for completeness; return Nothing.
                    let _ = self.infer_expr(inner, None);
                    return Type::Nothing;
                }

                // Set inside_background so the call-site `wait_required_on_state_machine_call`
                // check exempts this call — `background sm_fn()` from inside a state machine
                // is a legal route-to-I/O-pool pattern (Round 2 Required Fix #2).
                let prev_inside_background = self.inside_background;
                self.inside_background = true;
                let inner_ty = self.infer_expr(inner, None);
                self.inside_background = prev_inside_background;
                // background must wrap a function call — enforce this.
                if !matches!(inner.as_ref(), Expr::Call(_) | Expr::MethodCall { .. }) {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        "`background` must be followed by a function call.",
                        "Write `background process(data)` to call `process` in the background.",
                        "`background` schedules a function call to run outside the current scope. \
                         It cannot be applied to non-call expressions.",
                    ));
                }
                // Locked M8 decision (spec/concurrency.md:164-177): `background` must
                // reject callees that borrow their arguments via `share`. A `share`
                // borrow may outlive the caller's scope once the task runs in the background.
                // Reject `lend`-param callees: a borrow may outlive the owner across the
                // thread boundary — same safety hole as `share`.
                let callee_name: Option<&str> = match inner.as_ref() {
                    Expr::Call(call) => {
                        if let Expr::Ident(name, _) = &call.callee {
                            Some(name.as_str())
                        } else {
                            None
                        }
                    }
                    Expr::MethodCall { method, .. } => Some(method.as_str()),
                    _ => None,
                };
                if let Some(name) = callee_name {
                    if let Some(sig) = self.sig_table.fns.get(name) {
                        // v0.3-M4: `channel<T>` parameters are EXEMPT from the borrow
                        // rejects. A channel is the sanctioned cross-task conduit: the
                        // underlying bounded buffer is heap-owned, internally thread-safe,
                        // and refcount-shared at the spawn (`ynz_channel_share`) — the
                        // borrow-outlives-owner hole the rejects close cannot occur.
                        let borrowed_non_channel = |modifier: OwnershipModifier| {
                            sig.param_ownerships.iter().zip(sig.params.iter()).any(
                                |(o, (_, ty))| {
                                    o.as_ref() == Some(&modifier)
                                        && !matches!(ty, Type::BuiltinChannel { .. })
                                },
                            )
                        };
                        if borrowed_non_channel(OwnershipModifier::Share) {
                            self.diags.push(Diagnostic::error(
                                inner.span().clone(),
                                "Cannot use `background` with a function that borrows its arguments.",
                                "Change the parameter to `give` (take ownership) or pass a copy: `background fn(value.copy())`.",
                                "`background` will run this function outside the current scope. If the function only borrows its argument (via `share`), the borrow may outlive the value — a memory-safety hole. Pass ownership (`give`) or a copy so the background task has its own value.",
                            ));
                        }
                        // `lend` across a thread boundary is a safety error (same hole as `share`).
                        if borrowed_non_channel(OwnershipModifier::Lend) {
                            self.diags.push(Diagnostic::error(
                                inner.span().clone(),
                                "Cannot use `background` with a function that mutates its arguments via `lend`.",
                                "Change the parameter to `give` (transfer ownership) or pass a copy: `background fn(value.copy)`.",
                                "`background` runs this function outside the current scope. A `lend` borrow allows mutation through the borrow; if the value's owner reassigns or drops it concurrently, the background task's mutations would corrupt freed memory. Transfer ownership (`give`) or pass a copy so the background task owns its argument.",
                            ));
                        }
                    }
                }

                // Large-copy warning (Tier 3 lint): warn when a `.copy` arg (explicit or
                // inferred) is a shape with estimated size > 64 bytes.
                // Size estimate: each field = 8 bytes (all values are i64-sized in the
                // background ctx ABI). Threshold matches cache-line size.
                const BACKGROUND_LARGE_COPY_BYTES: usize = 64;
                if let Expr::Call(call) = inner.as_ref() {
                    for arg in &call.args {
                        // Explicit `.copy` postfix: PostfixOp { op: Copy, receiver }
                        if let Expr::PostfixOp { op, receiver, .. } = arg {
                            if *op == ynz_ast::nodes::PostfixOpKind::Copy {
                                let arg_ty = self.infer_expr(receiver, None);
                                let size = self.estimate_type_size_bytes(&arg_ty);
                                if size > BACKGROUND_LARGE_COPY_BYTES {
                                    self.diags.push(Diagnostic::warning(
                                        arg.span().clone(),
                                        format!("Copying {} bytes into a background task.", size),
                                        "If you don't need the value after the spawn, remove the `.copy()` — the compiler will transfer ownership to the task without copying.",
                                        "Transferring ownership is faster than copying for large values — the compiler does it automatically when the value is not used after the spawn. Use `.copy()` only when you need to keep using the value in the caller after the spawn.",
                                    ));
                                }
                            }
                        }
                        // Compiler-chosen `.copy` for plain Ident args where the value is
                        // read again after the spawn (so the task needs its own independent copy).
                        if let Expr::Ident(_, span) = arg {
                            let key = (span.start, span.end);
                            if matches!(self.bg_inferred.get(&key), Some(BgOwnership::Copy)) {
                                let arg_ty = self.infer_expr(arg, None);
                                let size = self.estimate_type_size_bytes(&arg_ty);
                                if size > BACKGROUND_LARGE_COPY_BYTES {
                                    self.diags.push(Diagnostic::warning(
                                        arg.span().clone(),
                                        format!("Copying {} bytes into a background task (the compiler chose copy because the value is used after the spawn).", size),
                                        "If you don't need the value after the spawn, restructure so the value is not read again — the compiler will transfer ownership instead of copying.",
                                        "When a value is not read after the spawn point, the compiler transfers ownership to the background task without copying. When the value IS read after the spawn, the compiler makes a copy so both the caller and the task have their own independent value.",
                                    ));
                                }
                            }
                        }
                    }
                }

                let _ = inner_ty;
                Type::Nothing // background discards the return value
            }
        };

        self.expr_types
            .insert((expr.span().start, expr.span().end), ty.clone());
        ty
    }

    fn resolve_ident(&mut self, name: &str, span: &SourceSpan) -> Type {
        // M6: if inside a union `is` arm, the binding may be narrowed to a specific variant.
        if let Some(narrowed_ty) = self.union_narrowed.get(name).cloned() {
            return narrowed_ty;
        }
        if let Some(entry) = self.scope.lookup(name) {
            if entry.is_consumed {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{name}` was already given away and cannot be used here."),
                    "Create a new value or use `.copy()` before passing if you need it in both places.",
                    "When a function takes ownership of a value, the caller no longer holds it. Using it afterward would be a memory safety violation.",
                ));
                return Type::Error;
            }

            // M7 P3a: ErrorsCapable binding use handling.
            // Return the ErrorsCapable type as-is so method dispatch (.failed, .or, etc.)
            // can handle it. Auto-propagation fires when the binding is passed to a function
            // expecting the plain success type — that check happens in check_user_fn_call.
            if let Type::ErrorsCapable { inner } = &entry.ty {
                let inner = inner.as_ref().clone();

                // Already narrowed to success type (after .failed() check or prior use) —
                // return the success type directly.
                if self.errors_success_narrowed.contains(name) {
                    return inner;
                }

                if self.current_fn_errors_capable {
                    // Inside an errors function: auto-propagation fires — narrow the
                    // binding to its success type. The compiler will insert early-return-
                    // on-failure IR at P4a; for typeck, just return the inner type.
                    self.errors_success_narrowed.insert(name.to_string());
                    self.errors_consumed.insert(name.to_string());
                    return inner;
                }
                // Outside an errors function: return the full ErrorsCapable type.
                // check_user_fn_call will emit the diagnostic if it's passed as
                // a success-typed argument. Method calls (.failed, .or) are fine.
            }

            return entry.ty.clone();
        }

        let mut candidates: Vec<&str> = self.scope.all_names();
        candidates.extend(self.sig_table.all_names());
        candidates.extend(["print", "range"]);
        self.diags.push(make_not_defined_diag(
            name,
            span.clone(),
            &candidates,
            format!("Check the spelling, or declare it: `let {name} = ...`"),
            &format!(
                "`{name}` has no declaration in scope. Declare it with `let {name} = ...` before \
                 using it, or check whether it's defined in a file you haven't imported yet."
            ),
        ));
        Type::Error
    }

    fn check_call(&mut self, call: &CallExpr) -> Type {
        let callee_name = match &call.callee {
            Expr::Ident(name, _) => name.clone(),
            _ => {
                self.infer_expr(&call.callee, None);
                return Type::Error;
            }
        };

        // Test-only functions (only compiled in test builds).
        #[cfg(test)]
        if let Some(sig) = self.intrinsics.lookup_test_fn(&callee_name) {
            let sig = sig.clone();
            return self.check_test_fn_call(call, &callee_name, &sig);
        }

        // Phase 6: explicit `wait` on a known-CPU-only intrinsic — the `wait` has no effect.
        // Only fires for non-suspending builtins. `sleep` and `__testFallibleAsync` are
        // may-block so they are excluded. User-defined function dispatch handles `suspends`
        // via the transitive analysis result on the sig table.
        if self.inside_wait
            && matches!(
                callee_name.as_str(),
                "print" | "range" | "sleepBlocking" | "sensitive"
            )
        {
            self.diags.push(wait_on_non_may_block_warning(
                call.span.clone(),
                &callee_name,
                &format!("{callee_name}(...)"),
            ));
        }

        // `wait`/`background` apply to the directly-awaited/backgrounded call itself — not to
        // calls nested inside its argument list. Save and clear so argument recursion sees no
        // wait/background context; restore after the dispatch returns.
        let was_inside_wait = self.inside_wait;
        let was_inside_background = self.inside_background;
        self.inside_wait = false;
        self.inside_background = false;

        let result = match callee_name.as_str() {
            "print" => self.check_print_call(call),
            "range" => self.check_range_call(call),
            // sleepBlocking(ms: int) — synchronous blocking sleep; lowers to ynz_thread_sleep_ms.
            "sleepBlocking" => self.check_sleep_blocking_call(call),
            // sleep(ms: int) — non-blocking sleep; codegen emits state-machine wait point.
            // Under the Phase-6 inference model, `sleep` is auto-awaited by the
            // transitive may-block analysis. Writing `wait sleep(...)` is valid-but-redundant
            // (the `wait_on_non_may_block` warning does NOT fire for may-block intrinsics).
            "sleep" => {
                if self.kernel_mode {
                    self.diags.push(Diagnostic::error(
                        call.span.clone(),
                        "`sleep` is not available in --kernel mode.",
                        "Use `sleepBlocking` for blocking sleep, or remove the call. \
                         Kernel-mode programs run without a scheduler runtime.",
                        "`sleep` requires the Tokio runtime (started by `ynz_rt_init`), \
                         which does not run in kernel mode. \
                         See `design/no-runtime-mode.md` for the kernel-mode contract.",
                    ));
                    for arg in &call.args {
                        self.infer_expr(arg, None);
                    }
                    return Type::Nothing;
                }
                self.check_sleep_call(call)
            }
            // __testFallibleAsync(succeed: bool) -> int errors — internal M2 test intrinsic.
            // Not in registry; not in LSP completion. Used only in P3/P5 driver fixtures.
            "__testFallibleAsync" => {
                if self.kernel_mode {
                    self.diags.push(Diagnostic::error(
                        call.span.clone(),
                        "`__testFallibleAsync` is not available in --kernel mode.",
                        "Remove the call or build without `--kernel`.",
                        "`__testFallibleAsync` requires the Tokio runtime.",
                    ));
                    for arg in &call.args {
                        self.infer_expr(arg, None);
                    }
                    return Type::Nothing;
                }
                // Resolve via internal lookup — not in the public registry free_fns.
                if let Some(sig) = self
                    .intrinsics
                    .lookup_free_fn_including_internal("__testFallibleAsync", call.args.len())
                {
                    let sig = sig.clone();
                    let ret = sig.ret.clone();
                    for (arg, expected_ty) in call.args.iter().zip(sig.params.iter()) {
                        let actual = self.infer_expr(arg, Some(expected_ty));
                        if actual != *expected_ty && actual != Type::Error {
                            self.diags.push(Diagnostic::error(
                                arg.span().clone(),
                                format!(
                                    "`__testFallibleAsync` argument type mismatch: \
                                     expected `{}`, got `{}`.",
                                    type_name(expected_ty),
                                    type_name(&actual)
                                ),
                                "Pass `true` or `false`.",
                                "`__testFallibleAsync` is an internal M2 test intrinsic.",
                            ));
                        }
                    }
                    ret
                } else {
                    self.diags.push(Diagnostic::error(
                        call.span.clone(),
                        format!(
                            "`__testFallibleAsync` takes 1 argument, got {}.",
                            call.args.len()
                        ),
                        "Write `wait __testFallibleAsync(true)`.",
                        "`__testFallibleAsync` is an internal M2 test intrinsic.",
                    ));
                    Type::Error
                }
            }
            // M8 P4: `sensitive(value)` constructor — wraps a string in Type::Sensitive.
            "sensitive" => {
                if call.args.len() != 1 {
                    self.diags.push(Diagnostic::error(
                        call.span.clone(),
                        format!("`sensitive` takes exactly one argument, got {}.", call.args.len()),
                        "Write `sensitive(`my secret value`)`.",
                        "`sensitive` marks a string value as sensitive so it auto-redacts in print output.",
                    ));
                    return Type::Error;
                }
                let arg_ty = self.infer_expr(&call.args[0], None);
                if arg_ty != Type::String && arg_ty != Type::Error {
                    self.diags.push(Diagnostic::error(
                        call.args[0].span().clone(),
                        format!(
                            "`sensitive` only wraps strings, not `{}`.",
                            type_name(&arg_ty)
                        ),
                        "Pass a string value: `sensitive(`my secret`)`.",
                        "Only string values can be marked sensitive in v0.1.",
                    ));
                    return Type::Error;
                }
                Type::Sensitive {
                    inner: Box::new(Type::String),
                }
            }
            // v0.3-M4: `channel<T>()` / `channel<T>(N)` — bounded task-communication channel
            // construction. A built-in generic constructor (like the `range(...)` free fn), NOT a
            // user function and NOT a `[[keyword]]` (P0 Lock 9). The suspending `.send()`/
            // `.receive()` method surface is Phase 2 (FRAGO 004) — Phase 1 ships only construction.
            "channel" => self.check_channel_construction(call),
            name => {
                // Non-generic user-defined function?
                if let Some(sig) = self.sig_table.fns.get(name) {
                    self.referenced_names.insert(name.to_string());
                    let params = sig.params.clone();
                    let ownerships = sig.param_ownerships.clone();
                    let ret = sig.ret.clone();
                    let callee_suspends = sig.suspends;

                    // Cross-module calls are resolved: `check_query` on the imported module
                    // sets `callee_suspends` correctly via the may-block fixpoint seeded by
                    // `imported_suspending_names`. No can't-infer error is emitted here —
                    // the call graph is fully traversable across module boundaries.

                    // Kernel-mode rejection for bare suspending calls. Every Yinz suspending
                    // call auto-suspends without an explicit `wait` keyword — the no-coloring
                    // model. The `wait`/`background`/`sleep` arms above each reject under kernel
                    // mode. This arm must also reject any suspending user-defined callee (bare
                    // auto-suspension form) to close the gap: a bare cross-module suspending
                    // call must not reach codegen under --kernel. Exactly ONE diagnostic per
                    // call site: when the call is under a `wait` that already rejected (the
                    // `Expr::Wait` arm returns early after emitting its error), this code is not
                    // reached, so there is no double-report.
                    if self.kernel_mode && callee_suspends {
                        self.diags.push(Diagnostic::error(
                            call.span.clone(),
                            format!("`{name}` suspends, which is not available in --kernel mode."),
                            format!("Remove the call to `{name}` or build without `--kernel`. Kernel-mode programs run without a scheduler runtime."),
                            "Suspension requires the thread-pool runtime, which does not run in kernel mode. See `design/no-runtime-mode.md` for the kernel-mode contract.",
                        ));
                    }

                    // Phase 6: explicit `wait` on a non-suspending callee — redundant hint.
                    // Uses the transitive `suspends` predicate (not the local `contains_wait`).
                    // Explicit `wait` on a suspending callee is valid-but-redundant; no warning.
                    if was_inside_wait && !callee_suspends && !is_base_suspension_intrinsic(name) {
                        let what_instead_args = params
                            .iter()
                            .map(|(n, _)| n.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        self.diags.push(wait_on_non_may_block_warning(
                            call.span.clone(),
                            name,
                            &format!("{name}({what_instead_args})"),
                        ));
                    }

                    // Phase 6: `wait_required_on_state_machine_call` is RETIRED under inference.
                    // Under the transitive analysis, every suspending caller is itself a state
                    // machine, and every suspending call is an inline-poll-yield — `wait` is
                    // never required. No replacement warning is needed.

                    let r = self.check_user_fn_call(call, name, &params, &ownerships, ret);
                    // M7 P3a: if the called function returns ErrorsCapable, handle context.
                    self.inside_wait = was_inside_wait;
                    self.inside_background = was_inside_background;
                    return self.handle_errors_capable_call_result(r, name, call.span.clone());
                }
                // Generic user-defined function?
                if let Some(sig) = self.generic_fn_table.fns.get(name) {
                    let sig = sig.clone();
                    self.inside_wait = was_inside_wait;
                    self.inside_background = was_inside_background;
                    return self.check_generic_fn_call(call, name, &sig);
                }
                // Unknown
                let mut candidates: Vec<&str> = self.sig_table.all_names();
                candidates.extend(self.generic_fn_table.all_names());
                candidates.extend(["print", "range", "sleepBlocking", "sleep"]);
                self.diags.push(make_not_defined_diag(
                    name,
                    call.callee.span().clone(),
                    &candidates,
                    format!("Define `{name}` as a function or check the spelling."),
                    &format!(
                        "`{name}` is not defined as a function in this file or its imports. \
                         Add `function {name}(...)` or import it from another file."
                    ),
                ));
                for arg in &call.args {
                    self.infer_expr(arg, None);
                }
                Type::Error
            }
        };
        self.inside_wait = was_inside_wait;
        self.inside_background = was_inside_background;
        result
    }

    /// Typecheck a `channel<T>()` / `channel<T>(N)` construction (v0.3-M4 Phase 1).
    ///
    /// - Exactly one type argument (the element type `T`).
    /// - Zero args → default capacity 64 (the P0-locked constant); one arg → an `int` capacity.
    /// - A non-positive integer LITERAL capacity is rejected at compile time (bounded by
    ///   construction, no unbounded constructor — stdlib-design Rule 4). A dynamic (non-literal)
    ///   int capacity is allowed; the runtime shim clamps `< 1 → 1` as a defensive floor.
    /// - Kernel-mode gate (R7): channel construction is a COMPILE ERROR under `--kernel` (the
    ///   scheduler runtime the channel needs does not run there), matching the `wait`/`background`
    ///   gates.
    ///
    /// Returns `Type::BuiltinChannel { elem }`. The suspending `.send()`/`.receive()` method
    /// surface is Phase 2 (FRAGO 004) — this Phase-1 path is construction only.
    /// v0.3-M4 Phase 2: mark `expr` (and its direct `wait` inner) as an allowed ROOT
    /// position for a suspending conduit-method call. Called by `check_stmts`/`check_let`
    /// for bare-expression statements and `let` values.
    fn record_conduit_root(&mut self, expr: &Expr) {
        let span = expr.span();
        self.conduit_root_spans.insert((span.start, span.end));
        if let Expr::Wait(inner, _) = expr {
            let ispan = inner.span();
            self.conduit_root_spans.insert((ispan.start, ispan.end));
        }
    }

    /// v0.3-M4 Phase 2: typecheck a suspending conduit-method call — `.send()`/`.receive()`
    /// on a `channel<T>` value or a background task handle (Lock 8: `.send()` is
    /// `-> nothing errors`; a dropped/closed receiver yields a TYPED channel-closed error).
    ///
    /// Enforced discipline (keeps the may-block fixpoint's syntactic resolver equivalent to
    /// this exact type-keyed view, and keeps the suspension codegen surface well-defined):
    /// - receiver must be a plain identifier (a named `let` binding or parameter);
    /// - the receiver binding's conduit-ness must be syntactically DERIVABLE — accumulated
    ///   in `derivable_conduits` via the shared `may_block::let_binds_derivable_conduit`
    ///   predicate (a channel from a shape field / collection element / loop variable /
    ///   cross-module call needs a `channel<T>` annotation on its binding);
    /// - the call must be its own statement (`ch.send(v)`) or a `let` value
    ///   (`let x = ch.receive()`) — never nested inside a larger expression.
    fn check_conduit_method_call(
        &mut self,
        receiver_ty: &Type,
        receiver: &Expr,
        method: &str,
        method_span: &SourceSpan,
        args: &[Expr],
        call_span: &SourceSpan,
    ) -> Type {
        let receiver_display = type_name(receiver_ty);

        // Kernel-mode gate (R7): matches the channel-construction gate. Construction is
        // already rejected under --kernel, but a `channel<T>` PARAMETER type-checks — the
        // method surface must not slip through.
        if self.kernel_mode {
            self.diags.push(Diagnostic::error(
                call_span.clone(),
                format!(
                    "`.{method}()` on a `{receiver_display}` is not available in --kernel mode."
                ),
                "Remove the channel operation, or build without `--kernel`.",
                "Channel operations suspend the calling task, which requires the thread-pool \
                 runtime started by `ynz_rt_init` — that runtime does not run in kernel mode. \
                 See `IMP-no-runtime-mode.md` for the kernel-mode contract.",
            ));
            return Type::Error;
        }

        // Method-name check first (unknown methods shouldn't trip the position rules).
        // The known-method set IS the authoritative suspending-method set (every conduit
        // method suspends in v0.3) — threaded from `suspension_source`, never a re-derived
        // local list (authoritative-derivation.md). If a future non-suspending conduit
        // method ships (e.g. `.tryReceive()`), extend THIS site to union it in explicitly.
        let known = match receiver_ty {
            Type::BuiltinChannel { .. } | Type::BackgroundHandle { .. } => {
                crate::suspension_source::channel_method_suspends(true, method)
            }
            _ => false,
        };
        if !known {
            self.diags.push(Diagnostic::error(
                method_span.clone(),
                format!("`{receiver_display}` does not have a method called `{method}`."),
                "Available methods: send(value), receive().",
                "Channels and task handles carry values between tasks: `send(value)` puts a \
                 value in (suspending when the buffer is full — backpressure), `receive()` \
                 takes the next value out (suspending until one arrives).",
            ));
            return Type::Error;
        }

        // Receiver discipline: plain identifier only.
        let Expr::Ident(receiver_name, _) = receiver else {
            self.diags.push(Diagnostic::error(
                call_span.clone(),
                format!(
                    "`.{method}()` needs the {} held in a named binding.",
                    match receiver_ty {
                        Type::BackgroundHandle { .. } => "task handle",
                        _ => "channel",
                    }
                ),
                format!(
                    "Bind it first: `let ch = ...` then `ch.{method}(...)` as its own statement."
                ),
                "A channel operation can suspend this function and resume it later. The \
                 compiler saves named bindings across that suspension; an unnamed in-flight \
                 value has nowhere to live while the function is suspended.",
            ));
            return Type::Error;
        };

        // Receiver ORIGIN discipline: the binding's conduit-ness must be syntactically
        // derivable — i.e. present in `derivable_conduits`, which is accumulated with THE
        // same shared predicate the may-block resolver uses. Without this check, a channel
        // reaching a binding through a shape field, a collection element, a loop variable,
        // or a cross-module call type-checks here but never enters the resolver's conduit
        // set — the function misses the suspend set and codegen ICEs on the conduit call
        // ("unknown method `receive` on BuiltinChannel"). This rejection is what makes the
        // "fixpoint can never under-approximate what typeck accepted" invariant
        // (suspension_source.rs) actually hold.
        if !self.derivable_conduits.contains(receiver_name.as_str()) {
            match receiver_ty {
                Type::BackgroundHandle { .. } => {
                    self.diags.push(Diagnostic::error(
                        call_span.clone(),
                        format!(
                            "`.{method}()` can't trace `{receiver_name}` back to a `background` spawn."
                        ),
                        format!(
                            "Call `.{method}()` on the binding created at the spawn — \
                             `let h = background f(...)` — or on a direct alias of it \
                             (`let h2 = h`)."
                        ),
                        "A task-handle operation can suspend this function and resume it \
                         later. The compiler decides which functions need that \
                         suspend-and-resume support before the rest of type checking runs, \
                         by tracing each handle straight back to its `background` spawn — a \
                         handle that arrives any other way is invisible at that stage.",
                    ));
                }
                _ => {
                    self.diags.push(Diagnostic::error(
                        call_span.clone(),
                        format!(
                            "`.{method}()` can't see where `{receiver_name}` got its channel \
                             — its declaration doesn't show a channel type."
                        ),
                        format!(
                            "Bind it with the type written out — `let ch: channel<T> = ...` \
                             — then call `ch.{method}(...)`."
                        ),
                        "A channel operation can suspend this function and resume it later. \
                         The compiler decides which functions need that suspend-and-resume \
                         support by reading each binding's declared type before the rest of \
                         type checking runs — a channel that arrives through a shape field, \
                         a collection element, a loop, or another module's function is \
                         invisible at that stage until the binding names its channel type.",
                    ));
                }
            }
            return Type::Error;
        }

        // Statement-position discipline: the call must be a statement root or a `let` value.
        let at_root = self
            .conduit_root_spans
            .contains(&(call_span.start, call_span.end));
        if !at_root {
            self.diags.push(Diagnostic::error(
                call_span.clone(),
                format!("`.{method}()` can suspend — it must be its own statement."),
                format!(
                    "Bind the value first:\n  let value = ch.{method}(...)\nthen use `value` in the larger expression."
                ),
                "A channel operation suspends this function when the channel is full (send) \
                 or empty (receive). The compiler resumes the function at a statement \
                 boundary, so the operation cannot sit inside a larger expression — the \
                 surrounding expression's partial results would not survive the suspension.",
            ));
            // Fall through to still return the correct type (limits error cascades).
        }

        match receiver_ty {
            Type::BuiltinChannel { elem } => match method {
                "send" => {
                    if args.len() != 1 {
                        self.diags.push(Diagnostic::error(
                            call_span.clone(),
                            format!(
                                "`.send()` takes exactly one argument, but got {}.",
                                args.len()
                            ),
                            format!(
                                "Send one value: `ch.send(value)` where value is `{}`.",
                                type_name(elem)
                            ),
                            "Each `send` puts one value into the channel's bounded buffer.",
                        ));
                        for a in args {
                            self.infer_expr(a, None);
                        }
                        return Type::Error;
                    }
                    let arg_ty = self.infer_expr(&args[0], Some(elem));
                    if arg_ty != **elem && arg_ty != Type::Error {
                        self.diags.push(Diagnostic::error(
                            args[0].span().clone(),
                            format!(
                                "This channel carries `{}` values, but you're sending `{}`.",
                                type_name(elem),
                                type_name(&arg_ty)
                            ),
                            format!(
                                "Send a `{}` value, or create a `channel<{}>` for this data.",
                                type_name(elem),
                                type_name(&arg_ty)
                            ),
                            "A channel's element type is fixed at construction so every \
                             receiver knows exactly what it gets. When the channel's buffer \
                             is full, `send` suspends this task until the receiver drains a \
                             slot — that is backpressure working, not a deadlock.",
                        ));
                    }
                    // Lock 8: `.send()` is `-> nothing errors` — a dropped/closed receiver
                    // yields a typed channel-closed error, never a silent drop.
                    Type::ErrorsCapable {
                        inner: Box::new(Type::Nothing),
                    }
                }
                "receive" => {
                    if !args.is_empty() {
                        self.diags.push(Diagnostic::error(
                            call_span.clone(),
                            format!("`.receive()` takes no arguments, but got {}.", args.len()),
                            "Call it bare: `let value = ch.receive()`.",
                            "`receive()` takes the next value out of the channel, suspending \
                             until one arrives.",
                        ));
                        for a in args {
                            self.infer_expr(a, None);
                        }
                    }
                    (**elem).clone()
                }
                _ => unreachable!("known-method check above"),
            },
            Type::BackgroundHandle { result, msg_elem } => match method {
                "receive" => {
                    if !args.is_empty() {
                        self.diags.push(Diagnostic::error(
                            call_span.clone(),
                            format!("`.receive()` takes no arguments, but got {}.", args.len()),
                            "Call it bare: `let value = h.receive()`.",
                            "`receive()` delivers the next thing from the task — a message \
                             reply, or the task's own completion value once it finishes.",
                        ));
                        for a in args {
                            self.infer_expr(a, None);
                        }
                    }
                    // ONE `.receive()` surface — typed `T errors`: the ok arm is the next
                    // delivery (message reply or completion value); the error arm is the
                    // task's own error, or task-already-finished.
                    Type::ErrorsCapable {
                        inner: result.clone(),
                    }
                }
                "send" => {
                    let Some(elem) = msg_elem else {
                        self.diags.push(Diagnostic::error(
                            call_span.clone(),
                            "This task takes no channel — it has no way to receive messages.",
                            "Add a `channel<T>` parameter to the task's function and pass a \
                             channel at the spawn: `let h = background worker(commands)` — \
                             `h.send(v)` then feeds that channel.",
                            "`h.send(v)` delivers into the FIRST `channel<T>` parameter of the \
                             spawned function, so the task can read messages with \
                             `commands.receive()` inside its own loop. A task whose function \
                             takes no channel never looks for messages, so sending to it \
                             would silently pile up values nobody reads.",
                        ));
                        for a in args {
                            self.infer_expr(a, None);
                        }
                        return Type::Error;
                    };
                    if args.len() != 1 {
                        self.diags.push(Diagnostic::error(
                            call_span.clone(),
                            format!(
                                "`.send()` takes exactly one argument, but got {}.",
                                args.len()
                            ),
                            format!(
                                "Send one value: `h.send(value)` where value is `{}`.",
                                type_name(elem)
                            ),
                            "Each `send` puts one value into the task's channel.",
                        ));
                        for a in args {
                            self.infer_expr(a, None);
                        }
                        return Type::Error;
                    }
                    let arg_ty = self.infer_expr(&args[0], Some(elem));
                    if arg_ty != **elem && arg_ty != Type::Error {
                        self.diags.push(Diagnostic::error(
                            args[0].span().clone(),
                            format!(
                                "This task's channel carries `{}` values, but you're sending `{}`.",
                                type_name(elem),
                                type_name(&arg_ty)
                            ),
                            format!("Send a `{}` value.", type_name(elem)),
                            "`h.send(v)` feeds the task function's first `channel<T>` \
                             parameter, so the value's type must match that channel's \
                             element type.",
                        ));
                    }
                    Type::ErrorsCapable {
                        inner: Box::new(Type::Nothing),
                    }
                }
                _ => unreachable!("known-method check above"),
            },
            _ => unreachable!("caller dispatches only conduit receivers"),
        }
    }

    fn check_channel_construction(&mut self, call: &CallExpr) -> Type {
        /// P0-locked default channel capacity. Re-tune is a one-constant change parked with a
        /// trigger (real workload evidence) — see the plan's Future Requirements.
        const DEFAULT_CHANNEL_CAPACITY: i64 = 64;

        // Resolve the element type from the required single type argument.
        let elem = match &call.type_args {
            Some(args) if args.len() == 1 => self.ast_type_to_type(&args[0]),
            Some(args) => {
                self.diags.push(Diagnostic::error(
                    call.span.clone(),
                    format!(
                        "`channel<T>` takes exactly one type argument — the element type — but got {}.",
                        args.len()
                    ),
                    "Write `channel<int>()` or `channel<Order>(32)`.",
                    "A channel carries values of a single element type `T` between tasks.",
                ));
                for a in args {
                    let _ = self.ast_type_to_type(a);
                }
                Type::Error
            }
            None => {
                self.diags.push(Diagnostic::error(
                    call.span.clone(),
                    "`channel` needs an element type — write `channel<int>()`.",
                    "Add the element type in angle brackets: `channel<int>()` (default capacity 64) or `channel<int>(32)`.",
                    "A channel carries values of a single element type `T`; the compiler needs to know `T` to check `send`/`receive` later.",
                ));
                Type::Error
            }
        };

        // v0.3-M4 Phase 2: the element type must survive crossing a task boundary. Values
        // travel through the channel as one 64-bit slot: scalars by value (int, float,
        // boolean) and heap-stable pointers (string, array, map). A `shape` value or a
        // `number` is backed by SENDER-STACK storage that is gone by the time the receiver
        // reads it — rejected until per-type heap-upgrade ships (mirrors the
        // UnsupportedCrossingLocalType discipline: a clean teaching error, never a silent
        // dangling read).
        let elem_supported = matches!(
            elem,
            Type::Error // already diagnosed upstream — don't cascade
                | Type::Int
                | Type::Float
                | Type::Bool
                | Type::String
                | Type::BuiltinArray { .. }
                | Type::BuiltinMap { .. }
        );
        if !elem_supported {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                format!(
                    "`channel<{}>` is not supported yet — this element type cannot cross a task boundary.",
                    type_name(&elem)
                ),
                "Use one of: int, float, boolean, string, array<T>, map<K, V>. For a shape, \
                 send its fields as separate values or as an array, and rebuild the shape on \
                 the receiving side.",
                "Channel values travel between tasks as a single 64-bit slot: numbers-by-value \
                 or a pointer to heap memory that both tasks can safely read. A `shape` or \
                 `number` value lives in the SENDING task's stack frame, which can be freed \
                 while the value still sits in the channel — the receiver would read freed \
                 memory. Per-type heap-copying for these ships in a later milestone.",
            ));
            return Type::Error;
        }

        // Kernel-mode gate (R7): no scheduler runtime in --kernel, so channels cannot exist there.
        if self.kernel_mode {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                "`channel<T>` is not available in --kernel mode.",
                "Remove the channel, or build without `--kernel`. Kernel-mode programs run without a scheduler runtime, so there are no tasks to communicate between.",
                "A `channel<T>` suspends a task on `send()`-when-full and `receive()`-when-empty, which requires the thread-pool runtime started by `ynz_rt_init` — that runtime does not run in kernel mode. See `IMP-no-runtime-mode.md` for the kernel-mode contract.",
            ));
        }

        // Capacity argument: 0 → default 64; 1 → an int (non-positive literal rejected); >1 → error.
        match call.args.len() {
            0 => {}
            1 => {
                let cap_ty = self.infer_expr(&call.args[0], Some(&Type::Int));
                if cap_ty != Type::Int && cap_ty != Type::Error {
                    self.diags.push(Diagnostic::error(
                        call.args[0].span().clone(),
                        format!(
                            "A channel's capacity must be an `int`, not `{}`.",
                            type_name(&cap_ty)
                        ),
                        "Pass a whole number of slots: `channel<int>(32)`.",
                        "The capacity is how many values the channel can hold before `send()` suspends the producer (backpressure), so it must be a count.",
                    ));
                }
                // Reject a non-positive literal capacity at compile time (no unbounded/empty
                // channel — stdlib-design Rule 4). Handles both `channel<int>(0)` (`IntLit`) and
                // `channel<int>(-5)` (unary-negated literal). A dynamic int capacity is allowed;
                // the runtime clamps `< 1 → 1` defensively.
                let literal_cap: Option<(i64, SourceSpan)> = match &call.args[0] {
                    Expr::IntLit(v, span) => Some((*v, span.clone())),
                    Expr::UnaryOp {
                        op: UnaryOpKind::Neg,
                        operand,
                        span,
                    } => match operand.as_ref() {
                        Expr::IntLit(v, _) => Some((v.wrapping_neg(), span.clone())),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some((v, span)) = literal_cap {
                    if v < 1 {
                        self.diags.push(Diagnostic::error(
                            span,
                            format!("A channel's capacity must be at least 1, but got {v}."),
                            "Use a positive capacity: `channel<int>(64)`. For very large buffering, pass a large explicit number — there is deliberately no unbounded channel.",
                            "A zero- or negative-capacity channel could never accept a value (or would be unbounded), so it is rejected. Bounded capacity is what makes `send()` apply backpressure instead of growing memory without limit.",
                        ));
                    }
                }
            }
            n => {
                self.diags.push(Diagnostic::error(
                    call.span.clone(),
                    format!("`channel<T>(...)` takes at most one argument — the capacity — but got {n}."),
                    "Write `channel<int>()` for the default capacity (64) or `channel<int>(32)` for an explicit capacity.",
                    "A channel is constructed with just its bounded capacity; values are added later with `send()`.",
                ));
                for a in &call.args {
                    let _ = self.infer_expr(a, None);
                }
            }
        }

        let _ = DEFAULT_CHANNEL_CAPACITY; // codegen supplies the literal 64 default; kept for docs.

        if matches!(elem, Type::Error) {
            Type::Error
        } else {
            Type::BuiltinChannel {
                elem: Box::new(elem),
            }
        }
    }

    fn check_print_call(&mut self, call: &CallExpr) -> Type {
        if call.args.len() != 1 {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                format!(
                    "`print` takes 1 argument, but {} were given.",
                    call.args.len()
                ),
                "Call it with one value: `print(value)`",
                "To display multiple values, use multiple `print` calls on separate lines.",
            ));
            for arg in &call.args {
                self.infer_expr(arg, None);
            }
            return Type::Error;
        }
        let arg_ty = self.infer_expr(&call.args[0], None);
        // Shapes are printable — the compiler emits a default "ShapeName { field: val, ... }"
        // representation. User-defined toString() can override this.
        // M8 P4: sensitive values are printable (they emit [REDACTED]).
        // M8 P6: all number<N> precisions (including bignum) are printable.
        let is_printable = self.intrinsics.is_print_type(&arg_ty)
            || matches!(
                &arg_ty,
                Type::Shape { .. }
                    | Type::BuiltinArray { .. }
                    | Type::Sensitive { .. }
                    | Type::Number { .. }
            );
        if arg_ty != Type::Error && !is_printable {
            // M7 P3a: give a more helpful diagnostic for ErrorsCapable values.
            if let Type::ErrorsCapable { .. } = &arg_ty {
                self.diags.push(Diagnostic::error(
                    call.args[0].span().clone(),
                    "This function can fail, but the failure is not handled here.",
                    "Three options: (1) Mark this function `-> T errors` to pass failures up. (2) Use `.or(default)` for a fallback. (3) Check `.failed()` explicitly.",
                    "When a function can fail, the failure must be handled somewhere. The compiler enforces this so failures can't silently pass through.",
                ));
            } else {
                let what_instead = match &arg_ty {
                    Type::BuiltinArray { .. } | Type::BuiltinFixed { .. } => {
                        "Loop and print each element: `for (item in collection) { print(item) }`"
                            .to_string()
                    }
                    Type::BuiltinMap { .. } => {
                        "Loop and print each entry: `for ((k, v) in collection) { print(k) }`"
                            .to_string()
                    }
                    _ => "Convert it to a string first with `.toString()`.".to_string(),
                };
                self.diags.push(Diagnostic::error(
                    call.args[0].span().clone(),
                    format!(
                        "`print` cannot display a `{}` value directly.",
                        type_name(&arg_ty)
                    ),
                    what_instead,
                    "`print` works with: int, float, number, boolean, string, and any shape.",
                ));
            }
            return Type::Error;
        }
        Type::Nothing
    }

    fn check_range_call(&mut self, call: &CallExpr) -> Type {
        match call.args.len() {
            1 | 2 => {
                for (i, arg) in call.args.iter().enumerate() {
                    let ty = self.infer_expr(arg, Some(&Type::Int));
                    if ty != Type::Int && ty != Type::Error {
                        self.diags.push(Diagnostic::error(
                            arg.span().clone(),
                            format!("Argument {} of `range` must be `int`, but got `{}`.", i + 1, type_name(&ty)),
                            "Pass an `int` value, e.g. `range(0, 10)`.",
                            "`range` produces integer sequences — its start and end must both be `int`.",
                        ));
                    }
                }
                Type::Range {
                    element: Box::new(Type::Int),
                    end_inclusive: false,
                }
            }
            n => {
                self.diags.push(Diagnostic::error(
                    call.span.clone(),
                    format!("`range` takes 1 or 2 arguments, but {} were given.", n),
                    "Use `range(end)` for 0..end or `range(start, end)` for start..end.",
                    "`range(end)` counts from 0 up to (but not including) end. `range(start, end)` starts at a specific value.",
                ));
                for arg in &call.args {
                    self.infer_expr(arg, None);
                }
                Type::Error
            }
        }
    }

    fn check_sleep_blocking_call(&mut self, call: &CallExpr) -> Type {
        if call.args.len() != 1 {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                format!(
                    "`sleepBlocking` takes exactly 1 argument, but {} were given.",
                    call.args.len()
                ),
                "Write `sleepBlocking(200)` — pass the number of milliseconds to sleep.",
                "`sleepBlocking` pauses the current thread for the given number of milliseconds. \
                 It takes one `int` argument.",
            ));
            for arg in &call.args {
                self.infer_expr(arg, None);
            }
            return Type::Nothing;
        }
        let ty = self.infer_expr(&call.args[0], Some(&Type::Int));
        if ty != Type::Int && ty != Type::Error {
            self.diags.push(Diagnostic::error(
                call.args[0].span().clone(),
                format!("`sleepBlocking` requires an `int` argument, but got `{}`.", type_name(&ty)),
                "Pass an integer number of milliseconds: `sleepBlocking(200)`.",
                "`sleepBlocking` converts the argument to a millisecond duration. Only `int` is accepted.",
            ));
        }
        Type::Nothing
    }

    /// Validate `sleep(ms)` call argument shape — non-blocking sleep for `wait` expressions.
    ///
    /// Accepts exactly one `int` argument. Returns `nothing` (the sleep completes silently).
    /// The kernel-mode rejection is handled by the dispatch arm in `check_call` before this
    /// helper is called.
    fn check_sleep_call(&mut self, call: &CallExpr) -> Type {
        if call.args.len() != 1 {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                format!(
                    "`sleep` takes exactly 1 argument, but {} were given.",
                    call.args.len()
                ),
                "Write `wait sleep(200)` — pass the number of milliseconds to pause.",
                "`sleep` suspends the calling function for the given number of milliseconds \
                 without blocking the OS thread. It takes one `int` argument.",
            ));
            for arg in &call.args {
                self.infer_expr(arg, None);
            }
            return Type::Nothing;
        }
        let ty = self.infer_expr(&call.args[0], Some(&Type::Int));
        if ty != Type::Int && ty != Type::Error {
            self.diags.push(Diagnostic::error(
                call.args[0].span().clone(),
                format!(
                    "`sleep` requires an `int` argument, but got `{}`.",
                    type_name(&ty)
                ),
                "Pass an integer number of milliseconds: `wait sleep(200)`.",
                "`sleep` converts the argument to a millisecond duration. Only `int` is accepted.",
            ));
        }
        Type::Nothing
    }

    /// Check ownership constraints when a binding is passed to a function parameter.
    ///
    /// Called from BOTH the UFCS dot-call path AND the regular function-call path to ensure
    /// the same ownership rules apply and the same diagnostic text is produced in both cases.
    /// Per `design/ide-hints.md` shared-wording rule, the error text must be byte-identical
    /// between the two call forms (e.g., `p.heal(20)` and `heal(p, 20)` produce the same error).
    ///
    /// Time: O(1) scope lookup.  Space: O(1).
    fn check_arg_ownership(
        &mut self,
        binding_name: &str,
        ownership: Option<&ynz_ast::nodes::OwnershipModifier>,
        fn_name: &str,
        arg_span: &SourceSpan,
    ) {
        match ownership {
            Some(ynz_ast::nodes::OwnershipModifier::Give) => {
                if let Some(entry) = self.scope.lookup(binding_name) {
                    if entry.is_const {
                        self.diags.push(Diagnostic::error(
                            arg_span.clone(),
                            format!("`{binding_name}` is `const` and cannot be given away."),
                            format!("Declare `{binding_name}` with `let` if you need to transfer ownership."),
                            "`const` bindings are fully read-only — the compiler cannot transfer ownership of a value that may not change.",
                        ));
                    } else if entry.param_ownership
                        == Some(ynz_ast::nodes::OwnershipModifier::Share)
                    {
                        self.diags.push(Diagnostic::error(
                            arg_span.clone(),
                            format!("`{binding_name}` is declared `share` (read-only); `{fn_name}` needs to take ownership of it (`give`)."),
                            format!("Declare `{binding_name}` as `give` to pass it here."),
                            "A `share` parameter is a read-only borrow — the caller still owns the value and trusts it is unchanged after the call. A function that takes ownership of a value would consume it, which a read-only borrow does not permit.",
                        ));
                    } else if !entry.is_consumed {
                        self.scope.consume(binding_name);
                    }
                }
            }
            Some(ynz_ast::nodes::OwnershipModifier::Lend) => {
                if let Some(entry) = self.scope.lookup(binding_name) {
                    if entry.is_const {
                        self.diags.push(Diagnostic::error(
                            arg_span.clone(),
                            format!("`{binding_name}` is `const` — `{fn_name}` needs to mutate it but `const` blocks mutation."),
                            format!("Declare `{binding_name}` with `let` if you need `{fn_name}` to modify it."),
                            "`const` bindings cannot be lent for mutation. The `lend` modifier means the function will write to the value.",
                        ));
                    } else if entry.param_ownership
                        == Some(ynz_ast::nodes::OwnershipModifier::Share)
                    {
                        // share→lend escalation (`design/concurrency.md` line 651): a function
                        // that receives a value as `share` (read-only) cannot lend it mutably
                        // to a callee. This is the load-bearing auto-parallel soundness rule.
                        self.diags.push(Diagnostic::error(
                            arg_span.clone(),
                            format!("`{binding_name}` is declared `share` (read-only); `{fn_name}` needs to modify it (`lend`)."),
                            format!("Declare `{binding_name}` as `lend` to pass it here."),
                            "A `share` parameter is a read-only borrow — the caller keeps ownership and trusts the value is unchanged after the call. Passing it where the value will be modified would break that promise; declare `lend` so the change is visible at every call site.",
                        ));
                    }
                }
            }
            _ => {} // share or unspecified: no restrictions
        }
    }

    fn check_user_fn_call(
        &mut self,
        call: &CallExpr,
        name: &str,
        params: &[(String, Type)],
        ownerships: &[Option<ynz_ast::nodes::OwnershipModifier>],
        ret: Type,
    ) -> Type {
        if call.args.len() != params.len() {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                format!(
                    "`{name}` takes {} argument(s), but {} were given.",
                    params.len(),
                    call.args.len()
                ),
                format!("Call it with {} argument(s).", params.len()),
                "Every function call must match the number of arguments the function declares.",
            ));
            for arg in &call.args {
                self.infer_expr(arg, None);
            }
            return Type::Error;
        }
        for (i, (arg, (_, expected_ty))) in call.args.iter().zip(params.iter()).enumerate() {
            let ownership = ownerships.get(i).and_then(|o| o.as_ref());
            let actual_ty = self.infer_expr(arg, Some(expected_ty));

            // Ownership enforcement on direct identifier arguments.
            if let Some(binding_name) = simple_ident_name(arg) {
                self.check_arg_ownership(binding_name, ownership, name, arg.span());
            }

            // Can't-infer: UFCS free-fn form of dynamic dispatch. When the EXPECTED param
            // type is `dynamic Contract` (the function explicitly takes a dynamic receiver)
            // AND the actual argument is also `dynamic Contract`, this is the free-fn
            // equivalent of `handler.method(args)` — both call forms must emit the error.
            // Note: passing a CONCRETE shape to a `dynamic Contract` param is valid (widening
            // coerce at the call site); only passing an ALREADY-dynamic value triggers this.
            // Gate on current_fn_suspends: only a caller that independently reaches a
            // suspension point needs this error; non-suspending callers treat dynamic calls
            // as non-suspending leaves per design/no-function-coloring.md:75.
            if let (
                Type::Dynamic {
                    contract: expected_contract,
                },
                Type::Dynamic {
                    contract: actual_contract,
                },
            ) = (expected_ty, &actual_ty)
            {
                if expected_contract == actual_contract && self.current_fn_suspends {
                    self.diags.push(Diagnostic::error(
                        arg.span().clone(),
                        format!(
                            "Can't determine whether `{name}` suspends — it's called with a \
                             `dynamic {expected_contract}` value whose concrete type is unknown at compile time."
                        ),
                        format!(
                            "Use a concrete type instead of `dynamic {expected_contract}`, or restructure \
                             so the call is statically resolvable from inside a suspending function."
                        ),
                        "Dynamic dispatch resolves the callee at runtime — the compiler \
                         can't determine its suspension status at compile time. \
                         v0.3-M2 requires static resolution for calls from suspending \
                         functions; dynamic dispatch support for suspending callees \
                         ships in a future version.",
                    ));
                }
            }

            // M7 P3c: range values are first-class — can be passed as function arguments.
            if let Type::ErrorsCapable { .. } = &actual_ty {
                // M7 P3a: give a more helpful diagnostic for ErrorsCapable values passed
                // to a function expecting the success type.
                if !matches!(expected_ty, Type::ErrorsCapable { .. }) {
                    self.diags.push(Diagnostic::error(
                        arg.span().clone(),
                        "This function can fail, but the failure is not handled here.",
                        "Three options: (1) Mark this function `-> T errors` to pass failures up. (2) Use `.or(default)` for a fallback. (3) Check `.failed()` explicitly.",
                        "When a function can fail, the failure must be handled somewhere. The compiler enforces this so failures can't silently pass through.",
                    ));
                }
            } else if actual_ty != Type::Error && !types_compatible(expected_ty, &actual_ty) {
                // Accept a concrete shape as a `dynamic Contract` argument when the
                // shape's declaration includes `follows Contract`.
                //
                // Both shapes and `dynamic` values are plain pointers in the LLVM ABI —
                // the coerce is a type-level widening only; no runtime fat-pointer packing
                // is needed at this call site.  Method dispatch through `d` inside the
                // callee uses the vtable at the call site where the concrete type is known.
                let is_valid_dyn_coerce = match (expected_ty, &actual_ty) {
                    (Type::Dynamic { contract }, Type::Shape { name: shape_name }) => self
                        .shape_table
                        .get(shape_name)
                        .map(|def| def.follows.contains(contract))
                        .unwrap_or(false),
                    _ => false,
                };

                if !is_valid_dyn_coerce {
                    self.diags.push(Diagnostic::error(
                        arg.span().clone(),
                        format!(
                            "This argument is `{}`, but `{name}` expects `{}` here.",
                            type_name(&actual_ty),
                            type_name(expected_ty)
                        ),
                        if let Type::Dynamic { contract } = expected_ty {
                            format!(
                                "Pass a shape that follows `{contract}` (add `follows {contract}` to the shape declaration)."
                            )
                        } else {
                            format!("Pass a `{}` value.", type_name(expected_ty))
                        },
                        if let Type::Dynamic { contract } = expected_ty {
                            format!(
                                "`{name}` declared this parameter as `dynamic {contract}`. Only shapes that declare `follows {contract}` can be passed here."
                            )
                        } else {
                            format!(
                                "`{name}` declared this parameter as `{}`. Passing a `{}` would be a type mismatch.",
                                type_name(expected_ty),
                                type_name(&actual_ty)
                            )
                        },
                    ));
                }
            }
        }
        // Reject Range return values (shouldn't be in sig_table, but guard anyway)
        if matches!(ret, Type::Range { .. }) {
            return Type::Error;
        }
        ret
    }

    fn check_binop(&mut self, op: &BinOpKind, lhs: &Type, rhs: &Type, span: &SourceSpan) -> Type {
        if *lhs == Type::Error || *rhs == Type::Error {
            return Type::Error;
        }

        use BinOpKind::*;
        match op {
            Add | Sub | Mul | Div => match (lhs, rhs) {
                (Type::Int, Type::Int) => Type::Int,
                (Type::Float, Type::Float) => Type::Float,
                // M8 P6: mixed-precision promotion — result precision = max(lhs, rhs).
                (Type::Number { precision: pa }, Type::Number { precision: pb }) => Type::Number {
                    precision: (*pa).max(*pb),
                },
                _ => {
                    self.emit_binop_mismatch(op, lhs, rhs, span);
                    Type::Error
                }
            },

            Rem => match (lhs, rhs) {
                (Type::Int, Type::Int) => Type::Int,
                (Type::Float, Type::Float) => Type::Float,
                (Type::Number { .. }, Type::Number { .. }) => {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        "The `%` (remainder) operator on `number` requires careful rounding semantics.",
                        "Use `int` instead of `number` if you want exact integer remainders, or write your own rounding-aware helper.",
                        "On decimal `number`, `%` (remainder) depends on which rounding mode is in effect — IEEE 754-2008 §5.3.1 defines remainder as `a − (round(a/b) × b)`, and different rounding modes (half-even, truncation, etc.) produce different results for the same inputs. Yinz refuses `%` on `number` to avoid the silent precision-loss class.",
                    ));
                    Type::Error
                }
                _ => {
                    self.emit_binop_mismatch(op, lhs, rhs, span);
                    Type::Error
                }
            },

            Lt | LtEq | Gt | GtEq => match (lhs, rhs) {
                (Type::Int, Type::Int)
                | (Type::Float, Type::Float)
                | (Type::Number { .. }, Type::Number { .. }) => Type::Bool,
                _ => {
                    self.emit_binop_mismatch(op, lhs, rhs, span);
                    Type::Error
                }
            },

            EqEq | NotEq => match (lhs, rhs) {
                (Type::Int, Type::Int)
                | (Type::Float, Type::Float)
                | (Type::Number { .. }, Type::Number { .. })
                | (Type::Bool, Type::Bool)
                | (Type::String, Type::String) => Type::Bool,
                // M6: same-options-type comparison is valid.
                (Type::Options { name: a }, Type::Options { name: b }) if a == b => Type::Bool,
                // M6: cross-options-type comparison is a compile error.
                (Type::Options { name: a }, Type::Options { name: b }) => {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!("Cannot compare `{a}` and `{b}` — they are different options types."),
                        "Compare values of the same options type, or convert both to the same type first.",
                        "Comparing values of different options types is almost always a bug — \
                         the tags have no shared meaning between types.",
                    ));
                    Type::Error
                }
                _ => {
                    self.emit_binop_mismatch(op, lhs, rhs, span);
                    Type::Error
                }
            },

            And | Or => match (lhs, rhs) {
                (Type::Bool, Type::Bool) => Type::Bool,
                _ => {
                    self.emit_binop_mismatch(op, lhs, rhs, span);
                    Type::Error
                }
            },

            BitAnd | BitOr | BitXor | Shl | Shr => match (lhs, rhs) {
                (Type::Int, Type::Int) => Type::Int,
                _ => {
                    self.emit_binop_mismatch(op, lhs, rhs, span);
                    Type::Error
                }
            },
        }
    }

    fn check_unaryop(&mut self, op: &UnaryOpKind, operand: &Type, span: &SourceSpan) -> Type {
        if *operand == Type::Error {
            return Type::Error;
        }
        match op {
            UnaryOpKind::Neg => {
                match operand {
                    Type::Int | Type::Float | Type::Number { .. } => operand.clone(),
                    other => {
                        self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!("Unary `-` cannot be used on a `{}` value.", type_name(other)),
                        "Unary `-` only works on `int`, `float`, and `number`.",
                        "Negation flips the sign of a number — it doesn't apply to other types.",
                    ));
                        Type::Error
                    }
                }
            }
            UnaryOpKind::Not => match operand {
                Type::Bool => Type::Bool,
                other => {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!("`!` cannot be used on a `{}` value.", type_name(other)),
                        "Use `!` only with `boolean` expressions.",
                        "`!` is the boolean NOT operator — it flips `true` to `false` and vice versa.",
                    ));
                    Type::Error
                }
            },
            UnaryOpKind::BitNot => match operand {
                Type::Int => Type::Int,
                other => {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!("`~` cannot be used on a `{}` value.", type_name(other)),
                        "Use `~` only with `int` values.",
                        "`~` flips every bit in the integer — it only makes sense for `int`.",
                    ));
                    Type::Error
                }
            },
        }
    }

    /// Type-check a call to a generic function, performing type inference and
    /// constraint checking, then recording the instantiation in the MonomorphizationTable.
    ///
    /// # Kernel-mode suspension
    ///
    /// No kernel-mode rejection guard here. `GenericFnSig` carries no `suspends` flag —
    /// Yinz generics currently cannot propagate suspension through type parameters because
    /// monomorphization is resolved at instantiation time, after the may-block fixpoint runs.
    /// The gap is vacuously safe today: no generic function in the current language surface
    /// can reach a suspension point through a type parameter.
    ///
    /// What: kernel-mode rejection for suspending generic instantiations.
    /// Why deferred: `GenericFnSig` has no `suspends` field; threading may-block analysis
    ///   through generic instantiation is a v0.4 generics-overhaul concern (~1 session of
    ///   work: add `pub suspends: bool` to `GenericFnSig`, propagate in may_block::analyze,
    ///   mirror the guard from the `Expr::Call name =>` arm at line ~2439 here).
    /// Cost if left unfixed: a generic function that reaches suspension would bypass the
    ///   kernel guard and reach codegen under --kernel; codegen would emit a runtime call
    ///   that panics because no scheduler is running.
    /// Trigger: any generic monomorphization where the instantiated call graph sets
    ///   `suspends=true` — requires v0.4 generic+suspension to be possible first.
    fn check_generic_fn_call(&mut self, call: &CallExpr, name: &str, sig: &GenericFnSig) -> Type {
        let non_self_params: Vec<(String, Type)> = sig
            .params
            .iter()
            .filter(|(p, _)| p != "self")
            .cloned()
            .collect();

        if call.args.len() != non_self_params.len() {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                format!(
                    "`{name}` takes {} argument(s), but {} were given.",
                    non_self_params.len(),
                    call.args.len()
                ),
                format!("Call it with {} argument(s).", non_self_params.len()),
                "Every function call must match the number of arguments the function declares.",
            ));
            for arg in &call.args {
                self.infer_expr(arg, None);
            }
            return Type::Error;
        }

        let mut subst: Substitution = HashMap::new();

        // Explicit type args (e.g. `foo<int>(x)`) seed the substitution directly.
        if let Some(type_args) = &call.type_args {
            for (tp_name, ast_ty) in sig.type_params.iter().zip(type_args.iter()) {
                let concrete = self.ast_type_to_type(ast_ty);
                subst.insert(tp_name.clone(), concrete);
            }
        }

        // Infer remaining type params from argument types; enforce ownership rules.
        // Ownerships aligned with non_self_params (skip the `self` entry if present).
        let skip = sig.params.len() - non_self_params.len();
        let non_self_ownerships: Vec<Option<ynz_ast::nodes::OwnershipModifier>> =
            sig.param_ownerships.iter().skip(skip).cloned().collect();
        let mut arg_types = Vec::new();
        for (i, (arg, (_, param_ty))) in call.args.iter().zip(non_self_params.iter()).enumerate() {
            let actual = self.infer_expr(arg, None);
            arg_types.push(actual.clone());
            if actual != Type::Error {
                let _ = unify_param(param_ty, &actual, &mut subst);
            }
            // Ownership enforcement via shared helper (same as check_user_fn_call and UFCS path).
            let ownership = non_self_ownerships.get(i).and_then(|o| o.as_ref());
            if let Some(binding_name) = simple_ident_name(arg) {
                self.check_arg_ownership(binding_name, ownership, name, arg.span());
            }
        }

        // Verify all type params were resolved — emit one consolidated error if multiple are missing.
        let unresolved: Vec<&String> = sig
            .type_params
            .iter()
            .filter(|tp| !subst.contains_key(*tp))
            .collect();
        if !unresolved.is_empty() {
            match unresolved.len() {
                1 => {
                    let tp_name = unresolved[0];
                    self.diags.push(Diagnostic::error(
                        call.span.clone(),
                        format!("Cannot work out the type parameter `{tp_name}` for function `{name}` — pass a value or annotate explicitly."),
                        format!("Examples: `{name}(5)` (T = int) or `{name}<int>()`"),
                        "Yinz figures out type parameters from the argument types. If there are no arguments, specify the type explicitly.",
                    ));
                }
                n => {
                    let list = unresolved
                        .iter()
                        .map(|tp| format!("`{tp}`"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.diags.push(Diagnostic::error(
                        call.span.clone(),
                        format!("{n} type parameters could not be resolved for `{name}`: {list}."),
                        format!("Annotate the call explicitly: `{name}<Type1, Type2>(...)` or pass typed arguments."),
                        "Yinz figures out type parameters from the argument types. If there are no arguments, specify all types explicitly.",
                    ));
                }
            }
            return Type::Error;
        }

        // Verify follows constraints.
        for (tp_name, contracts) in &sig.constraints {
            let Some(concrete_ty) = subst.get(tp_name) else {
                continue;
            };
            let concrete_ty = concrete_ty.clone();
            for contract_name in contracts {
                match &concrete_ty {
                    Type::Shape { name: shape_name } => {
                        let satisfies = self
                            .shape_table
                            .get(shape_name)
                            .map(|def| def.follows.contains(contract_name))
                            .unwrap_or(false);
                        if !satisfies {
                            self.diags.push(Diagnostic::error(
                                call.span.clone(),
                                format!("Type `{shape_name}` does not follow contract `{contract_name}`."),
                                format!("To use `{shape_name}` here, add `follows {contract_name}` to its declaration AND implement the required methods."),
                                format!("`{name}<{tp_name} follows {contract_name}>` requires the concrete type to satisfy the `{contract_name}` contract."),
                            ));
                            return Type::Error;
                        }
                    }
                    other => {
                        self.diags.push(Diagnostic::error(
                            call.span.clone(),
                            format!("Type `{}` does not follow contract `{contract_name}` — only shapes can follow contracts.", type_name(other)),
                            format!("Use a shape type for `{tp_name}`, or remove the `follows {contract_name}` constraint."),
                            "`follows` constraints can only be satisfied by user-defined shapes.",
                        ));
                        return Type::Error;
                    }
                }
            }
        }

        // Compute concrete return type.
        let concrete_ret = apply_substitution(&sig.ret, &subst);

        // Record the instantiation.
        let concrete_type_args: Vec<Type> = sig
            .type_params
            .iter()
            .map(|tp| subst.get(tp).cloned().unwrap_or(Type::Error))
            .collect();
        let concrete_params: Vec<Type> = non_self_params
            .iter()
            .map(|(_, ty)| apply_substitution(ty, &subst))
            .collect();
        self.mono_table.record(
            name.to_string(),
            concrete_type_args,
            MonoSignature {
                param_types: concrete_params,
                ret_type: concrete_ret.clone(),
            },
        );

        concrete_ret
    }

    /// Type-check a UFCS dot-call expression: `receiver.method(args)`.
    ///
    /// `receiver_expr` carries the original receiver `Expr` so the ownership
    /// check can inspect the binding name and enforce `const` constraints on
    /// the first parameter.  `None` is used for synthetic calls where no
    /// source receiver expression is available (e.g., intrinsic dispatch helpers).
    ///
    /// Time: O(sig lookup) on cache miss.  Space: O(1).
    fn check_method_call(
        &mut self,
        receiver_ty: &Type,
        receiver_expr: Option<&Expr>,
        method: &str,
        method_span: &SourceSpan,
    ) -> Type {
        if *receiver_ty == Type::Error {
            return Type::Error;
        }

        // M8 P4: sensitive type method dispatch.
        if let Type::Sensitive { inner } = receiver_ty {
            let inner = inner.as_ref().clone();
            return sensitive_method_return(method, &inner, method_span, &mut self.diags);
        }

        // M6: reject `.toInt()` on bool — no silent 0/1 coercion.
        if *receiver_ty == Type::Bool && method == "toInt" {
            self.diags.push(Diagnostic::error(
                method_span.clone(),
                "`.toInt()` is not available on `boolean`.",
                "Use an `if` expression instead: `if (b) { 1 } else { 0 }`",
                "Automatic bool-to-int coercion is a common source of bugs. \
                 Yinz requires an explicit conversion.",
            ));
            return Type::Error;
        }

        // Primitive intrinsic methods (M2/M3 — toString, toFloat, etc.)
        if let Some(ret_ty) = self.intrinsics.lookup_method(receiver_ty, method) {
            return ret_ty;
        }

        // M5 P3b: built-in collection method dispatch.
        if let Type::BuiltinArray { elem } = receiver_ty {
            let elem = elem.as_ref().clone();
            // An in-place mutator (`.add`/`.set`/`.remove`/…) writes the receiver — reject it
            // on a `share` parameter or `const` binding, the same as a direct element assign.
            if self.reject_mutating_collection_method(
                receiver_expr,
                method,
                "elements",
                array_method_is_mutating(method),
                method_span,
            ) {
                return Type::Nothing;
            }
            return if let Some(ret) = array_method_return(method, &elem) {
                ret
            } else {
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!("`array<{}>` does not have a method called `{method}`.", type_name(&elem)),
                    "Available methods: add, remove, get, set, count, first, last, contains, sort, filter, find, map, concat.",
                    "These are the built-in methods on `array<T>`. Check the spelling.",
                ));
                Type::Error
            };
        }
        if let Type::BuiltinFixed { elem, .. } = receiver_ty {
            let elem = elem.as_ref().clone();
            if self.reject_mutating_collection_method(
                receiver_expr,
                method,
                "elements",
                fixed_method_is_mutating(method),
                method_span,
            ) {
                return Type::Nothing;
            }
            return if let Some(ret) = fixed_method_return(method, &elem) {
                ret
            } else {
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!("`fixed<{}>` does not have a method called `{method}`.", type_name(&elem)),
                    "Available methods: get, set, count, first, last, contains, sort, filter, find, concat.",
                    "`fixed<T>` is a size-locked array — it does not have `.add()` or `.remove()`. Use `array<T>` for growable collections.",
                ));
                Type::Error
            };
        }
        if let Type::Maybe { inner } = receiver_ty {
            let inner = inner.as_ref().clone();
            return if let Some(ret) = maybe_method_return(method, &inner) {
                ret
            } else {
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!("`maybe<{}>` does not have a method called `{method}`.", type_name(&inner)),
                    "Available methods: exists(), or(default).",
                    "`maybe<T>` values can only be checked with `.exists()` or given a default with `.or(default)`. Access the value with `.value` (after checking `.exists()`).",
                ));
                Type::Error
            };
        }
        if let Type::BuiltinMap { key, val } = receiver_ty {
            let key = key.as_ref().clone();
            let val = val.as_ref().clone();
            if self.reject_mutating_collection_method(
                receiver_expr,
                method,
                "entries",
                map_method_is_mutating(method),
                method_span,
            ) {
                return Type::Nothing;
            }
            return if let Some(ret) = map_method_return(method, &key, &val) {
                ret
            } else {
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!(
                        "`map<{}, {}>` does not have a method called `{method}`.",
                        type_name(&key),
                        type_name(&val)
                    ),
                    "Available methods: get, set, has, remove, count, keys, values, entries.",
                    "Check the spelling. Use `m[key]` for reads and `m[key] = value` for writes.",
                ));
                Type::Error
            };
        }
        if let Type::MapEntry { key, val } = receiver_ty {
            let key = key.as_ref().clone();
            let val = val.as_ref().clone();
            self.diags.push(Diagnostic::error(
                method_span.clone(),
                format!("`MapEntry<{}, {}>` does not have a method called `{method}`.", type_name(&key), type_name(&val)),
                "Use `entry.key` to get the key and `entry.value` to get the value.",
                "`MapEntry` values only have two fields: `.key` and `.value`. They have no methods.",
            ));
            return Type::Error;
        }

        // Shape or dynamic receiver — try UFCS.
        if let Type::Dynamic { contract } = receiver_ty {
            // Dynamic dispatch: look up the method on the contract shape's sigs.
            if let Some(shape_def) = self.shape_table.get(contract) {
                if let Some(sig) = shape_def.contract_sigs.iter().find(|s| s.name == method) {
                    // Can't-infer: dynamic dispatch through a vtable from a suspending caller.
                    // The concrete callee is unknown at compile time so its suspension status
                    // cannot be determined. Gate on current_fn_suspends: only callers that
                    // independently reach a suspension point (intra-unit `sleep`) get the
                    // error; non-suspending callers treat this as a non-suspending leaf per
                    // design/no-function-coloring.md:75 (the M2 intentional under-approximation).
                    if self.current_fn_suspends {
                        self.diags.push(Diagnostic::error(
                            method_span.clone(),
                            format!(
                                "Can't determine whether `{method}` suspends — it's a \
                                 dynamic-dispatch call through a `dynamic {contract}` vtable."
                            ),
                            format!(
                                "Make the boundary explicit: use a concrete type instead of \
                                 `dynamic {contract}`, or restructure so the call is statically \
                                 resolvable from inside a suspending function."
                            ),
                            "Dynamic dispatch resolves the callee at runtime — the compiler \
                             can't determine its suspension status at compile time. \
                             v0.3-M2 requires static resolution for calls from suspending \
                             functions; dynamic dispatch support for suspending callees \
                             ships in a future version.",
                        ));
                    }
                    return sig.ret_ty.clone();
                }
            }
            self.diags.push(Diagnostic::error(
                method_span.clone(),
                format!("Contract `{contract}` does not declare a method `{method}`."),
                format!("Add `{method}(share self) -> ...` to the `shape {contract}` body."),
                "Dynamic dispatch only routes calls to methods declared in the contract shape's body.",
            ));
            return Type::Error;
        }
        if let Type::Shape { name } = receiver_ty {
            if let Some(sig) = self.sig_table.fns.get(method) {
                // Check that first param type matches receiver
                if let Some((_, first_ty)) = sig.params.first() {
                    if first_ty == receiver_ty || *first_ty == Type::Error {
                        self.referenced_names.insert(method.to_string());
                        // Receiver ownership check via the shared helper — called from BOTH
                        // this UFCS dot-call path AND the regular function-call arg loop so
                        // the diagnostic text is byte-identical between `p.heal(20)` and
                        // `heal(p, 20)` per design/ide-hints.md shared-wording rule.
                        let receiver_ownership =
                            sig.param_ownerships.first().and_then(|o| o.as_ref());
                        if let Some(recv_expr) = receiver_expr {
                            if let Some(binding_name) = simple_ident_name(recv_expr) {
                                self.check_arg_ownership(
                                    binding_name,
                                    receiver_ownership,
                                    method,
                                    recv_expr.span(),
                                );
                            }
                        }
                        // Kernel-mode rejection for UFCS suspending method calls. The
                        // bare call-dispatch arm guards `Expr::Call name =>` at the call site;
                        // UFCS calls route through this path and need the same guard so that
                        // `player.longJob()` (sugar for `longJob(player)`) is also rejected
                        // under --kernel when the resolved function suspends.
                        if self.kernel_mode && sig.suspends {
                            self.diags.push(Diagnostic::error(
                                method_span.clone(),
                                format!("`{method}` suspends, which is not available in --kernel mode."),
                                format!("Remove the call to `{method}` or build without `--kernel`. Kernel-mode programs run without a scheduler runtime."),
                                "Suspension requires the thread-pool runtime, which does not run in kernel mode. See `design/no-runtime-mode.md` for the kernel-mode contract.",
                            ));
                            return sig.ret.clone();
                        }
                        return sig.ret.clone();
                    }
                }
                // Function exists but first param doesn't match
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!("No function `{method}` takes a `{name}` as its first argument."),
                    format!("Define `function {method}(share self: {name}) -> ...` to call it as `value.{method}()`."),
                    "In Yinz, `value.method()` is sugar for `method(value)` — the function's first parameter must match the receiver's type.",
                ));
                return Type::Error;
            }
            // No function named `method` at all
            self.diags.push(Diagnostic::error(
                method_span.clone(),
                format!("No function `{method}` is defined for `{name}` values."),
                format!("Define `function {method}(share self: {name}) -> ...` then call it as `value.{method}()`."),
                "In Yinz, `value.method()` is sugar for `method(value)` (UFCS). Both call forms work — define the function first.",
            ));
            return Type::Error;
        }

        // M7 P3a: errors-capable value method dispatch.
        if let Type::ErrorsCapable { inner } = receiver_ty {
            let inner = inner.as_ref().clone();
            return self.check_errors_capable_method(method, method_span, &inner);
        }

        // M6: options type method dispatch.
        if let Type::Options { name: opts_name } = receiver_ty {
            return match method {
                "toString" => Type::String,
                other => {
                    self.diags.push(Diagnostic::error(
                        method_span.clone(),
                        format!("`{opts_name}` does not have a method called `{other}`."),
                        "Options types only have `.toString()` as a built-in method.",
                        "Method calls are checked at compile time. Only `.toString()` exists on options values.",
                    ));
                    Type::Error
                }
            };
        }

        // M7 P3b: string method dispatch.
        if receiver_ty == &Type::String {
            return if let Some(ret) = string_method_return(method) {
                ret
            } else {
                let available_list = ynz_registry::primitive_intrinsics()
                    .filter(|e| e.receiver_type == Some("string"))
                    .map(|e| e.name)
                    .collect::<Vec<_>>()
                    .join(", ");
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!("`string` does not have a method called `{method}`."),
                    format!("Available string methods: {available_list}."),
                    "Check the spelling. String methods are fixed built-ins — they cannot be extended.",
                ));
                Type::Error
            };
        }

        // Primitive type with unknown method
        let available = self.intrinsics.methods_for_type(receiver_ty);
        let what_instead = if available.is_empty() {
            format!("`{}` has no built-in methods.", type_name(receiver_ty))
        } else {
            format!(
                "Available on `{}`: {}",
                type_name(receiver_ty),
                available.join(", ")
            )
        };
        self.diags.push(Diagnostic::error(
            method_span.clone(),
            format!(
                "`{}` does not have a method called `{method}`.",
                type_name(receiver_ty)
            ),
            what_instead,
            "Method calls are checked at compile time. Only the listed methods exist on this type.",
        ));
        Type::Error
    }

    /// M7 P3a: type-check a method call on an `errors`-capable value.
    ///
    /// Available methods: `.failed()`, `.or(default)`, `.message`, `.suggestions`,
    /// `.trace`, `.source`. All other method names are a compile error.
    fn check_errors_capable_method(
        &mut self,
        method: &str,
        method_span: &SourceSpan,
        inner: &Type,
    ) -> Type {
        match method {
            "failed" => {
                // .failed() returns bool. Records that the check happened.
                Type::Bool
            }
            "or" => {
                // .or(default) — returns the success type. Arg checking happens
                // at the call site where args are inferred; here return inner.
                inner.clone()
            }
            "message" => Type::String,
            "suggestions" => Type::BuiltinArray {
                elem: Box::new(Type::String),
            },
            "trace" => {
                // trace returns array<Frame> — Frame is a compiler-synthesized shape.
                Type::BuiltinArray {
                    elem: Box::new(Type::Shape {
                        name: "Frame".into(),
                    }),
                }
            }
            "source" => {
                // source returns SourceLoc — a compiler-synthesized shape.
                Type::Shape {
                    name: "SourceLoc".into(),
                }
            }
            other => {
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!("An `errors`-capable value does not have a method called `{other}`."),
                    "Available methods: failed(), or(default), message, suggestions, trace, source.",
                    "An `errors`-capable value is the output of a call that can fail. Check `.failed()` first, then use the success value directly.",
                ));
                Type::Error
            }
        }
    }

    /// M7 P3a: after calling a function that returns `ErrorsCapable`, return the
    /// `ErrorsCapable` type as-is regardless of context.
    ///
    /// The caller must handle the `ErrorsCapable` type — either by chaining `.or()` /
    /// `.failed()` immediately (method dispatch handles it), or by storing in a binding
    /// (the binding carries the `ErrorsCapable` type). When the binding is later used
    /// as a success value in a non-errors function, `resolve_ident` emits the diagnostic.
    fn handle_errors_capable_call_result(
        &mut self,
        result: Type,
        _fn_name: &str,
        _call_span: SourceSpan,
    ) -> Type {
        // Always return the ErrorsCapable type — method chaining (.or, .failed)
        // and resolve_ident (for binding uses) handle the diagnostic responsibility.
        result
    }

    fn ast_type_to_type(&mut self, ast_ty: &AstType) -> Type {
        match ast_ty {
            AstType::Nothing => Type::Nothing,
            AstType::Int => Type::Int,
            AstType::Float => Type::Float,
            AstType::Number { precision } => Type::Number {
                precision: *precision,
            },
            AstType::Bool => Type::Bool,
            AstType::Error => Type::Error,
            AstType::Named(n, _) if n == "string" => Type::String,
            // Type param names resolve to TypeParam when inside a generic context.
            AstType::Named(n, _) if self.type_param_scope.contains_key(n) => {
                Type::TypeParam { name: n.clone() }
            }
            // M6: union type aliases resolve to the full union type.
            AstType::Named(n, _) if self.union_aliases.contains_key(n) => {
                self.union_aliases[n].clone()
            }
            // M6: options type names resolve to Type::Options.
            AstType::Named(n, _) if self.options_table.contains(n) => {
                self.referenced_names.insert(n.clone());
                Type::Options { name: n.clone() }
            }
            AstType::Named(n, _) if self.shape_table.contains(n) => {
                self.referenced_names.insert(n.clone());
                Type::Shape { name: n.clone() }
            }
            // M7 P3c: built-in compiler-synthesized types — always recognized.
            AstType::Named(n, _) if matches!(n.as_str(), "Frame" | "SourceLoc") => {
                Type::Shape { name: n.clone() }
            }
            // M7 P3c: first-class range type — `range` as a type annotation.
            AstType::Named(n, _) if n == "range" => Type::Range {
                element: Box::new(Type::Int),
                end_inclusive: false,
            },
            AstType::Named(n, span)
                if matches!(
                    n.as_str(),
                    "array" | "fixed" | "maybe" | "map" | "MapEntry" | "channel"
                ) =>
            {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{n}` requires type argument(s)."),
                    format!("Write `{n}<T>` — for example, `map<string, int>` or `array<int>`.", n = n),
                    format!("`{n}` is a built-in generic type. It must have the required type arguments.", n = n),
                ));
                Type::Error
            }
            AstType::Named(n, _) if self.generic_shape_table.contains(n) => {
                // Bare generic shape name without type args — invalid in non-generic context.
                Type::Error
            }
            AstType::Named(n, span) => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{n}` is not a known type."),
                    format!("Use a built-in (`int`, `float`, `number`, `boolean`, `string`) or declare `shape {n} {{ ... }}` in this file, or import it from another file."),
                    format!("`{n}` is not declared or imported. If it's defined in another file, add `import {{ {n} }} from \\`path/to/file\\`` at the top."),
                ).with_kind(ynz_diagnostics::DiagnosticKind::NotDefined));
                Type::Error
            }
            AstType::Range { .. } => Type::Error,
            AstType::SelfType { span } => match &self.current_shape {
                Some(name) => Type::Shape { name: name.clone() },
                None => {
                    self.diags.push(Diagnostic::error(
                            span.clone(),
                            "`Self` can only be used inside a function that operates on a shape.",
                            "Use the concrete shape name instead, e.g. `Player`.",
                            "`Self` refers to the type of the enclosing shape — it only makes sense inside functions with a `self` receiver parameter.",
                        ));
                    Type::Error
                }
            },
            AstType::Dynamic { contract, span } => {
                if self.shape_table.contains(contract) {
                    // Record the contract name as referenced so an import used exclusively
                    // in `dynamic Contract` type position is not flagged as unused.
                    self.referenced_names.insert(contract.clone());
                    Type::Dynamic {
                        contract: contract.clone(),
                    }
                } else {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!("`{contract}` is not a known shape — cannot use it as a `dynamic` contract."),
                        format!("Declare `shape {contract} {{ ... }}` with bare method signatures first."),
                        "`dynamic Foo` requires `Foo` to be a contract shape with bare method signature declarations.",
                    ));
                    Type::Error
                }
            }
            AstType::TypeParam { name, .. } => {
                if self.type_param_scope.contains_key(name) {
                    Type::TypeParam { name: name.clone() }
                } else {
                    Type::Error
                }
            }
            AstType::Generic {
                name,
                args,
                name_span,
                span,
            } => {
                // Catch capitalized built-in names (Array, Fixed, Map) — Golden Rule 13:
                // capital letter = type, everything else = lowercase. Built-ins are lowercase.
                let lower = name.to_lowercase();
                if name.as_str() != lower.as_str()
                    && matches!(lower.as_str(), "array" | "fixed" | "map" | "channel")
                {
                    self.diags.push(Diagnostic::error(
                        name_span.clone(),
                        format!("`{name}` is not a type — built-in collection types are lowercase in Yinz."),
                        format!("Use `{lower}` (lowercase): `{lower}<...>`"),
                        "In Yinz, capital letter = user-defined shape, lowercase = built-in. \
                         `Array`, `Fixed`, and `Map` are not valid — use `array`, `fixed`, `map`.",
                    ));
                    return Type::Error;
                }
                let resolved_args: Vec<Type> =
                    args.iter().map(|a| self.ast_type_to_type(a)).collect();
                match name.as_str() {
                    "array" => {
                        let elem = resolved_args.into_iter().next().unwrap_or(Type::Error);
                        Type::BuiltinArray {
                            elem: Box::new(elem),
                        }
                    }
                    "fixed" => {
                        if resolved_args.len() > 1 {
                            self.diags.push(Diagnostic::error(
                                span.clone(),
                                "`fixed<T>` takes one type argument. Yinz doesn't have tuple types.",
                                "Define a shape with named fields instead:\n  shape IntervalConfig { minutes: int, timeframe: Timeframe }\n  const intervals: fixed<IntervalConfig> = [\n      { minutes: 5, timeframe: Timeframe.fiveMinute },\n      ...\n  ]",
                                "Named fields are always self-documenting — `config.minutes` is clearer than a positional index. The shape compiles to the same stack-allocated memory layout a tuple would use, with zero overhead. Yinz also auto-reorders shape fields for optimal memory alignment, so the shape may pack tighter than a manual tuple.",
                            ));
                        }
                        let elem = resolved_args.into_iter().next().unwrap_or(Type::Error);
                        Type::BuiltinFixed {
                            elem: Box::new(elem),
                            size: None,
                        }
                    }
                    "map" => {
                        let mut args = resolved_args.into_iter();
                        let key = args.next().unwrap_or(Type::Error);
                        let val = args.next().unwrap_or(Type::Error);
                        Type::BuiltinMap {
                            key: Box::new(key),
                            val: Box::new(val),
                        }
                    }
                    "MapEntry" => {
                        let mut args = resolved_args.into_iter();
                        let key = args.next().unwrap_or(Type::Error);
                        let val = args.next().unwrap_or(Type::Error);
                        Type::MapEntry {
                            key: Box::new(key),
                            val: Box::new(val),
                        }
                    }
                    // v0.3-M4: `channel<T>` type annotation. One type argument (the element
                    // type). Construction (`channel<T>()` / `channel<T>(N)`) is a separate path
                    // handled in `check_channel_construction` (call position, not type position).
                    "channel" => {
                        if resolved_args.len() != 1 {
                            self.diags.push(Diagnostic::error(
                                span.clone(),
                                "`channel<T>` takes exactly one type argument — the element type.",
                                "Write `channel<int>` — for example, `channel<int>` or `channel<Order>`.",
                                "A `channel<T>` carries values of a single element type `T` between tasks.",
                            ));
                        }
                        let elem = resolved_args.into_iter().next().unwrap_or(Type::Error);
                        Type::BuiltinChannel {
                            elem: Box::new(elem),
                        }
                    }
                    _ => {
                        if self.generic_shape_table.contains(name) {
                            // Record the generic shape name as referenced so an import
                            // used exclusively in `Container<T>` type position is not
                            // flagged as unused.
                            self.referenced_names.insert(name.clone());
                            Type::Generic {
                                name: name.clone(),
                                args: resolved_args,
                            }
                        } else {
                            Type::Error
                        }
                    }
                }
            }
            AstType::Maybe { inner, .. } => {
                let inner_ty = self.ast_type_to_type(inner);
                Type::Maybe {
                    inner: Box::new(inner_ty),
                }
            }
            // M6: Union types — resolve each variant and return Type::Union.
            AstType::Union { variants, .. } => {
                let resolved: Vec<Type> =
                    variants.iter().map(|v| self.ast_type_to_type(v)).collect();
                // `T | none` is rewritten to `maybe<T>` per design/narrowing.md.
                if resolved.len() == 2 {
                    let none_idx = resolved.iter().position(|t| *t == Type::Error); // none resolves oddly
                    let _ = none_idx; // For now, leave `T | none` as Union; P3b note
                }
                // Single-variant union: typeck error (degenerate form).
                if resolved.len() < 2 {
                    Type::Error
                } else {
                    Type::Union { variants: resolved }
                }
            }
            // M7 P3a: `-> T errors` — resolve to ErrorsCapable wrapping the inner type.
            AstType::ErrorCapable { inner, .. } => {
                let inner_ty = self.ast_type_to_type(inner);
                Type::ErrorsCapable {
                    inner: Box::new(inner_ty),
                }
            }
            // M8 P4: `sensitive T` — resolve to Sensitive wrapping the inner type.
            AstType::Sensitive(inner) => {
                let inner_ty = self.ast_type_to_type(inner);
                // Only string is allowed as the inner type in v0.1. Other types
                // will produce type-mismatch errors downstream; no extra diagnostic needed.
                Type::Sensitive {
                    inner: Box::new(inner_ty),
                }
            }
            // AnonShape: hoisted to a synthetic named shape by collect_shapes.
            // Resolve to the same canonical name so the checker sees Type::Shape.
            AstType::AnonShape { fields, .. } => Type::Shape {
                name: crate::shapes::canonical_anon_name(fields),
            },
        }
    }

    /// Walk an `AstType` tree and record every referenced user-defined name
    /// (shape, options, generic shape) in `self.referenced_names`.
    ///
    /// This is the non-diagnostic-emitting companion to `ast_type_to_type`. It
    /// exists so the unused-import pass can see names used only in type-annotation
    /// positions (shape fields, const type annotations, generic type arguments)
    /// without triggering spurious "T is not a known type" diagnostics for bare
    /// type-parameter names inside generic shapes.
    ///
    /// `type_params` is the set of type-parameter names in the enclosing generic
    /// context (e.g. `{"T", "U"}` for `shape Box<T, U>`). Names in this set are
    /// skipped — they are local placeholders, not imported symbols.
    ///
    /// Time: O(n)  Space: O(n)  where n = nodes in the AST type tree (recursion
    /// depth bounds the stack; generic args recurse).
    fn collect_referenced_names_in_ast_type(
        &mut self,
        ast_ty: &AstType,
        type_params: &HashSet<String>,
    ) {
        match ast_ty {
            // Primitives and keywords carry no user-defined symbol references.
            AstType::Nothing
            | AstType::Int
            | AstType::Float
            | AstType::Number { .. }
            | AstType::Bool
            | AstType::Error
            | AstType::Range { .. }
            | AstType::SelfType { .. } => {}

            AstType::Named(n, _) => {
                // Skip bare type-parameter names — they are not imported symbols.
                if type_params.contains(n.as_str()) {
                    return;
                }
                if self.options_table.contains(n)
                    || self.shape_table.contains(n)
                    || self.union_aliases.contains_key(n)
                    || self.generic_shape_table.contains(n)
                {
                    self.referenced_names.insert(n.clone());
                }
            }

            // TypeParam is a resolved placeholder — the name is the param itself,
            // not an imported symbol, so there is nothing to record here.
            AstType::TypeParam { .. } => {}

            AstType::Dynamic { contract, .. } => {
                if self.shape_table.contains(contract) {
                    self.referenced_names.insert(contract.clone());
                }
            }

            AstType::Generic { name, args, .. } => {
                if self.generic_shape_table.contains(name) {
                    self.referenced_names.insert(name.clone());
                }
                for arg in args {
                    self.collect_referenced_names_in_ast_type(arg, type_params);
                }
            }

            AstType::Maybe { inner, .. } => {
                self.collect_referenced_names_in_ast_type(inner, type_params);
            }

            AstType::Union { variants, .. } => {
                for v in variants {
                    self.collect_referenced_names_in_ast_type(v, type_params);
                }
            }

            AstType::ErrorCapable { inner, .. } => {
                self.collect_referenced_names_in_ast_type(inner, type_params);
            }

            AstType::Sensitive(inner) => {
                self.collect_referenced_names_in_ast_type(inner, type_params);
            }

            AstType::AnonShape { fields, .. } => {
                for f in fields {
                    self.collect_referenced_names_in_ast_type(&f.ty, type_params);
                }
            }
        }
    }

    /// Walk an `Expr` tree and record every identifier that names a user-defined
    /// symbol (shape, options type, or any known function/binding) in
    /// `self.referenced_names`.
    ///
    /// This is the non-diagnostic-emitting companion to `infer_expr`. It exists
    /// so the unused-import pass can track names referenced in module-level const
    /// initializers without triggering type-checking diagnostics (which is
    /// `infer_expr`'s job and runs only inside function bodies or generic bodies).
    ///
    /// The walk mirrors `symbol_lookup::collect_use_sites_in_expr` in structure —
    /// full traversal, no leaves skipped — but instead of checking canonical
    /// resolution it simply inserts every `Ident` name that belongs to a known
    /// imported symbol table. For the module-level const case the universe of
    /// "imported names" is the union of shape_table, options_table, sig_table,
    /// generic_shape_table, and generic_fn_table; any ident in any of those is
    /// a candidate referenced name.
    ///
    /// Time: O(n)  Space: O(n)  where n = nodes in the expression tree (recursion
    /// depth bounds the stack).
    fn collect_referenced_names_in_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Ident(n, _) => {
                // An identifier in expression position references an imported
                // name when it is a known shape, options type, or function.
                // sig_table covers user-defined functions (including imported ones).
                if self.shape_table.contains(n)
                    || self.options_table.contains(n)
                    || self.sig_table.fns.contains_key(n)
                    || self.generic_shape_table.contains(n)
                    || self.generic_fn_table.fns.contains_key(n)
                {
                    self.referenced_names.insert(n.clone());
                }
            }
            Expr::Call(call) => {
                self.collect_referenced_names_in_expr(&call.callee);
                for arg in &call.args {
                    self.collect_referenced_names_in_expr(arg);
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.collect_referenced_names_in_expr(receiver);
                for arg in args {
                    self.collect_referenced_names_in_expr(arg);
                }
            }
            Expr::FieldAccess { receiver, .. } => {
                self.collect_referenced_names_in_expr(receiver);
            }
            Expr::BinOp { lhs, rhs, .. } => {
                self.collect_referenced_names_in_expr(lhs);
                self.collect_referenced_names_in_expr(rhs);
            }
            Expr::UnaryOp { operand, .. } => {
                self.collect_referenced_names_in_expr(operand);
            }
            Expr::StructLit { fields, .. } => {
                for f in fields {
                    self.collect_referenced_names_in_expr(&f.value);
                }
            }
            Expr::PostfixOp { receiver, .. } => {
                self.collect_referenced_names_in_expr(receiver);
            }
            Expr::IndexAccess {
                receiver, index, ..
            } => {
                self.collect_referenced_names_in_expr(receiver);
                self.collect_referenced_names_in_expr(index);
            }
            Expr::ArrayLit { elements, .. } => {
                for e in elements {
                    self.collect_referenced_names_in_expr(e);
                }
            }
            Expr::MapLit { entries, .. } => {
                for (k, v) in entries {
                    self.collect_referenced_names_in_expr(k);
                    self.collect_referenced_names_in_expr(v);
                }
            }
            Expr::Is { expr, .. } => {
                self.collect_referenced_names_in_expr(expr);
            }
            Expr::InterpolatedString(parts, _) => {
                for part in parts {
                    if let ynz_ast::nodes::StringPart::Expr(e, _) = part {
                        self.collect_referenced_names_in_expr(e);
                    }
                }
            }
            Expr::Wait(inner, _) | Expr::Background(inner, _) => {
                self.collect_referenced_names_in_expr(inner);
            }
            // Leaves that carry no sub-expressions referencing imported names.
            Expr::StringLit(..)
            | Expr::IntLit(..)
            | Expr::NumberLit(..)
            | Expr::BoolLit(..)
            | Expr::NoneLit { .. }
            | Expr::SelfValue { .. }
            | Expr::Error(..) => {}
        }
    }

    fn emit_binop_mismatch(&mut self, op: &BinOpKind, lhs: &Type, rhs: &Type, span: &SourceSpan) {
        let what = format!(
            "`{}` cannot be used with `{}` and `{}`.",
            binop_display(op),
            type_name(lhs),
            type_name(rhs)
        );
        let what_instead = suggest_conversion(lhs, rhs);
        self.diags.push(Diagnostic::error(
            span.clone(),
            what,
            what_instead,
            "Yinz does not convert between types automatically. Both sides of an expression must have the same type.",
        ));
    }

    #[cfg(test)]
    fn check_test_fn_call(
        &mut self,
        call: &CallExpr,
        name: &str,
        sig: &crate::intrinsics::FreeFnSig,
    ) -> Type {
        if call.args.len() != sig.params.len() {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                format!(
                    "`{name}` takes {} argument(s), but {} were given.",
                    sig.params.len(),
                    call.args.len()
                ),
                format!("Call it with {} argument(s).", sig.params.len()),
                "Every function call must match the number of arguments the function expects.",
            ));
            return Type::Error;
        }
        for (i, (arg, expected)) in call.args.iter().zip(&sig.params).enumerate() {
            let actual = self.infer_expr(arg, None);
            if actual != *expected && actual != Type::Error {
                self.diags.push(Diagnostic::error(
                    arg.span().clone(),
                    format!(
                        "Argument {} to `{name}` should be `{}`, but got `{}`.",
                        i + 1,
                        type_name(expected),
                        type_name(&actual)
                    ),
                    format!("Pass a `{}` here.", type_name(expected)),
                    format!(
                        "`{name}` expects `{}` in this position.",
                        type_name(expected)
                    ),
                ));
            }
        }
        sig.ret.clone()
    }

    // ── M4 P3b: inheritance + follows contract verification ──────────────────

    /// Verify every `shape X follows Y` declaration.
    ///
    /// For each contract sig `method(self, params...) -> Ret` in Y, there must be
    /// a standalone function `method` in `sig_table` whose first param is
    /// `Type::Shape { name: X }` and whose return type matches.
    fn check_follows_contracts(&mut self) {
        // Collect (shape_name, follows_list) to avoid borrow conflicts.
        let follows_list: Vec<(String, Vec<String>, Vec<crate::shapes::ContractSigDef>)> = self
            .shape_table
            .shapes
            .iter()
            .filter(|(_, def)| !def.follows.is_empty())
            .map(|(name, def)| (name.clone(), def.follows.clone(), def.contract_sigs.clone()))
            .collect();

        for (shape_name, contracts, _own_sigs) in &follows_list {
            let shape_ty = Type::Shape {
                name: shape_name.clone(),
            };
            // Record all `follows` contract names as referenced. An import used
            // only as `shape X follows ImportedContract` would otherwise go unseen
            // by the check pass (shapes are pre-resolved in shapes.rs before
            // referenced_names exists). This is the single chokepoint for follows
            // because check_follows_contracts already iterates every shape that
            // has at least one follows contract. The extends case is handled in
            // the ShapeDecl arm of check_module, which sees ALL shape decls.
            for contract_name in contracts.iter() {
                self.referenced_names.insert(contract_name.clone());
            }
            for contract_name in contracts {
                let Some(contract_def) = self.shape_table.get(contract_name) else {
                    continue; // already errored in collect_shapes
                };
                let contract_sigs = contract_def.contract_sigs.clone();
                let shape_def_span = self
                    .shape_table
                    .get(shape_name)
                    .map(|s| s.defined_at.clone())
                    .unwrap_or_else(|| SourceSpan::new("", 0, 0));

                for sig in &contract_sigs {
                    match self.sig_table.fns.get(&sig.name) {
                        None => {
                            self.diags.push(Diagnostic::error(
                                shape_def_span.clone(),
                                format!("`{shape_name}` follows `{contract_name}` but is missing function `{}`.", sig.name),
                                format!("Add `function {}(share self: {shape_name}) -> ...` to this file.", sig.name),
                                format!("`{contract_name}` requires a function named `{}` — define it as a standalone function whose first parameter is `self: {shape_name}`.", sig.name),
                            ));
                        }
                        Some(fn_sig) => {
                            // Check first param matches the implementing shape.
                            match fn_sig.params.first() {
                                Some((_, first_ty)) if *first_ty == shape_ty => {
                                    // Return type must match.
                                    if fn_sig.ret != sig.ret_ty
                                        && fn_sig.ret != Type::Error
                                        && sig.ret_ty != Type::Error
                                    {
                                        self.diags.push(Diagnostic::error(
                                            shape_def_span.clone(),
                                            format!("Function `{}` for `{shape_name}` returns `{}`, but `{contract_name}` requires `{}`.", sig.name, type_name(&fn_sig.ret), type_name(&sig.ret_ty)),
                                            format!("Change the return type to `{}` to satisfy `{contract_name}`.", type_name(&sig.ret_ty)),
                                            "Functions that satisfy a contract must return exactly the type the contract declares.",
                                        ));
                                    }
                                }
                                Some((_, first_ty)) => {
                                    self.diags.push(Diagnostic::error(
                                        shape_def_span.clone(),
                                        format!("Function `{}` cannot satisfy `{contract_name}` for `{shape_name}` — its first parameter is `{}`, not `{shape_name}`.", sig.name, type_name(first_ty)),
                                        format!("Change the first parameter to `share self: {shape_name}`."),
                                        "Contract satisfaction requires the function's first parameter to be the implementing shape.",
                                    ));
                                }
                                None => {
                                    self.diags.push(Diagnostic::error(
                                        shape_def_span.clone(),
                                        format!("Function `{}` has no parameters but `{contract_name}` requires a `self: {shape_name}` receiver.", sig.name),
                                        format!("Add `share self: {shape_name}` as the first parameter."),
                                        "Contract functions must have the implementing shape as their first parameter.",
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ── M4 P3a: shape type-checking ──────────────────────────────────────────

    /// Infer the type of a field access `receiver.field`.
    fn infer_field_access(
        &mut self,
        receiver: &Expr,
        field: &str,
        field_span: &SourceSpan,
    ) -> Type {
        let receiver_ty = self.infer_expr(receiver, None);

        // M7 P3a: errors-capable value property access (message, suggestions, trace, source).
        // These are dot-property accesses (no parens) per the Yinz dot-postfix rule.
        if let Type::ErrorsCapable { inner } = &receiver_ty {
            let inner = inner.as_ref().clone();
            return match field {
                "message" => Type::String,
                "suggestions" => Type::BuiltinArray {
                    elem: Box::new(Type::String),
                },
                "trace" => Type::BuiltinArray {
                    elem: Box::new(Type::Shape {
                        name: "Frame".into(),
                    }),
                },
                "source" => Type::Shape {
                    name: "SourceLoc".into(),
                },
                other => {
                    // Not an error-property — check if it's a field on the inner type.
                    // Recurse by pretending the receiver has the inner type.
                    let _ = inner;
                    self.diags.push(Diagnostic::error(
                        field_span.clone(),
                        format!("An `errors`-capable value does not have a field called `{other}`."),
                        "Available properties: message, suggestions, trace, source. Call `.failed()` first to check, or `.or(default)` for a fallback.",
                        "An `errors`-capable value is the output of a call that can fail. Check `.failed()` first, then access the success value directly.",
                    ));
                    Type::Error
                }
            };
        }

        // M5 P3c: `MapEntry<K,V>.key` / `.value` field access.
        if let Type::MapEntry { key, val } = &receiver_ty {
            return match field {
                "key" => key.as_ref().clone(),
                "value" => val.as_ref().clone(),
                other => {
                    self.diags.push(Diagnostic::error(
                        field_span.clone(),
                        format!("`MapEntry` does not have a field called `{other}`."),
                        "Use `.key` to get the key and `.value` to get the value.",
                        "`MapEntry<K, V>` has exactly two fields: `key: K` and `value: V`.",
                    ));
                    Type::Error
                }
            };
        }

        // M5 P3c: dot access on map — emit "use bracket syntax" error.
        if let Type::BuiltinMap { key, val } = &receiver_ty {
            self.diags.push(Diagnostic::error(
                field_span.clone(),
                format!("Cannot use `.{field}` to look up a map key."),
                format!("Map keys are runtime values — use `m[\"{field}\"]` to look up a key. For checking existence, use `m.has(\"{field}\")`.",),
                "Dot access is for shape fields with compile-time-known names. Map keys are dynamic — use bracket syntax.",
            ));
            let _ = (key, val);
            return Type::Error;
        }

        // M5 P3b: `maybe<T>.value` — flow-sensitive field access.
        if let Type::Maybe { inner } = &receiver_ty {
            if field == "value" {
                let inner = inner.as_ref().clone();
                // Check if the binding is known-non-none from a prior .exists() check.
                let is_safe = if let Expr::Ident(name, _) = receiver {
                    self.maybe_non_none.contains(name.as_str())
                } else {
                    false
                };
                if !is_safe {
                    self.diags.push(Diagnostic::error(
                        field_span.clone(),
                        "`maybe.value` requires you to first check `m.exists()`.",
                        "Add a check: `if (m.exists()) { print(m.value) }`. Or use a default: `m.or(0)`.",
                        "The compiler cannot prove this `maybe` has a value here. `.value` without a prior `.exists()` check is a compile error.",
                    ));
                    return Type::Error;
                }
                return inner;
            } else {
                self.diags.push(Diagnostic::error(
                    field_span.clone(),
                    format!("`maybe<{}>` does not have a field called `{field}`.", type_name(inner.as_ref())),
                    "Use `.value` to get the value (after `.exists()` check), `.exists()` to check, or `.or(default)` for a safe fallback.",
                    "`maybe<T>` only has the virtual field `.value` (requires prior `.exists()` guard) and the methods `.exists()` and `.or()`.",
                ));
                return Type::Error;
            }
        }

        // Generic shape field access: `p.first` where `p: Pair<int, string>`.
        if let Type::Generic { name, args } = &receiver_ty {
            let name = name.clone();
            let args = args.clone();
            if let Some(generic_def) = self.generic_shape_table.get(&name) {
                let subst = generic_def.make_substitution(&args);
                return match generic_def.field_type(field, &subst) {
                    Some(ty) => ty,
                    None => {
                        let available = generic_def.field_names();
                        self.emit_unknown_field_error(
                            &name,
                            field,
                            &available,
                            field_span.clone(),
                            false,
                        );
                        Type::Error
                    }
                };
            }
        }

        // Dynamic dispatch: treat the contract shape as the lookup target.
        let shape_name = match &receiver_ty {
            Type::Shape { name } => name.clone(),
            Type::Dynamic { contract } => contract.clone(),
            Type::Error => return Type::Error,
            other => {
                self.diags.push(Diagnostic::error(
                    field_span.clone(),
                    format!("`{}` values do not have fields.", type_name(other)),
                    "Field access is only available on shape values.",
                    "Shapes are the only Yinz types with named fields. Primitive types like `int` and `string` use methods instead.",
                ));
                return Type::Error;
            }
        };
        let shape_name = shape_name.clone();

        // M7 P3c: built-in compiler-synthesized shapes — Frame and SourceLoc.
        // These are never user-declared in source; their fields are hardcoded here.
        if shape_name == "Frame" {
            return match field {
                "file" => Type::String,
                "line" => Type::Maybe {
                    inner: Box::new(Type::Int),
                },
                "function" => Type::String,
                other => {
                    self.diags.push(Diagnostic::error(
                        field_span.clone(),
                        format!("`Frame` does not have a field called `{other}`."),
                        "Frame has three fields: `file: string`, `line: maybe<int>`, `function: string`.",
                        "`Frame` is a compiler-synthesized shape that represents one stack frame in an error trace.",
                    ));
                    Type::Error
                }
            };
        }
        if shape_name == "SourceLoc" {
            return match field {
                "file" => Type::String,
                "line" => Type::Maybe {
                    inner: Box::new(Type::Int),
                },
                other => {
                    self.diags.push(Diagnostic::error(
                        field_span.clone(),
                        format!("`SourceLoc` does not have a field called `{other}`."),
                        "SourceLoc has two fields: `file: string`, `line: maybe<int>`.",
                        "`SourceLoc` is a compiler-synthesized shape that records the source position of an error.",
                    ));
                    Type::Error
                }
            };
        }

        let Some(shape_def) = self.shape_table.get(&shape_name) else {
            return Type::Error; // shape not in table — already errored at pre-pass
        };
        let Some(field_def) = shape_def.field(field) else {
            let available: Vec<&str> = shape_def.fields.iter().map(|f| f.name.as_str()).collect();
            self.emit_unknown_field_error(
                &shape_name,
                field,
                &available,
                field_span.clone(),
                false,
            );
            return Type::Error;
        };
        // Hidden field visibility: only accessible inside the declaring shape's functions.
        if field_def.is_hidden {
            let inside_shape = self.current_shape.as_deref() == Some(&shape_name);
            if !inside_shape {
                self.diags.push(Diagnostic::error(
                    field_span.clone(),
                    format!("`{field}` is a hidden field of `{shape_name}` and cannot be read here."),
                    format!("Move this access inside a function whose first parameter is `self: {shape_name}`."),
                    "Hidden fields are only accessible to functions that explicitly operate on that shape — they cannot be read by outside code.",
                ));
                return Type::Error;
            }
        }
        field_def.ty.clone()
    }

    /// Emit a "shape X does not have a field called Y" diagnostic.
    ///
    /// Shared by field access on concrete/generic shapes and struct-literal unknown-field
    /// checks.  The `why` wording differs slightly between access (reading a field that
    /// doesn't exist) and literal construction (writing a field that doesn't exist).
    fn emit_unknown_field_error(
        &mut self,
        shape_name: &str,
        field: &str,
        available: &[&str],
        span: SourceSpan,
        is_struct_literal: bool,
    ) {
        let display_name = if shape_name.starts_with("__anon__") {
            type_name(&Type::Shape {
                name: shape_name.to_string(),
            })
        } else {
            shape_name.to_string()
        };
        let suggestion = find_closest_name(field, available);
        let what_instead = match suggestion {
            Some(close) => format!("Did you mean `{close}`?"),
            None => format!(
                "`{display_name}` has these fields: {}",
                available.join(", ")
            ),
        };
        let why = if is_struct_literal {
            "Shape values can only set fields declared on the shape."
        } else {
            "Field names must match exactly what was declared in the `shape` body."
        };
        self.diags.push(Diagnostic::error(
            span,
            format!("`{display_name}` does not have a field called `{field}`."),
            what_instead,
            why,
        ));
    }

    /// Type-check a struct literal `{ name: "x", health: 100 }` against the hint type.
    fn check_struct_lit(
        &mut self,
        fields: &[StructLitField],
        hint: Option<&Type>,
        span: &SourceSpan,
    ) -> Type {
        // When the hint is a map (or a union containing a map), accept identifier-key syntax.
        // `{ name: value }` works the same as `{ "name": value }` when the type context says map.
        // Quoted keys remain valid too; this just removes the friction of requiring them.
        let map_hint = match hint {
            Some(Type::BuiltinMap { key, val }) => Some((key, val)),
            Some(Type::Union { variants }) => variants.iter().find_map(|v| {
                if let Type::BuiltinMap { key, val } = v {
                    Some((key, val))
                } else {
                    None
                }
            }),
            _ => None,
        };
        if let Some((key, val)) = map_hint {
            let val = val.as_ref().clone();
            for f in fields {
                let actual = self.infer_expr(&f.value, Some(&val));
                if actual != Type::Error && val != Type::Error && actual != val {
                    self.diags.push(Diagnostic::error(
                        f.value.span().clone(),
                        format!(
                            "Map value for key `{}` is `{}`, but this map holds `{}`.",
                            f.name,
                            type_name(&actual),
                            type_name(&val)
                        ),
                        format!("Pass a `{}` value.", type_name(&val)),
                        "All values in a map must be the same type.",
                    ));
                }
            }
            return Type::BuiltinMap {
                key: key.clone(),
                val: Box::new(val),
            };
        }

        let shape_name = match hint {
            Some(Type::Shape { name }) => name.clone(),
            // `let x: array<Symbol> = { ... }` — they wrote a shape value where an array goes.
            // Specific suggestion: wrap in brackets.
            Some(Type::BuiltinArray { elem }) => {
                let elem_name = type_name(elem);
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{{ ... }}` creates a single `{elem_name}` value, not an `array<{elem_name}>`."),
                    "Put it inside `[...]` to make an array: `[{ ... }]`",
                    format!("`{{ ... }}` creates one value. `[...]` creates a collection. \
                             Use `array<{elem_name}>` when you need multiple values."),
                ));
                for f in fields {
                    self.infer_expr(&f.value, None);
                }
                return Type::Error;
            }
            Some(Type::BuiltinFixed { elem, .. }) => {
                let elem_name = type_name(elem);
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{{ ... }}` creates a single `{elem_name}` value, not a `fixed<{elem_name}>`."),
                    "Put it inside `[...]` to make a fixed array: `[{ ... }]`",
                    "`{ ... }` creates one value. `[...]` creates a collection.",
                ));
                for f in fields {
                    self.infer_expr(&f.value, None);
                }
                return Type::Error;
            }
            Some(other) if *other != Type::Error => {
                let what_instead = match other {
                    Type::BuiltinMap { .. } =>
                        "For a map literal, use quoted string keys: `{ \"key\": value, \"key2\": value2 }`".to_string(),
                    Type::Union { .. } =>
                        "Check the union type — if one variant is a `shape`, annotate with its name; if it's a `map`, use quoted string keys.".to_string(),
                    _ =>
                        "Annotate the binding with a `shape` name: `let p: Player = { ... }`".to_string(),
                };
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!(
                        "A shape value `{{ ... }}` cannot produce a `{}` value.",
                        type_name(other)
                    ),
                    what_instead,
                    "Shape values use identifier field names (`name: value`). Map literals use quoted string keys (`\"name\": value`).",
                ));
                for f in fields {
                    self.infer_expr(&f.value, None);
                }
                return Type::Error;
            }
            Some(Type::Error) => {
                // Hint is Type::Error — an upstream diagnostic already explained the problem.
                // Don't cascade with a confusing "needs a type annotation" message.
                for f in fields {
                    self.infer_expr(&f.value, None);
                }
                return Type::Error;
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    "This shape value needs a type annotation — the compiler needs to know which `shape` to create.",
                    "Add a type annotation: `let p: Player = { ... }`",
                    "Shape values are anonymous — the `shape` type comes from the annotation on the left. Without it, the compiler cannot check field names or types.",
                ));
                for f in fields {
                    self.infer_expr(&f.value, None);
                }
                return Type::Error;
            }
        };

        let Some(shape_def) = self.shape_table.get(&shape_name) else {
            for f in fields {
                self.infer_expr(&f.value, None);
            }
            return Type::Error;
        };

        let display_name = if shape_name.starts_with("__anon__") {
            type_name(&Type::Shape {
                name: shape_name.clone(),
            })
        } else {
            shape_name.clone()
        };

        // base shapes cannot be instantiated
        if shape_def.is_base {
            self.diags.push(Diagnostic::error(
                span.clone(),
                format!("`{display_name}` is a `base shape` and cannot be constructed directly."),
                format!("Create a shape that extends `{display_name}`, then construct that instead."),
                "`base shape` declarations are meant to be extended — they provide shared fields for child shapes but cannot be instantiated on their own.",
            ));
            for f in fields {
                self.infer_expr(&f.value, None);
            }
            return Type::Error;
        }

        // Collect all missing required fields, then emit one consolidated diagnostic.
        let missing: Vec<&str> = shape_def
            .fields
            .iter()
            .filter(|sf| !sf.is_hidden && !fields.iter().any(|f| f.name == sf.name))
            .map(|sf| sf.name.as_str())
            .collect();
        match missing.len() {
            0 => {}
            1 => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("Missing field `{}` in `{display_name}` construction.", missing[0]),
                    format!("Add `{}: value` to the shape value.", missing[0]),
                    "Every visible field of a shape must be provided when constructing a value — the compiler cannot fill them in for you.",
                ));
            }
            n => {
                let list = missing
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let add = missing
                    .iter()
                    .map(|name| format!("`{name}: value`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("{n} fields are missing from this `{display_name}` value: {list}."),
                    format!("Add the missing fields: {add}."),
                    "Every visible field of a shape must be provided when constructing a value — the compiler cannot fill them in for you.",
                ));
            }
        }

        // Check each provided field: name must exist and value type must match.
        for lit_field in fields {
            let found_field = shape_def.fields.iter().find(|f| f.name == lit_field.name);
            match found_field {
                None => {
                    let available: Vec<&str> = shape_def
                        .fields
                        .iter()
                        .filter(|f| !f.is_hidden)
                        .map(|f| f.name.as_str())
                        .collect();
                    self.emit_unknown_field_error(
                        &shape_name,
                        &lit_field.name,
                        &available,
                        lit_field.name_span.clone(),
                        true,
                    );
                    self.infer_expr(&lit_field.value, None);
                }
                Some(field_def)
                    if field_def.is_hidden
                        && lit_field.name_span.file != shape_def.defined_at.file =>
                {
                    // External file is trying to set a hidden field at construction time.
                    let declaring_file = std::path::Path::new(&shape_def.defined_at.file)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&shape_def.defined_at.file);
                    let setter_name = {
                        let mut chars = lit_field.name.chars();
                        match chars.next() {
                            Some(first) if first.is_alphabetic() => {
                                format!("with{}{}", first.to_uppercase(), chars.as_str())
                            }
                            _ => format!("with_{}", lit_field.name),
                        }
                    };
                    let field_ty_name = type_name(&field_def.ty);
                    self.diags.push(Diagnostic::error(
                        lit_field.name_span.clone(),
                        format!(
                            "`{}` is hidden — code in this file cannot set it at construction.",
                            lit_field.name
                        ),
                        format!(
                            "Remove `{}: <value>` from the shape value. \
                             The default from `{declaring_file}` will be used. \
                             To customize the value, expose a setter from the declaring file: \
                             `function {setter_name}(lend self: {shape_name}, v: {field_ty_name}) -> nothing {{ self.{} = v }}`\
                             — then call that function instead.",
                            lit_field.name, lit_field.name
                        ),
                        "Hidden fields are file-private (the spec calls this `visibility, not mutability`). \
                         Allowing external construction to set them would bypass whatever invariants \
                         the declaring file's functions maintain.",
                    ));
                    self.infer_expr(&lit_field.value, None);
                }
                Some(field_def) => {
                    let expected = field_def.ty.clone();
                    let actual = self.infer_expr(&lit_field.value, Some(&expected));
                    if actual != Type::Error
                        && expected != Type::Error
                        && !types_compatible(&actual, &expected)
                    {
                        self.diags.push(Diagnostic::error(
                            lit_field.name_span.clone(),
                            format!(
                                "Field `{}` expects `{}`, but got `{}`.",
                                lit_field.name,
                                type_name(&expected),
                                type_name(&actual)
                            ),
                            format!(
                                "Pass a `{}` value for `{}`.",
                                type_name(&expected),
                                lit_field.name
                            ),
                            format!(
                                "`{shape_name}.{}` was declared as `{}`.",
                                lit_field.name,
                                type_name(&expected)
                            ),
                        ));
                    }
                }
            }
        }

        Type::Shape { name: shape_name }
    }

    /// Type-check a field assignment `target.field = value`.
    /// Reject a write through a `share`-declared parameter.
    ///
    /// `design/ownership.md` (line 41) requires the compiler to verify that an explicit
    /// ownership modifier matches the body's use of the parameter. A `share` parameter is a
    /// read-only borrow — the caller keeps ownership and trusts the value is unchanged after
    /// the call (`design/concurrency.md` line 651 makes this the auto-parallel soundness
    /// premise). Mutating a field or element of a `share` parameter contradicts that promise.
    ///
    /// A *bare* parameter (no explicit modifier) that the body mutates is LEGAL — the compiler
    /// figures out the effective modifier (`lend`) from the body. Only an explicitly-declared
    /// `share` parameter (including `share self`) is the contradiction.
    ///
    /// Returns `true` when a diagnostic was emitted (the caller skips downstream type-checking
    /// of the assignment, mirroring the `const` reject); `false` otherwise.
    ///
    /// `kind` is the word for what is being changed (`"fields"` for field writes, `"elements"`
    /// for index/element writes) so the diagnostic reads accurately at each call site.
    fn reject_share_param_mutation(
        &mut self,
        root_name: &str,
        kind: &str,
        span: &SourceSpan,
    ) -> bool {
        let Some(entry) = self.scope.lookup(root_name) else {
            return false;
        };
        if entry.param_ownership != Some(ynz_ast::nodes::OwnershipModifier::Share) {
            return false;
        }
        let value_ty = type_name(&entry.ty);
        self.diags.push(Diagnostic::error(
            span.clone(),
            format!("`{root_name}` is declared `share` — read-only — so its {kind} cannot be changed."),
            format!("Change the parameter to `lend {root_name}: {value_ty}` if this function needs to modify it."),
            "A `share` parameter is a read-only borrow: the caller keeps ownership and trusts the value is unchanged after the call. When a function modifies a value, declare `lend` so the change is visible at every call site.",
        ));
        true
    }

    /// Reject an in-place collection mutator (`array.add`/`map.set`/`fixed.set`/…) called on a
    /// receiver whose binding cannot be mutated — a `const` binding or a `share`-declared
    /// parameter (including `share self`).
    ///
    /// A mutating method writes its receiver in place, exactly like a field or element assign.
    /// The pure-named-method contract (`stdlib-design.md` Rule 1) guarantees that only the
    /// methods named in `*_method_is_mutating` change the receiver; read methods (`.get`,
    /// `.count`, `.contains`) leave it unchanged and are never rejected here. `kind` is the word
    /// for what is being changed (`"elements"` for arrays/fixed, `"entries"` for maps) so the
    /// reused `share` diagnostic reads accurately.
    ///
    /// Returns `true` when a diagnostic was emitted (the caller returns `Type::Nothing` without
    /// running the rest of the method dispatch); `false` otherwise.
    fn reject_mutating_collection_method(
        &mut self,
        receiver_expr: Option<&Expr>,
        method: &str,
        kind: &str,
        is_mutating: bool,
        span: &SourceSpan,
    ) -> bool {
        if !is_mutating {
            return false;
        }
        let Some(recv) = receiver_expr else {
            return false;
        };
        let Some(root_name) = root_binding_name(recv) else {
            return false;
        };
        let Some(entry) = self.scope.lookup(root_name) else {
            return false;
        };
        if entry.is_const {
            self.diags.push(Diagnostic::error(
                span.clone(),
                format!("`{root_name}` is `const` so `.{method}()` cannot change its {kind}."),
                format!("Declare it with `let` instead: `let {root_name} = ...`"),
                "`const` bindings are fully read-only — no reassignment, no field changes, no element writes. A method that adds, removes, or replaces items writes the value in place, which `const` does not permit. Use `let` for collections that need to change.",
            ));
            return true;
        }
        self.reject_share_param_mutation(root_name, kind, span)
    }

    fn check_field_assign(&mut self, target: &Expr, value: &Expr, span: &SourceSpan) {
        let Expr::FieldAccess {
            receiver,
            field,
            field_span,
            ..
        } = target
        else {
            // Parser only produces FieldAssign when target is a FieldAccess, but be defensive.
            self.infer_expr(target, None);
            self.infer_expr(value, None);
            return;
        };

        // The receiver must be a mutable (let-bound, non-const) shape value.
        // Walk the receiver chain to find the root binding and check it.
        if let Some(root_name) = root_binding_name(receiver) {
            let is_const = self
                .scope
                .lookup(root_name)
                .is_some_and(|entry| entry.is_const);
            if is_const {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{root_name}` is `const` and its fields cannot be changed."),
                    format!("Declare it with `let` instead: `let {root_name}: ShapeType = {{ ... }}`"),
                    "`const` bindings are fully read-only — no reassignment, no field mutation. Use `let` for values that need to change.",
                ));
                self.infer_expr(value, None);
                return;
            }
            // A `share` parameter is a read-only borrow — mutating a field contradicts the
            // ownership promise the caller relies on (`design/ownership.md` line 41).
            if self.reject_share_param_mutation(root_name, "fields", span) {
                self.infer_expr(value, None);
                return;
            }
        }

        // Resolve the field and check the value type.
        let field_ty = self.infer_field_access(receiver, field, field_span);
        let value_ty = self.infer_expr(value, Some(&field_ty));

        if field_ty != Type::Error && value_ty != Type::Error && field_ty != value_ty {
            self.diags.push(Diagnostic::error(
                span.clone(),
                format!(
                    "Cannot assign `{}` to field `{field}` which has type `{}`.",
                    type_name(&value_ty),
                    type_name(&field_ty)
                ),
                format!("Pass a `{}` value.", type_name(&field_ty)),
                format!(
                    "The field `{field}` was declared as `{}`.",
                    type_name(&field_ty)
                ),
            ));
        }
    }

    /// Type-check a dot-postfix body operation (`.copy()` or `.freeze()`).
    fn check_postfix_op(&mut self, receiver: &Expr, op: &PostfixOpKind, span: &SourceSpan) -> Type {
        let receiver_ty = self.infer_expr(receiver, None);
        match op {
            PostfixOpKind::Copy => {
                // P3c will enforce trivially-copyable requirement.
                // P3a: just return the receiver type.
                if receiver_ty == Type::Error {
                    return Type::Error;
                }
                receiver_ty
            }
            PostfixOpKind::Freeze => {
                // P3c will flip the binding's mutability.
                // P3a: no-op semantically, returns nothing.
                let _ = span;
                Type::Nothing
            }
        }
    }

    /// Type-check an array or fixed literal `[e1, e2, ...]`.
    fn check_array_lit(
        &mut self,
        elements: &[Expr],
        hint: Option<&Type>,
        span: &SourceSpan,
    ) -> Type {
        // Determine element type and whether this is a fixed literal from the hint.
        let (hint_elem, is_fixed) = match hint {
            Some(Type::BuiltinArray { elem }) => (Some(elem.as_ref().clone()), false),
            Some(Type::BuiltinFixed { elem, .. }) => (Some(elem.as_ref().clone()), true),
            Some(Type::Maybe { inner }) => {
                // maybe<array<T>> — the inner array has an element type.
                if let Type::BuiltinArray { elem } = inner.as_ref() {
                    (Some(elem.as_ref().clone()), false)
                } else {
                    (None, false)
                }
            }
            Some(Type::Shape { name }) => {
                // `let x: SomeShape = [...]` — shape is a single value, not a collection.
                // Emit one targeted error and return Type::Error to suppress the downstream
                // let-binding type mismatch, which would just be noise.
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`[]` is an array literal, but `{name}` is a shape — a single value, not a collection."),
                    format!("Use `array<{name}>` if you want a list: `let ... : array<{name}> = []`"),
                    format!("`{name}` holds one value. To store multiple `{name}` values, use `array<{name}>`."),
                ));
                return Type::Error;
            }
            _ => (None, false),
        };

        let mut elem_ty = hint_elem.clone().unwrap_or(Type::Error);
        for (i, elem) in elements.iter().enumerate() {
            let ty = self.infer_expr(elem, hint_elem.as_ref());
            if i == 0 && elem_ty == Type::Error {
                elem_ty = ty.clone();
            } else if ty != Type::Error && elem_ty != Type::Error && ty != elem_ty {
                self.diags.push(Diagnostic::error(
                    elem.span().clone(),
                    format!(
                        "Element {} has type `{}`, but the array expects `{}`.",
                        i + 1,
                        type_name(&ty),
                        type_name(&elem_ty)
                    ),
                    format!(
                        "Use a `{}` value here, or change the annotation.",
                        type_name(&elem_ty)
                    ),
                    "All elements of an array or fixed literal must have the same type.",
                ));
            }
        }

        let hint_is_error = matches!(hint, Some(Type::Error));
        if elem_ty == Type::Error && elements.is_empty() && !hint_is_error {
            // Only emit the "cannot work out element type" diagnostic when there's no
            // hint at all (bare `let arr = []`). When hint is Type::Error, an upstream
            // diagnostic already captured the annotation problem — don't cascade.
            self.diags.push(Diagnostic::error(
                span.clone(),
                "Cannot work out the element type of this empty array literal.",
                "Add a type annotation: `let arr: array<int> = []`.",
                "Without an annotation, the compiler cannot determine what type of elements this array holds.",
            ));
        }

        let size = elements.len();
        if is_fixed {
            Type::BuiltinFixed {
                elem: Box::new(elem_ty),
                size: Some(size),
            }
        } else {
            Type::BuiltinArray {
                elem: Box::new(elem_ty),
            }
        }
    }

    /// Type-check an index assignment `receiver[index] = value`.
    fn check_index_assign(
        &mut self,
        receiver: &Expr,
        index: &Expr,
        value: &Expr,
        span: &SourceSpan,
    ) {
        // const-deep-immutability: reject element writes on const-bound collections,
        // mirroring the same guard in check_field_assign.
        if let Some(root_name) = root_binding_name(receiver) {
            let is_const = self
                .scope
                .lookup(root_name)
                .is_some_and(|entry| entry.is_const);
            if is_const {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{root_name}` is `const` and its elements cannot be changed."),
                    format!("Declare it with `let` instead: `let {root_name}: array<...> = [...]`"),
                    "`const` bindings are fully read-only — no reassignment, no field changes, no element writes. Use `let` for values that need to change.",
                ));
                self.infer_expr(value, None);
                return;
            }
            // A `share` parameter is a read-only borrow — writing an element contradicts the
            // ownership promise the caller relies on (`design/ownership.md` line 41).
            if self.reject_share_param_mutation(root_name, "elements", span) {
                self.infer_expr(value, None);
                return;
            }
        }

        let recv_ty = self.infer_expr(receiver, None);
        let _idx_ty = self.infer_expr(index, Some(&Type::Int));
        match &recv_ty {
            Type::BuiltinArray { elem } => {
                let expected = elem.as_ref().clone();
                let val_ty = self.infer_expr(value, Some(&expected));
                if val_ty != Type::Error && expected != Type::Error && val_ty != expected {
                    self.diags.push(Diagnostic::error(
                        value.span().clone(),
                        format!(
                            "This value is `{}`, but the array holds `{}`.",
                            type_name(&val_ty),
                            type_name(&expected)
                        ),
                        format!("Assign a `{}` value.", type_name(&expected)),
                        "Index assignment must match the array's element type.",
                    ));
                }
            }
            Type::BuiltinFixed { elem, .. } => {
                let expected = elem.as_ref().clone();
                let val_ty = self.infer_expr(value, Some(&expected));
                if val_ty != Type::Error && expected != Type::Error && val_ty != expected {
                    self.diags.push(Diagnostic::error(
                        value.span().clone(),
                        format!(
                            "This value is `{}`, but the fixed array holds `{}`.",
                            type_name(&val_ty),
                            type_name(&expected)
                        ),
                        format!("Assign a `{}` value.", type_name(&expected)),
                        "Index assignment must match the fixed array's element type.",
                    ));
                }
            }
            Type::BuiltinMap { val: map_val, .. } => {
                let expected = map_val.as_ref().clone();
                let val_ty = self.infer_expr(value, Some(&expected));
                if val_ty != Type::Error && expected != Type::Error && val_ty != expected {
                    self.diags.push(Diagnostic::error(
                        value.span().clone(),
                        format!(
                            "This value has type `{}`, but the map holds `{}` values.",
                            type_name(&val_ty),
                            type_name(&expected)
                        ),
                        format!("Assign a `{}` value.", type_name(&expected)),
                        "Map value assignment must match the map's value type.",
                    ));
                }
            }
            Type::Error => {
                self.infer_expr(value, None);
            }
            other => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{}` does not support index assignment.", type_name(other)),
                    "Index assignment works on `array<T>`, `fixed<T>`, and `map<K, V>`.",
                    "Only built-in collection types support `collection[index] = value` syntax.",
                ));
                self.infer_expr(value, None);
            }
        }
    }

    /// Type-check a map literal `{ "alice": 90, "bob": 85 }`.
    fn check_map_lit(
        &mut self,
        entries: &[(Expr, Expr)],
        hint: Option<&Type>,
        span: &SourceSpan,
    ) -> Type {
        let (hint_key, hint_val) = match hint {
            Some(Type::BuiltinMap { key, val }) => {
                (Some(key.as_ref().clone()), Some(val.as_ref().clone()))
            }
            _ => (None, None),
        };

        let mut key_ty = hint_key.clone().unwrap_or(Type::Error);
        let mut val_ty = hint_val.clone().unwrap_or(Type::Error);
        let mut seen_keys: std::collections::HashMap<String, ynz_diagnostics::SourceSpan> =
            std::collections::HashMap::new();

        for (key_expr, val_expr) in entries {
            let k = self.infer_expr(key_expr, hint_key.as_ref());
            let v = self.infer_expr(val_expr, hint_val.as_ref());

            if key_ty == Type::Error {
                key_ty = k.clone();
            }
            if val_ty == Type::Error {
                val_ty = v.clone();
            }

            if k != Type::Error && key_ty != Type::Error && k != key_ty {
                self.diags.push(Diagnostic::error(
                    key_expr.span().clone(),
                    format!(
                        "This key has type `{}`, but the map uses `{}` keys.",
                        type_name(&k),
                        type_name(&key_ty)
                    ),
                    format!(
                        "Use a `{}` key, or change the map annotation.",
                        type_name(&key_ty)
                    ),
                    "All keys in a map literal must have the same type.",
                ));
            }
            if v != Type::Error && val_ty != Type::Error && v != val_ty {
                self.diags.push(Diagnostic::error(
                    val_expr.span().clone(),
                    format!(
                        "This value has type `{}`, but the map holds `{}` values.",
                        type_name(&v),
                        type_name(&val_ty)
                    ),
                    format!(
                        "Use a `{}` value, or change the map annotation.",
                        type_name(&val_ty)
                    ),
                    "All values in a map literal must have the same type.",
                ));
            }

            // Duplicate-key detection for literal string/int keys.
            let key_repr = match key_expr {
                Expr::StringLit(bytes, _) => Some(String::from_utf8_lossy(bytes).to_string()),
                Expr::IntLit(n, _) => Some(n.to_string()),
                // M7: backtick strings with no interpolation are pure literals — check for duplicates.
                Expr::InterpolatedString(parts, _) => {
                    if parts.len() == 1 {
                        if let ynz_ast::nodes::StringPart::Lit(bytes, _) = &parts[0] {
                            Some(String::from_utf8_lossy(bytes).to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(key_str) = key_repr {
                if let Some(first_span) = seen_keys.get(&key_str) {
                    self.diags.push(
                        Diagnostic::error(
                            key_expr.span().clone(),
                            format!("Duplicate key `\"{key_str}\"` in this map literal — the key is listed twice."),
                            "Remove or rename one of the two entries so each key is unique.",
                            "The compiler refuses to silently pick one — duplicate keys in a map literal are always a mistake.",
                        ).with_related(first_span.clone(), "first occurrence here"),
                    );
                } else {
                    seen_keys.insert(key_str, key_expr.span().clone());
                }
            }
        }

        if (key_ty == Type::Error || val_ty == Type::Error) && entries.is_empty() {
            self.diags.push(Diagnostic::error(
                span.clone(),
                "Cannot work out the key and value types of this empty map literal.",
                "Add a type annotation: `let m: map<string, int> = {}`.",
                "Without an annotation, the compiler cannot determine what type of keys and values this map holds.",
            ));
        }

        Type::BuiltinMap {
            key: Box::new(key_ty),
            val: Box::new(val_ty),
        }
    }

    // ── M6: union + narrowing typeck ──────────────────────────────────────────

    /// Validate an `Is(TypePath)` arm pattern in a multi-case block.
    /// Emits a diagnostic if the named type is not a variant of the scrutinee's union.
    fn check_is_arm_pattern(
        &mut self,
        scrutinee_ty: &Type,
        type_path: &ynz_ast::nodes::TypePath,
        span: &SourceSpan,
    ) {
        match scrutinee_ty {
            Type::Union { variants } => {
                let valid: Vec<String> = variants
                    .iter()
                    .filter_map(|v| {
                        if let Type::Shape { name } = v {
                            Some(name.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                // Record the variant name as referenced so an import used exclusively
                // via `is TypeName` arm pattern is not flagged as unused.
                if !type_path.name.is_empty() {
                    self.referenced_names.insert(type_path.name.clone());
                }
                if !type_path.name.is_empty() && !valid.contains(&type_path.name) {
                    self.diags.push(Diagnostic::error(
                        type_path.span.clone(),
                        format!("`{}` is not a variant of this union.", type_path.name),
                        format!("Valid variants are: {}", valid.join(", ")),
                        "The `is TypeName` arm must name one of the union's declared variants.",
                    ));
                }
            }
            Type::Error => {} // suppress cascades
            other => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`is {}` used on `{}` which is not a union type.", type_path.name, type_name(other)),
                    "The `is TypeName =>` arm form is for union types: `shape S = A | B`.",
                    "Union types have multiple variants — `is` checks which variant a value is at runtime. \
                     Use `variantName =>` for options types.",
                ));
            }
        }
    }

    // ── M6: options typeck helpers ────────────────────────────────────────────

    /// Typecheck `x is Foo` type-narrowing predicate expression.
    ///
    /// Returns `Type::Bool`. Validates that the scrutinee is a union type AND
    /// that the type name is a declared variant of that union.
    ///
    /// For the condition-form narrowing (`if (x is Foo) { ... }`), the
    /// actual narrowing fact is applied in `check_stmt_if` when it detects
    /// an `Expr::Is` condition. This method just produces the bool type.
    fn check_is_expr(
        &mut self,
        inner: &Expr,
        type_path: &ynz_ast::nodes::TypePath,
        _span: &SourceSpan,
    ) -> Type {
        let scrutinee_ty = self.infer_expr(inner, None);
        if type_path.name.is_empty() {
            return Type::Bool; // parse error already emitted
        }
        // Record the variant type name as referenced so an import used exclusively
        // via `is TypeName` expression is not flagged as unused.
        self.referenced_names.insert(type_path.name.clone());
        match &scrutinee_ty {
            Type::Union { variants } => {
                let variant_names: Vec<String> = variants
                    .iter()
                    .filter_map(|v| {
                        if let Type::Shape { name } = v {
                            Some(name.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if !variant_names.contains(&type_path.name) {
                    self.diags.push(Diagnostic::error(
                        type_path.span.clone(),
                        format!("`{}` is not a variant of this union.", type_path.name),
                        format!("Valid variants are: {}", variant_names.join(", ")),
                        "The `is` check must name one of the union's declared variants.",
                    ));
                }
            }
            Type::Error => {}
            other => {
                // INFO-level: is-check on non-union (always true or wrong).
                // For now emit a regular error for structural correctness.
                self.diags.push(Diagnostic::error(
                    type_path.span.clone(),
                    format!("`is {}` used on `{}` which is not a union type.", type_path.name, type_name(other)),
                    "The `is TypeName` check is for union types: `shape S = A | B`.",
                    "Union types have multiple variants — `is` checks which variant a value is at runtime.",
                ));
            }
        }
        Type::Bool
    }

    // ── M6: options typeck helpers ────────────────────────────────────────────

    /// Typecheck an options value access: `OptionsTypeName.variantName`.
    ///
    /// Called from the `Expr::FieldAccess` handler when the receiver is an identifier
    /// that names an options type. Returns `Type::Options { name }` on success.
    fn check_options_value(&mut self, type_name: &str, variant: &str, span: &SourceSpan) -> Type {
        let entry = self.options_table.get(type_name).unwrap(); // caller verified contains()
                                                                // Record the options type name as referenced so an import used exclusively
                                                                // via variant access (`Timeframe.fiveMinute`) is not flagged as unused.
        self.referenced_names.insert(type_name.to_string());
        if entry.variants.contains(&variant.to_string()) {
            Type::Options {
                name: type_name.to_string(),
            }
        } else {
            let valid: Vec<&str> = entry.variants.iter().map(String::as_str).collect();
            self.diags.push(Diagnostic::error(
                span.clone(),
                format!("`{type_name}` has no variant named `{variant}`."),
                format!("Valid variants are: {}", valid.join(", ")),
                "Options variants must be declared in the `options` type body.",
            ));
            Type::Error
        }
    }

    /// Typecheck an `OptionName` arm in a multi-case `if`.
    ///
    /// Validates: scrutinee is an options type; variant name is valid for that type.
    /// Resolve an AST type annotation to a typeck `Type` for the crossing-local guard.
    ///
    /// Handles union aliases, maybe, dynamic, and inline union types. Returns `None` for
    /// types that cannot be classified without mutating the checker (e.g., unknown type names
    /// that would push an error). Callers use the result only for the
    /// UnsupportedCrossingLocalType guard; unresolved annotations are silently skipped.
    fn resolve_type_for_guard(&self, ast_ty: &AstType) -> Option<Type> {
        resolve_type_for_guard_free(ast_ty, &self.union_aliases)
    }

    fn check_option_name_arm(
        &mut self,
        scrutinee_ty: &Type,
        variant_name: &str,
        span: &SourceSpan,
    ) {
        match scrutinee_ty {
            Type::Options { name: opts_name } => {
                if let Some(entry) = self.options_table.get(opts_name) {
                    if !entry.variants.contains(&variant_name.to_string()) {
                        let valid: Vec<&str> = entry.variants.iter().map(String::as_str).collect();
                        self.diags.push(Diagnostic::error(
                            span.clone(),
                            format!("`{opts_name}` has no variant `{variant_name}`."),
                            format!("Valid variants are: {}", valid.join(", ")),
                            "Each arm in a multi-case `if` over an options type must name one of the declared variants.",
                        ));
                    }
                }
            }
            Type::Error => {} // already reported upstream
            other => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("Cannot use variant-name arm `{variant_name}` on a `{}` value.", type_name(other)),
                    "Variant-name arms are for options types: `options Status { active, inactive }`.",
                    "The `variantName =>` arm form matches against named options variants. \
                     Use `is TypeName =>` to narrow a union, or a value pattern for other types.",
                ));
            }
        }
    }
}

/// Resolve a simple AST type to a typeck Type using available table info.
///
/// Used for union alias resolution before the full Checker is built.
/// Only handles: Named shapes, union of Named shapes, primitives.
fn resolve_alias_type(ast_ty: &AstType, shape_table: &crate::shapes::ShapeTable) -> Type {
    match ast_ty {
        AstType::Int => Type::Int,
        AstType::Float => Type::Float,
        AstType::Bool => Type::Bool,
        AstType::Number { precision } => Type::Number {
            precision: *precision,
        },
        AstType::Named(n, _) if n == "string" => Type::String,
        AstType::Named(n, _) if shape_table.contains(n) => Type::Shape { name: n.clone() },
        AstType::Union { variants, .. } => {
            let resolved: Vec<Type> = variants
                .iter()
                .map(|v| resolve_alias_type(v, shape_table))
                .collect();
            if resolved.len() < 2 {
                Type::Error
            } else {
                Type::Union { variants: resolved }
            }
        }
        _ => Type::Error,
    }
}

/// Collect `shape Name = Type` alias declarations from the module.
///
/// These are union type aliases like `shape Shape = Circle | Square | Triangle`.
/// The alias name maps to the resolved alias type.
pub(crate) fn collect_union_aliases(
    module: &Module,
    shape_table: &crate::shapes::ShapeTable,
) -> HashMap<String, Type> {
    let mut aliases = HashMap::new();
    for item in &module.items {
        if let Item::ShapeDecl(sd) = item {
            if let Some(alias_ast_ty) = &sd.alias_ty {
                let resolved = resolve_alias_type(alias_ast_ty, shape_table);
                aliases.insert(sd.name.clone(), resolved);
            }
        }
    }
    aliases
}

/// Check whether two types are compatible for assignment.
///
/// This is mostly structural equality, with one exception: `BuiltinFixed` ignores the
/// `size` field so that `let f: fixed<int> = [1, 2, 3]` does not fail (annotation has
/// `size: None`; the literal infers `size: Some(3)`).
fn types_compatible(a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::BuiltinFixed { elem: ea, .. }, Type::BuiltinFixed { elem: eb, .. }) => {
            types_compatible(ea, eb)
        }
        (Type::BuiltinArray { elem: ea }, Type::BuiltinArray { elem: eb }) => {
            types_compatible(ea, eb)
        }
        (Type::Maybe { inner: ia }, Type::Maybe { inner: ib }) => types_compatible(ia, ib),
        // M6: union type compatibility — same set of variants (order-insensitive for now).
        (Type::Union { variants: va }, Type::Union { variants: vb }) => {
            va.len() == vb.len()
                && va
                    .iter()
                    .zip(vb.iter())
                    .all(|(a, b)| types_compatible(a, b))
        }
        // M6: assigning a concrete variant type to a union is valid.
        // e.g., `let s: Circle | Square = { radius: 5.0 }` — Circle is a valid union value.
        (Type::Union { variants }, concrete) => {
            variants.iter().any(|v| types_compatible(v, concrete))
        }
        // Symmetric: a concrete type is compatible with a union if it matches any variant.
        // e.g., map<string,string> is valid for map<string,string> | nothing.
        (concrete, Type::Union { variants }) => {
            variants.iter().any(|v| types_compatible(concrete, v))
        }
        // M7 P3a: ErrorsCapable is compatible with itself when inner types match.
        (Type::ErrorsCapable { inner: ia }, Type::ErrorsCapable { inner: ib }) => {
            types_compatible(ia, ib)
        }
        // M8 P6: mixed-precision number compatibility.
        // Any number<A> is compatible with any number<B> — widening always succeeds;
        // narrowing (A > B) will emit a warning at the call site. Here we just
        // allow the assignment so the program can type-check. The narrowing warning
        // is emitted in check_let_stmt when we detect precision shrinkage.
        (Type::Number { .. }, Type::Number { .. }) => true,
        _ => a == b,
    }
}

/// Return whether `ty` can appear inside a string interpolation `${}`.
///
/// Primitive types are always stringifiable (they have implicit `.toString()`).
/// Shape types are stringifiable when a standalone `toString` function exists
/// whose first parameter is that shape type. All other types are not stringifiable.
fn is_stringifiable(ty: &Type, sig_table: &crate::signatures::SignatureTable) -> bool {
    match ty {
        Type::String | Type::Int | Type::Float | Type::Bool => true,
        Type::Number { .. } => true,
        Type::Error => true, // suppress cascade errors from upstream type failures
        Type::Options { .. } => true, // .toString() built-in on options types
        Type::Shape { name } => {
            // A shape is stringifiable if there's a standalone `toString` function
            // whose first parameter type is `Shape { name }`.
            if let Some(sig) = sig_table.fns.get("toString") {
                if let Some((_, first_ty)) = sig.params.first() {
                    return first_ty == &Type::Shape { name: name.clone() };
                }
            }
            false
        }
        _ => false,
    }
}

/// Return the typeck `Type` for a type-attached constant like `int.max` or `number.epsilon`.
///
/// Returns `None` if the (type_name, const_name) pair is not a known constant.
/// All data lives in registry/features.toml — edit that file to add new constants.
pub fn type_attached_const_type(type_name: &str, const_name: &str) -> Option<Type> {
    let entry = ynz_registry::type_attached_constant_lookup(type_name, const_name)?;
    Some(match entry.value_type {
        "int" => Type::Int,
        "float" => Type::Float,
        "number" => Type::Number { precision: 34 },
        other => panic!(
            "type_attached_const_type: unknown value_type {other:?} for {type_name}.{const_name}"
        ),
    })
}

// ── Option-B deferral checks (v0.3-M2 scope boundary) ────────────────────────
//
// v0.3-M2 supports `wait` at the top level of a function and inside `if` blocks.
// Two patterns require the full coroutine-locals transform that lands in M3:
//
//   1. `wait` inside a `while`/`for`/`match` body — the loop counter must survive
//      the pause point, which needs frame-backed mutable locals (M3 machinery).
//   2. A local binding (`let`/`const`) declared before a `wait` and read after it —
//      the binding would need a frame slot and flush-before-suspend discipline,
//      also M3 machinery.
//
// Function parameters are exempt from (2): they ARE frame-backed (slot-loaded at
// each resume point), so a param read after a `wait` is correct and accepted.
//
// These helpers run as a pre-pass in `check_function` for functions that contain
// any `wait`, and emit clean teaching errors instead of letting the codegen no-op
// or crash.

/// Returns true if the block contains any `wait` expression anywhere in its tree.
pub(crate) fn block_contains_wait(block: &Block) -> bool {
    block.stmts.iter().any(stmt_contains_wait_anywhere)
}

fn stmt_contains_wait_anywhere(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Expr(e) => expr_contains_wait_anywhere(e),
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_contains_wait_anywhere(value),
        Stmt::If { cond, body, .. } => {
            expr_contains_wait_anywhere(cond) || block_contains_wait(body)
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            expr_contains_wait_anywhere(scrutinee)
                || arms.iter().any(|a| block_contains_wait(&a.body))
                || else_arm.as_ref().is_some_and(block_contains_wait)
        }
        Stmt::While { cond, body, .. }
        | Stmt::For {
            iter: cond, body, ..
        } => expr_contains_wait_anywhere(cond) || block_contains_wait(body),
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_contains_wait_anywhere),
        Stmt::FieldAssign { target, value, .. } => {
            expr_contains_wait_anywhere(target) || expr_contains_wait_anywhere(value)
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            expr_contains_wait_anywhere(receiver)
                || expr_contains_wait_anywhere(index)
                || expr_contains_wait_anywhere(value)
        }
    }
}

fn expr_contains_wait_anywhere(expr: &Expr) -> bool {
    match expr {
        Expr::Wait(_, _) => true,
        Expr::Background(inner, _) => expr_contains_wait_anywhere(inner),
        Expr::Call(c) => {
            expr_contains_wait_anywhere(&c.callee) || c.args.iter().any(expr_contains_wait_anywhere)
        }
        Expr::BinOp { lhs, rhs, .. } => {
            expr_contains_wait_anywhere(lhs) || expr_contains_wait_anywhere(rhs)
        }
        Expr::UnaryOp { operand, .. } => expr_contains_wait_anywhere(operand),
        Expr::MethodCall { receiver, args, .. } => {
            expr_contains_wait_anywhere(receiver) || args.iter().any(expr_contains_wait_anywhere)
        }
        Expr::FieldAccess { receiver, .. } => expr_contains_wait_anywhere(receiver),
        Expr::IndexAccess {
            receiver, index, ..
        } => expr_contains_wait_anywhere(receiver) || expr_contains_wait_anywhere(index),
        Expr::StructLit { fields, .. } => {
            fields.iter().any(|f| expr_contains_wait_anywhere(&f.value))
        }
        Expr::ArrayLit { elements, .. } => elements.iter().any(expr_contains_wait_anywhere),
        Expr::MapLit { entries, .. } => entries
            .iter()
            .any(|(k, v)| expr_contains_wait_anywhere(k) || expr_contains_wait_anywhere(v)),
        Expr::PostfixOp { receiver, .. } => expr_contains_wait_anywhere(receiver),
        Expr::Is { expr: inner, .. } => expr_contains_wait_anywhere(inner),
        Expr::InterpolatedString(parts, _) => parts.iter().any(|p| match p {
            ynz_ast::nodes::StringPart::Expr(e, _) => expr_contains_wait_anywhere(e),
            ynz_ast::nodes::StringPart::Lit(_, _) => false,
        }),
        // Leaf nodes — no wait nested here.
        Expr::Ident(_, _)
        | Expr::StringLit(_, _)
        | Expr::IntLit(_, _)
        | Expr::NumberLit(_, _)
        | Expr::BoolLit(_, _)
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. }
        | Expr::Error(_) => false,
    }
}

/// Returns `true` if any statement in `block` is a statement-position inferred-suspension
/// call — i.e., a bare `Stmt::Expr(Expr::Call)` or `Stmt::Let { value: Expr::Call, .. }`
/// where `is_suspending_call` is true.
///
/// Used by `collect_crossings_in_stmts` to decide whether an `if`-body contains a
/// suspension point when no explicit `wait` token is present.
fn block_contains_inferred_suspension(
    block: &Block,
    suspending: &std::collections::HashSet<&str>,
    expr_types: &HashMap<(usize, usize), Type>,
) -> bool {
    block.stmts.iter().any(|stmt| match stmt {
        Stmt::Expr(Expr::Call(c)) => is_suspending_call(c, suspending),
        Stmt::Let {
            value: Expr::Call(c),
            ..
        } => is_suspending_call(c, suspending),
        // v0.3-M4: a suspending conduit-method statement (`ch.send(v)` / `let x = ch.receive()`).
        Stmt::Expr(e) | Stmt::Let { value: e, .. } if expr_is_conduit_suspend(e, expr_types) => {
            true
        }
        // Recurse into nested `if` bodies — a branch inside a branch can suspend.
        Stmt::If { body, .. } => {
            block_contains_wait(body)
                || block_contains_inferred_suspension(body, suspending, expr_types)
        }
        _ => false,
    })
}

/// v0.3-M4 Phase 2: true when `expr` is a suspending conduit-method call at its ROOT —
/// `ch.send(v)` / `ch.receive()` / `h.send(v)` / `h.receive()` (optionally under an explicit
/// `wait`) on a plain-ident receiver whose typeck-resolved type is `channel<T>` or a
/// background task handle.
///
/// Receiver-type classification threads the ONE authoritative
/// [`crate::suspension_source::channel_method_suspends`] — never a second list. Typeck's
/// receiver/statement-position discipline (see `check_conduit_method_call`) guarantees every
/// conduit-method suspension in a well-typed program matches this ROOT-shape predicate.
pub fn expr_is_conduit_suspend(expr: &Expr, expr_types: &HashMap<(usize, usize), Type>) -> bool {
    let inner = match expr {
        Expr::Wait(inner, _) => inner.as_ref(),
        other => other,
    };
    let Expr::MethodCall {
        receiver, method, ..
    } = inner
    else {
        return false;
    };
    let Expr::Ident(_, rspan) = receiver.as_ref() else {
        return false;
    };
    let receiver_is_conduit = matches!(
        expr_types.get(&(rspan.start, rspan.end)),
        Some(Type::BuiltinChannel { .. } | Type::BackgroundHandle { .. })
    );
    crate::suspension_source::channel_method_suspends(receiver_is_conduit, method)
}

/// v0.3-M4 Phase 2: true when `stmt` is a suspending conduit-method statement — a bare
/// `ch.send(v)` / `ch.receive()` expression statement or a `let x = ch.receive()` binding.
pub fn stmt_is_conduit_suspend(stmt: &Stmt, expr_types: &HashMap<(usize, usize), Type>) -> bool {
    match stmt {
        Stmt::Expr(e) | Stmt::Let { value: e, .. } => expr_is_conduit_suspend(e, expr_types),
        _ => false,
    }
}

/// v0.3-M4 Phase 2: true when `stmt` CONTAINS a suspending conduit-method statement at any
/// nesting depth (the statement itself, or inside an `if`/`while`/`for`/`match` body).
/// The SM-lowering router uses this the same way it uses `stmt_contains_wait` — a statement
/// containing a conduit suspension must route through the SM walker so the nested suspension
/// consumes its continuation state.
pub fn stmt_contains_conduit_suspend(
    stmt: &Stmt,
    expr_types: &HashMap<(usize, usize), Type>,
) -> bool {
    if stmt_is_conduit_suspend(stmt, expr_types) {
        return true;
    }
    let block_contains = |b: &Block| {
        b.stmts
            .iter()
            .any(|s| stmt_contains_conduit_suspend(s, expr_types))
    };
    match stmt {
        Stmt::If { body, .. } | Stmt::While { body, .. } | Stmt::For { body, .. } => {
            block_contains(body)
        }
        Stmt::Match { arms, else_arm, .. } => {
            arms.iter().any(|a| block_contains(&a.body))
                || else_arm.as_ref().is_some_and(block_contains)
        }
        _ => false,
    }
}

/// Look up the typeck-resolved `Type` for a `let` or `const` binding named `target`
/// by scanning `stmts` and reading the resolved type from the typed module's expr_types.
///
/// Used by the nested-shape crossing guard (Check 2) to get the authoritative type
/// for a crossing local, including inferred types that may differ from the annotation.
pub fn find_crossing_local_typeck_type(
    stmts: &[Stmt],
    target: &str,
    typed: &TypedModule,
) -> Option<Type> {
    find_crossing_local_typeck_type_in_map(stmts, target, &typed.expr_types)
}

/// Implementation of type lookup using an expr_types map directly.
/// Called from Check 2 after check_stmts runs (where self.expr_types is populated)
/// and from the public helper above.
pub(crate) fn find_crossing_local_typeck_type_in_map(
    stmts: &[Stmt],
    target: &str,
    expr_types: &HashMap<(usize, usize), Type>,
) -> Option<Type> {
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, value, .. } if name == target => {
                let key = (value.span().start, value.span().end);
                return expr_types.get(&key).cloned();
            }
            Stmt::If { body, .. } => {
                if let Some(t) =
                    find_crossing_local_typeck_type_in_map(&body.stmts, target, expr_types)
                {
                    return Some(t);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                if let Some(t) =
                    find_crossing_local_typeck_type_in_map(&body.stmts, target, expr_types)
                {
                    return Some(t);
                }
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let Some(t) =
                        find_crossing_local_typeck_type_in_map(&arm.body.stmts, target, expr_types)
                    {
                        return Some(t);
                    }
                }
                if let Some(eb) = else_arm {
                    if let Some(t) =
                        find_crossing_local_typeck_type_in_map(&eb.stmts, target, expr_types)
                    {
                        return Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Scan `stmts` for a `for` loop whose variable name is `target` and return the
/// element type of its iterator, derived from `expr_types`.
///
/// For-loop variables are bound by the iteration mechanism, not via `Stmt::Let`, so
/// neither `find_crossing_local_typeck_type_in_map` nor `find_let_annotation_type_in_stmts`
/// can find their type. This function fills that gap for Check 2b
/// (UnsupportedCrossingLocalType), which needs to know when a for-loop var over a map
/// (yielding `MapEntry`) or a fixed array crosses a `wait`.
///
/// Returns the ITERATOR's element type (e.g. `Type::MapEntry{..}` for a map iter,
/// `elem` for an array iter), or `None` if the target name is not a for-loop var.
pub(crate) fn find_for_loop_var_type_in_stmts(
    stmts: &[Stmt],
    target: &str,
    expr_types: &HashMap<(usize, usize), Type>,
) -> Option<Type> {
    for stmt in stmts {
        match stmt {
            Stmt::For {
                var, iter, body, ..
            } if var == target => {
                let key = (iter.span().start, iter.span().end);
                let iter_ty = expr_types.get(&key)?;
                return match iter_ty {
                    Type::BuiltinArray { elem } | Type::BuiltinFixed { elem, .. } => {
                        Some(*elem.clone())
                    }
                    Type::BuiltinMap { key: k, val: v } => Some(Type::MapEntry {
                        key: k.clone(),
                        val: v.clone(),
                    }),
                    Type::Range { .. } => Some(Type::Int),
                    _ => None,
                };
            }
            Stmt::If { body, .. } => {
                if let Some(t) = find_for_loop_var_type_in_stmts(&body.stmts, target, expr_types) {
                    return Some(t);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                if let Some(t) = find_for_loop_var_type_in_stmts(&body.stmts, target, expr_types) {
                    return Some(t);
                }
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let Some(t) =
                        find_for_loop_var_type_in_stmts(&arm.body.stmts, target, expr_types)
                    {
                        return Some(t);
                    }
                }
                if let Some(eb) = else_arm {
                    if let Some(t) = find_for_loop_var_type_in_stmts(&eb.stmts, target, expr_types)
                    {
                        return Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Scan `stmts` for the first `let`/`const` binding named `target` and return its
/// annotation AST type (the `ty` field), if any.
///
/// Used by Check 2b (UnsupportedCrossingLocalType) to read the annotation type of a
/// crossing local without going through the mutating `ast_type_to_type` path.
fn find_let_annotation_type_in_stmts(stmts: &[Stmt], target: &str) -> Option<AstType> {
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, ty, .. } if name == target => {
                return ty.clone();
            }
            Stmt::If { body, .. } => {
                if let Some(t) = find_let_annotation_type_in_stmts(&body.stmts, target) {
                    return Some(t);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                if let Some(t) = find_let_annotation_type_in_stmts(&body.stmts, target) {
                    return Some(t);
                }
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let Some(t) = find_let_annotation_type_in_stmts(&arm.body.stmts, target) {
                        return Some(t);
                    }
                }
                if let Some(eb) = else_arm {
                    if let Some(t) = find_let_annotation_type_in_stmts(&eb.stmts, target) {
                        return Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Find the span of a `let`/`const` binding or for-loop header named `target` in `stmts`.
/// Returns `None` if the binding is not found (should not happen for valid crossings).
fn find_crossing_local_span(stmts: &[Stmt], target: &str) -> Option<SourceSpan> {
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, span, .. } if name == target => {
                return Some(span.clone());
            }
            // For-loop variables are bound by the for header, not a Stmt::Let.
            // Point the error at the for statement itself so the user sees the loop.
            Stmt::For { var, span, .. } if var == target => {
                return Some(span.clone());
            }
            Stmt::If { body, .. } => {
                if let Some(s) = find_crossing_local_span(&body.stmts, target) {
                    return Some(s);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                if let Some(s) = find_crossing_local_span(&body.stmts, target) {
                    return Some(s);
                }
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let Some(s) = find_crossing_local_span(&arm.body.stmts, target) {
                        return Some(s);
                    }
                }
                if let Some(eb) = else_arm {
                    if let Some(s) = find_crossing_local_span(&eb.stmts, target) {
                        return Some(s);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Returns `true` if `target` has a `let` declaration at the TOP LEVEL of `stmts`
/// that appears BEFORE any suspension point (explicit `wait` or inferred-suspension
/// call). Used to guard shadow detection: a crossing local that is only defined inside
/// a nested block, or whose top-level `let` appears AFTER all suspensions, cannot be
/// shadowed by the same name elsewhere — the outer `let` is not itself a crossing local.
fn has_top_level_let_before_suspension(
    stmts: &[Stmt],
    target: &str,
    suspending: &std::collections::HashSet<&str>,
    expr_types: &HashMap<(usize, usize), Type>,
) -> bool {
    for stmt in stmts {
        match stmt {
            // A suspension point before any top-level `let target` → the target is inner-only.
            Stmt::Expr(Expr::Wait(_, _)) => return false,
            Stmt::Expr(Expr::Call(c)) if is_suspending_call(c, suspending) => return false,
            // v0.3-M4: a bare conduit-method suspension (`ch.send(v)` / `ch.receive()`).
            Stmt::Expr(e) if expr_is_conduit_suspend(e, expr_types) => return false,
            // v0.3-M4: a conduit-suspend result binding — target's own producing suspension
            // makes it a top-level crossing candidate; a different binding's is a suspension.
            Stmt::Let { name, value, .. } if expr_is_conduit_suspend(value, expr_types) => {
                return name == target;
            }
            Stmt::Let {
                name,
                value: Expr::Wait(_, _),
                ..
            } if name == target => {
                // The target itself is a result-binding of a wait — it crosses by its OWN
                // wait, so it's a top-level crossing candidate.
                return true;
            }
            Stmt::Let {
                name,
                value: Expr::Wait(_, _),
                ..
            } => {
                // A DIFFERENT result-binding wait — counts as a suspension point.
                let _ = name;
                return false;
            }
            Stmt::Let {
                name,
                value: Expr::Call(c),
                ..
            } if is_suspending_call(c, suspending) && name != target => {
                // A different result-binding via suspending call — suspension point.
                return false;
            }
            // An `if` body containing a wait is a suspension point for the outer sequence.
            Stmt::If { body, .. }
                if block_contains_wait(body)
                    || block_contains_inferred_suspension(body, suspending, expr_types) =>
            {
                return false;
            }
            // A `while` or `for` body containing a wait is equally a suspension point for
            // the outer sequence — the loop may suspend and resume, so any `let target`
            // appearing AFTER the loop is past a suspension boundary, not before it.
            Stmt::While { body, .. } | Stmt::For { body, .. }
                if block_contains_wait(body)
                    || block_contains_inferred_suspension(body, suspending, expr_types) =>
            {
                return false;
            }
            // A `match` arm containing a wait is also a suspension point for the outer
            // sequence.
            Stmt::Match { arms, else_arm, .. }
                if arms.iter().any(|a| {
                    block_contains_wait(&a.body)
                        || block_contains_inferred_suspension(&a.body, suspending, expr_types)
                }) || else_arm.as_ref().is_some_and(|eb| {
                    block_contains_wait(eb)
                        || block_contains_inferred_suspension(eb, suspending, expr_types)
                }) =>
            {
                return false;
            }
            // A top-level `let target` found before any suspension.
            Stmt::Let { name, .. } if name == target => return true,
            _ => {}
        }
    }
    false
}

/// Returns `true` if `target` is re-declared with a `let` inside any nested scope
/// (if/while/for/match body) within `stmts`, AND there is also an outer `let target`
/// at the top level of `stmts` that establishes the crossing local.
///
/// This distinguishes two cases:
///   (a) Shadow: outer `let x = 10` at top level, inner `let x = 99` in nested scope.
///       Both exist → this is a shadow. Return true.
///   (b) Sole nested definition: crossing local `let inner = 42` is ONLY defined inside
///       a nested scope, no outer `let inner` exists at top level.
///       Only inner exists → NOT a shadow. Return false.
///
/// Case (a) is rejected at typeck (ambiguous name across suspension boundary).
/// Case (b) is handled by codegen: the sm_entry alloca is reused regardless of depth.
fn find_shadow_in_stmts(stmts: &[Stmt], target: &str) -> bool {
    // Check: is there an outer top-level `let target` definition?
    let has_outer_def = stmts
        .iter()
        .any(|stmt| matches!(stmt, Stmt::Let { name, .. } if name == target));
    if !has_outer_def {
        // No outer definition at this level — any nested `let target` is the SOLE
        // definition of this crossing local, not a shadow.
        return false;
    }
    // Outer definition exists: now check if there's also a re-declaration inside
    // any nested scope.
    for stmt in stmts {
        match stmt {
            Stmt::If { body, .. } if let_in_stmts_at_top_or_nested(&body.stmts, target) => {
                return true;
            }
            Stmt::While { body, .. } | Stmt::For { body, .. }
                if let_in_stmts_at_top_or_nested(&body.stmts, target) =>
            {
                return true;
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let_in_stmts_at_top_or_nested(&arm.body.stmts, target) {
                        return true;
                    }
                }
                if let Some(eb) = else_arm {
                    if let_in_stmts_at_top_or_nested(&eb.stmts, target) {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// Returns `true` if a `let target = ...` appears anywhere in `stmts` (at top level
/// of this list or in any nested scope within it).
fn let_in_stmts_at_top_or_nested(stmts: &[Stmt], target: &str) -> bool {
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, .. } if name == target => return true,
            Stmt::If { body, .. } if let_in_stmts_at_top_or_nested(&body.stmts, target) => {
                return true;
            }
            Stmt::While { body, .. } | Stmt::For { body, .. }
                if let_in_stmts_at_top_or_nested(&body.stmts, target) =>
            {
                return true;
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let_in_stmts_at_top_or_nested(&arm.body.stmts, target) {
                        return true;
                    }
                }
                if let Some(eb) = else_arm {
                    if let_in_stmts_at_top_or_nested(&eb.stmts, target) {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// Returns `true` if a `let target = ...` appears at the TOP LEVEL of `stmts` — not
/// inside any nested block. Used by Check 3b to detect top-level parameter shadowing.
///
/// Time: O(n) where n = len(stmts).
fn has_top_level_let_in_stmts(stmts: &[Stmt], target: &str) -> bool {
    stmts
        .iter()
        .any(|stmt| matches!(stmt, Stmt::Let { name, .. } if name == target))
}

/// Returns `true` if a `let target = ...` appears at the TOP LEVEL of `stmts` AFTER the
/// first suspension point.
///
/// Used by Check 3 shape (b): a crossing local (pre-wait binding exists) that is also
/// re-declared at the top level after a suspension has a guaranteed frame-slot collision
/// regardless of whether any read resolves to the outer or redeclared binding.
///
/// Time: O(n) where n = len(stmts).
fn has_top_level_let_after_suspension(
    stmts: &[Stmt],
    target: &str,
    suspending: &std::collections::HashSet<&str>,
    expr_types: &HashMap<(usize, usize), Type>,
) -> bool {
    let Some(susp_idx) = first_top_level_suspension_idx(stmts, suspending, expr_types) else {
        return false;
    };
    stmts[susp_idx + 1..]
        .iter()
        .any(|stmt| matches!(stmt, Stmt::Let { name, .. } if name == target))
}

/// Returns the index of the FIRST top-level suspension point in `stmts`, or `None`
/// if there is no suspension at the top level.
///
/// A "top-level suspension" is one of:
/// - An explicit `wait expr` statement
/// - A direct suspending-call statement
/// - A `let name = wait expr` or `let name = suspending_call(...)` binding
/// - An `if` body that itself contains a wait (the if is therefore a suspension point
///   for the enclosing sequence, because the resume_switch must be able to jump into it)
///
/// Used by `outer_is_genuine_crossing_local` to identify the slice of statements that
/// follow the first suspension — the slice that must be scanned for outer-binding reads
/// via `stmts_ref_target_non_shadowed_sequential`.
fn first_top_level_suspension_idx(
    stmts: &[Stmt],
    suspending: &std::collections::HashSet<&str>,
    expr_types: &HashMap<(usize, usize), Type>,
) -> Option<usize> {
    for (i, stmt) in stmts.iter().enumerate() {
        match stmt {
            Stmt::Expr(Expr::Wait(_, _)) => return Some(i),
            Stmt::Expr(Expr::Call(c)) if is_suspending_call(c, suspending) => return Some(i),
            // v0.3-M4: conduit-method suspension statements (`ch.send(v)` / `let x = ch.receive()`).
            s if stmt_is_conduit_suspend(s, expr_types) => return Some(i),
            Stmt::Let {
                value: Expr::Wait(_, _),
                ..
            } => return Some(i),
            Stmt::Let {
                value: Expr::Call(c),
                ..
            } if is_suspending_call(c, suspending) => return Some(i),
            Stmt::If { body, .. }
                if block_contains_wait(body)
                    || block_contains_inferred_suspension(body, suspending, expr_types) =>
            {
                return Some(i);
            }
            // A `while` or `for` body that contains a suspension is itself a suspension
            // point at the top level — each iteration may suspend, so any local declared
            // before the loop and read after it (or via the back-edge condition) crosses
            // a suspension boundary.
            Stmt::While { body, .. } | Stmt::For { body, .. }
                if block_contains_wait(body)
                    || block_contains_inferred_suspension(body, suspending, expr_types) =>
            {
                return Some(i);
            }
            // A `match` arm body that contains a suspension is also a top-level suspension
            // point — any local declared before the `match` and read after it crosses.
            Stmt::Match { arms, else_arm, .. }
                if arms.iter().any(|a| {
                    block_contains_wait(&a.body)
                        || block_contains_inferred_suspension(&a.body, suspending, expr_types)
                }) || else_arm.as_ref().is_some_and(|eb| {
                    block_contains_wait(eb)
                        || block_contains_inferred_suspension(eb, suspending, expr_types)
                }) =>
            {
                return Some(i);
            }
            _ => {}
        }
    }
    None
}

/// Returns `true` when `target` is a GENUINE top-level crossing local, gating the shadow
/// and top-level-redeclaration checks in Check 3.
///
/// Two cases both return `true` (either triggers the subsequent collision checks):
///
/// - **Unmasked post-wait read**: outer `let target` before suspension AND `target` is
///   read after a top-level suspension in a context where the read lexically resolves to
///   the outer binding (not masked by a same-level re-declaration).
///
/// - **Top-level redeclaration after suspension**: outer `let target` before suspension
///   AND another `let target` at the TOP LEVEL of `stmts` after the suspension. Even
///   when all post-wait reads resolve to the re-declared binding (not the outer one),
///   both bindings share the same name-keyed frame slot — the redeclaration clobbers the
///   outer value, producing a silent wrong answer. The caller (Check 3) emits a distinct
///   error for this shape via `has_top_level_let_after_suspension`.
///
/// Contrasting cases:
///   GENUINE (error, unmasked read): `let x=10; wait; print(x)` → outer x read after wait
///   GENUINE (error, deep shadow): `let x=10; wait; if{ if{let x=99}; print(x) }` →
///     `print(x)` at the outer if-body level resolves to outer x → error
///   GENUINE (error, top-level redecl): `let x=10; wait; let x=99; if{print(x)}` →
///     top-level `let x=99` after wait → slot collision → error (shape b in Check 3)
///   FALSE POSITIVE (outer read-only before wait): `let x=hi; print(x); if{let x=42;
///     wait; print(x)}` — outer x has NO read and NO redeclaration after any top-level
///     suspension → must NOT fire
fn outer_is_genuine_crossing_local(
    stmts: &[Stmt],
    target: &str,
    suspending: &std::collections::HashSet<&str>,
    expr_types: &HashMap<(usize, usize), Type>,
) -> bool {
    // Precondition: outer `let target` must exist before a top-level suspension.
    if !has_top_level_let_before_suspension(stmts, target, suspending, expr_types) {
        return false;
    }
    let Some(susp_idx) = first_top_level_suspension_idx(stmts, suspending, expr_types) else {
        return false;
    };
    let post_wait = &stmts[susp_idx + 1..];
    // Case 1: unmasked post-wait read resolving to the outer binding.
    if stmts_ref_target_non_shadowed_sequential(post_wait, target) {
        return true;
    }
    // Case 2: top-level re-declaration after the suspension (slot collision even when
    // all reads are masked). The sequential walker stops at `let target` and returns
    // false for reads, but the slot collision exists regardless.
    if post_wait
        .iter()
        .any(|stmt| matches!(stmt, Stmt::Let { name, .. } if name == target))
    {
        return true;
    }
    // Case 3: the first top-level suspension is a suspending loop (`while` or `for`) or a
    // suspending `match`. Both exhibit the back-edge problem:
    //
    // For loops — the condition/iter expression is re-evaluated on each iteration AFTER
    // the previous iteration's `wait` completes. A local read in the condition appears
    // textually before the `wait` in the source but is a post-suspension read at runtime.
    // A purely post-stmt scan (Cases 1-2) misses it because the back-edge read lives
    // inside the loop node, not in the statements that follow it.
    //
    // For match — any arm body containing a `wait` can reference outer locals. If the
    // match is the first top-level suspension and all post-match reads are masked, Cases
    // 1-2 miss those arm-internal reads.
    //
    // `stmt_refs_target_non_shadowed` handles `Stmt::While`, `Stmt::For`, AND
    // `Stmt::Match` — it scans the iter/cond and all arm bodies with correct inner-shadow
    // semantics — so one call covers any of these suspension nodes.
    if matches!(
        &stmts[susp_idx],
        Stmt::While { .. } | Stmt::For { .. } | Stmt::Match { .. }
    ) && stmt_refs_target_non_shadowed(&stmts[susp_idx], target)
    {
        return true;
    }
    false
}

/// Returns `true` when any nested block inside `stmts` (if/while/for/match body)
/// contains a `let target` declaration anywhere within it (at any depth).
///
/// Used by Check 3b Shape (a) to conservatively reject any nested param shadow in a
/// suspending function. The conservative guard is necessary because the frame-slot system
/// keys every crossing local and parameter by NAME — a nested `let pname` shares the
/// parameter's name-keyed slot, and every continuation state's `reload_params_from_frame`
/// overwrites `cg.locals[pname]` with the current slot pointer. Even a non-crossing
/// inner shadow would cause the reload to install the inner alloca into `cg.locals`,
/// corrupting the parameter across the next suspension.
///
/// The precise per-binding-ID lifting path (one slot per binding ID, keyed by span or
/// monotonic counter rather than by name) is tracked as roadmap M3c
/// (`v0-3-m3c-shadow-parity`). Until then, same-name reuse in a nested scope is a
/// safe-conservative compile error for all suspending functions.
///
/// Time: O(n) where n = total statement count in `stmts` (recursive).
pub(crate) fn param_has_nested_let_shadow(stmts: &[Stmt], target: &str) -> bool {
    for stmt in stmts {
        let bodies: Vec<&[Stmt]> = match stmt {
            Stmt::If { body, .. } => vec![&body.stmts],
            Stmt::While { body, .. } | Stmt::For { body, .. } => vec![&body.stmts],
            Stmt::Match { arms, else_arm, .. } => {
                let mut bs: Vec<&[Stmt]> = arms.iter().map(|a| a.body.stmts.as_slice()).collect();
                if let Some(eb) = else_arm {
                    bs.push(&eb.stmts);
                }
                bs
            }
            _ => continue,
        };
        for body in bodies {
            if let_in_stmts_at_top_or_nested(body, target) {
                return true;
            }
        }
    }
    false
}

/// Returns `true` if `stmt` contains ANY read of the identifier `name` — conservative
/// (may report true for shadowed names in nested scopes).
///
/// Used for `background` give/copy inference: safe direction is `.copy` (do not
/// consume the binding) whenever we cannot PROVE the name is dead after the spawn.
/// A false positive here only costs a copy (the safe choice); a false negative
/// (`.give` on a still-live binding) would be a use-after-move bug.
///
/// Time: O(stmt nodes).  Space: O(1).
fn ident_read_in_stmt(stmt: &Stmt, name: &str) -> bool {
    match stmt {
        Stmt::Expr(e) => expr_refs_ident(e, name),
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_refs_ident(value, name),
        Stmt::Return { value, .. } => value.as_ref().is_some_and(|e| expr_refs_ident(e, name)),
        Stmt::FieldAssign { target, value, .. } => {
            expr_refs_ident(target, name) || expr_refs_ident(value, name)
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            expr_refs_ident(receiver, name)
                || expr_refs_ident(index, name)
                || expr_refs_ident(value, name)
        }
        Stmt::If { cond, body, .. } => {
            expr_refs_ident(cond, name) || body.stmts.iter().any(|s| ident_read_in_stmt(s, name))
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            expr_refs_ident(scrutinee, name)
                || arms
                    .iter()
                    .any(|arm| arm.body.stmts.iter().any(|s| ident_read_in_stmt(s, name)))
                || else_arm
                    .as_ref()
                    .is_some_and(|b| b.stmts.iter().any(|s| ident_read_in_stmt(s, name)))
        }
        Stmt::While { cond, body, .. } => {
            expr_refs_ident(cond, name) || body.stmts.iter().any(|s| ident_read_in_stmt(s, name))
        }
        Stmt::For { iter, body, .. } => {
            expr_refs_ident(iter, name) || body.stmts.iter().any(|s| ident_read_in_stmt(s, name))
        }
    }
}

/// Returns `true` if `stmt` references `target` in a context where the reference
/// lexically resolves to an OUTER declaration — i.e., the nearest enclosing `let target`
/// at the point of the read is the outer binding, not an inner shadow.
///
/// Lexical resolution rule: a `let target` only shadows `target` within its own scope
/// (the block it is declared in) from its declaration point forward. It does NOT shadow
/// `target` in statements at the SAME level before the inner `let target` appears, and it
/// does NOT shadow `target` in statements AFTER a nested block whose INTERIOR declares
/// `target` (the inner scope has closed by then).
///
/// Correct examples:
///   `if { if { let x=99 }; print(x) }` — the `print(x)` is at the outer if-body level;
///     the inner `let x=99` is inside a deeper nested scope that has closed before `print`.
///     `print(x)` lexically resolves to the outer `x` → returns true.
///   `if { let x=42; print(x) }` — `let x=42` is at THIS level; `print(x)` after it
///     resolves to the INNER binding, not the outer → returns false.
///
/// Used by `outer_is_genuine_crossing_local`.
fn stmt_refs_target_non_shadowed(stmt: &Stmt, target: &str) -> bool {
    match stmt {
        Stmt::Expr(e) => expr_refs_ident(e, target),
        Stmt::Let { value, .. } => {
            // The RHS is evaluated before the new binding takes effect; even if this
            // stmt re-declares `target`, the RHS read resolves to the outer binding.
            expr_refs_ident(value, target)
        }
        Stmt::Assign { value, .. } => expr_refs_ident(value, target),
        Stmt::Return { value, .. } => value.as_ref().is_some_and(|e| expr_refs_ident(e, target)),
        Stmt::FieldAssign {
            target: recv,
            value,
            ..
        } => expr_refs_ident(recv, target) || expr_refs_ident(value, target),
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            expr_refs_ident(receiver, target)
                || expr_refs_ident(index, target)
                || expr_refs_ident(value, target)
        }
        Stmt::If { cond, body, .. } => {
            if expr_refs_ident(cond, target) {
                return true;
            }
            stmts_ref_target_non_shadowed_sequential(&body.stmts, target)
        }
        Stmt::While { cond, body, .. }
        | Stmt::For {
            iter: cond, body, ..
        } => {
            if expr_refs_ident(cond, target) {
                return true;
            }
            stmts_ref_target_non_shadowed_sequential(&body.stmts, target)
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            if expr_refs_ident(scrutinee, target) {
                return true;
            }
            for arm in arms {
                if stmts_ref_target_non_shadowed_sequential(&arm.body.stmts, target) {
                    return true;
                }
            }
            if let Some(eb) = else_arm {
                if stmts_ref_target_non_shadowed_sequential(&eb.stmts, target) {
                    return true;
                }
            }
            false
        }
    }
}

/// Walks `stmts` sequentially, returning `true` if any statement references `target`
/// where the reference lexically resolves to an outer binding (not an inner shadow
/// declared at THIS scope level).
///
/// A `let target` at the top level of `stmts` creates a shadow from its declaration
/// point forward within THIS scope. Statements before that `let` still resolve to the
/// outer binding. Statements after it at THIS level resolve to the inner binding and
/// are NOT counted. A `let target` inside a nested sub-block (e.g., `if { let target }`)
/// only shadows within that sub-block — it has NO effect on sibling statements at THIS
/// level, even those appearing AFTER the sub-block.
///
/// Time: O(n × d) where n = stmts count, d = nesting depth.
fn stmts_ref_target_non_shadowed_sequential(stmts: &[Stmt], target: &str) -> bool {
    for stmt in stmts {
        match stmt {
            // A `let target` AT THIS SCOPE LEVEL: the RHS resolves to the outer binding,
            // but all subsequent statements at this level now see the inner binding.
            // Return based only on the RHS, then stop (remaining stmts shadow the outer).
            Stmt::Let { name, value, .. } if name == target => {
                return expr_refs_ident(value, target);
            }
            // Any other statement: check if it references `target` resolving to the outer.
            s => {
                if stmt_refs_target_non_shadowed(s, target) {
                    return true;
                }
            }
        }
    }
    false
}

/// Returns `true` if `expr` contains an `Expr::Ident` node whose name is `target`.
fn expr_refs_ident(expr: &Expr, target: &str) -> bool {
    match expr {
        Expr::Ident(name, _) => name == target,
        Expr::Wait(inner, _) | Expr::Background(inner, _) => expr_refs_ident(inner, target),
        Expr::Call(c) => {
            expr_refs_ident(&c.callee, target) || c.args.iter().any(|a| expr_refs_ident(a, target))
        }
        Expr::BinOp { lhs, rhs, .. } => {
            expr_refs_ident(lhs, target) || expr_refs_ident(rhs, target)
        }
        Expr::UnaryOp { operand, .. } => expr_refs_ident(operand, target),
        Expr::MethodCall { receiver, args, .. } => {
            expr_refs_ident(receiver, target) || args.iter().any(|a| expr_refs_ident(a, target))
        }
        Expr::FieldAccess { receiver, .. } => expr_refs_ident(receiver, target),
        Expr::IndexAccess {
            receiver, index, ..
        } => expr_refs_ident(receiver, target) || expr_refs_ident(index, target),
        Expr::StructLit { fields, .. } => fields.iter().any(|f| expr_refs_ident(&f.value, target)),
        Expr::ArrayLit { elements, .. } => elements.iter().any(|e| expr_refs_ident(e, target)),
        Expr::MapLit { entries, .. } => entries
            .iter()
            .any(|(k, v)| expr_refs_ident(k, target) || expr_refs_ident(v, target)),
        Expr::PostfixOp { receiver, .. } => expr_refs_ident(receiver, target),
        Expr::Is { expr: inner, .. } => expr_refs_ident(inner, target),
        Expr::InterpolatedString(parts, _) => parts.iter().any(|p| {
            if let ynz_ast::nodes::StringPart::Expr(e, _) = p {
                expr_refs_ident(e, target)
            } else {
                false
            }
        }),
        Expr::StringLit(_, _)
        | Expr::IntLit(_, _)
        | Expr::NumberLit(_, _)
        | Expr::BoolLit(_, _)
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. }
        | Expr::Error(_) => false,
    }
}

/// Describes a local binding declared before a `wait` in a function body that is
/// also referenced after that wait.
pub struct LocalCrossesWait {
    /// Name of the local binding.
    pub name: String,
    /// Span of the usage AFTER the wait (for error reporting).
    pub use_span: SourceSpan,
}

/// Return the deduplicated set of local binding NAMES that cross a suspension
/// boundary in `f` — the subset that codegen must frame-back.
///
/// Excludes parameters (they already have frame slots). The result is a sorted,
/// deduplicated `Vec<String>` suitable for deterministic slot index assignment.
///
/// `expr_types` is required to detect map-iterator for-loops. For map loops the
/// loop variable (e.g. `entry`) is NOT added to crossing_names because the SM map
/// codegen re-creates the entry struct on each body-bb entry from ynz_map_iter_get —
/// it does not need a frame slot. Passing `None` disables this detection and falls
/// back to the old behaviour (adding the var for all non-destructure for-loops).
pub fn crossing_local_names(
    stmts: &[Stmt],
    param_names: &[&str],
    suspending: &std::collections::HashSet<&str>,
    expr_types: &HashMap<(usize, usize), Type>,
) -> Vec<String> {
    crossing_local_names_with_cpu_spike(
        stmts,
        param_names,
        suspending,
        &std::collections::HashSet::new(),
        expr_types,
    )
}

/// Crossing-local collection that additionally treats a CPU spike group's join as a
/// suspension point.
///
/// A pure-CPU parallel group (an adjacent pair of `let x = callee(...)` whose callees are
/// non-suspending and in `cpu_supported`) is spawned-and-join-polled by the M3d codegen,
/// and that join is a real suspension boundary: a local declared before the pair and read
/// after the join must survive across it via a frame slot, exactly like a local crossing a
/// `wait`. The plain [`crossing_local_names`] entry point passes an empty `cpu_supported`
/// set, so its behavior is unchanged for every caller that is not collecting slots for a
/// spike host.
///
/// `cpu_supported` is the set of callee names whose return class fits the CPU-result ABI —
/// the codegen spike-host's eligibility filter — and is non-empty ONLY when collecting for a
/// function the codegen will actually spike-host. This keeps the typeck crossing set and the
/// codegen frame reservation in lock-step: both recognize the same pair as the same
/// suspension. Polluting `suspending` with the CPU callees instead would make the codegen's
/// `!suspend_set.contains(callee)` eligibility filter drop the pair (the callees would look
/// like state machines), so the join would NOT fire while the crossing slot was reserved —
/// the divergence that corrupts. The join, not the callees, is the suspension.
///
/// Time: O(N log N) where N = AST nodes (one crossing scan + a final name sort)  Space: O(C)
/// where C = crossing local names collected
pub fn crossing_local_names_with_cpu_spike(
    stmts: &[Stmt],
    param_names: &[&str],
    suspending: &std::collections::HashSet<&str>,
    cpu_supported: &std::collections::HashSet<&str>,
    expr_types: &HashMap<(usize, usize), Type>,
) -> Vec<String> {
    let crossings = locals_crossing_wait(stmts, param_names, suspending, cpu_supported, expr_types);
    let mut seen = std::collections::HashSet::new();
    let mut names: Vec<String> = crossings
        .into_iter()
        .filter_map(|c| {
            if seen.insert(c.name.clone()) {
                Some(c.name)
            } else {
                None
            }
        })
        .collect();
    // v0.3-M4: every conduit-typed local (`channel<T>` / background task handle) is marked
    // crossing — a SOUND over-approximation (recorded in the plan). The conduit local IS the
    // receiver at its own suspension point: when `ch.send(v)` suspends, the resume path
    // re-polls through `ch`'s frame slot, so the binding must be frame-backed even when no
    // read appears lexically after the suspension (the natural read-after-suspension scan
    // above would miss exactly that case).
    collect_conduit_locals(stmts, param_names, expr_types, &mut seen, &mut names);
    // Collect synthetic frame slots for for-loops whose bodies contain a suspension.
    // For-loop iteration requires an internal index counter that must survive suspension;
    // giving it a named frame slot (prefixed `__ynz_for_idx_`) integrates it with the
    // existing crossing-local slot machinery. The name is deterministic and collision-free
    // (user code cannot declare names starting with `__ynz_`).
    // Only `wait`/may-block loop bodies suspend here: a CPU spike-group inside a loop body
    // DECLINES to sequential lowering (the codegen `spike_nested_blocks` excludes loop
    // bodies), so it is never a suspension point and needs no synthetic iteration slot.
    collect_for_loop_synthetic_crossings(stmts, suspending, &mut seen, &mut names, expr_types);
    names.sort();
    names
}

/// v0.3-M4: recursively collect every local whose binding type is a conduit
/// (`channel<T>` or a background task handle) into the crossing-name set.
///
/// Detection: the `let` value expression's typeck-resolved type (covers construction,
/// aliasing, and channel-returning calls) — the same span-keyed `expr_types` the rest of
/// the crossing analysis uses. Parameters are excluded (they always have frame slots).
///
/// Time: O(N) where N = AST nodes  Space: O(D) recursion depth
fn collect_conduit_locals(
    stmts: &[Stmt],
    param_names: &[&str],
    expr_types: &HashMap<(usize, usize), Type>,
    seen: &mut std::collections::HashSet<String>,
    names: &mut Vec<String>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let vspan = value.span();
                let is_conduit = matches!(
                    expr_types.get(&(vspan.start, vspan.end)),
                    Some(Type::BuiltinChannel { .. } | Type::BackgroundHandle { .. })
                );
                if is_conduit && !param_names.contains(&name.as_str()) && seen.insert(name.clone())
                {
                    names.push(name.clone());
                }
            }
            Stmt::If { body, .. } | Stmt::While { body, .. } | Stmt::For { body, .. } => {
                collect_conduit_locals(&body.stmts, param_names, expr_types, seen, names);
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    collect_conduit_locals(&arm.body.stmts, param_names, expr_types, seen, names);
                }
                if let Some(eb) = else_arm {
                    collect_conduit_locals(&eb.stmts, param_names, expr_types, seen, names);
                }
            }
            _ => {}
        }
    }
}

/// Recursively scan `stmts` for `for` loops whose bodies contain a suspension, and
/// add a synthetic crossing-local name for their internal index counter.
///
/// Each suspending for-loop gets one slot named `__ynz_for_idx_N` (N is a per-function
/// counter threaded through the recursion). This slot holds the iteration index across
/// suspension boundaries — the same mechanism user-declared crossing locals use.
///
/// The synthetic name is guaranteed not to alias user code because Yinz identifiers
/// may not start with `__ynz_` (reserved prefix).
///
/// Time: O(N) where N = AST nodes in `stmts`  Space: O(D) recursion depth + O(F) synthetic
/// names where F = suspending for-loops
fn collect_for_loop_synthetic_crossings(
    stmts: &[Stmt],
    suspending: &std::collections::HashSet<&str>,
    seen: &mut std::collections::HashSet<String>,
    names: &mut Vec<String>,
    expr_types: &HashMap<(usize, usize), Type>,
) {
    collect_for_loop_synthetic_crossings_inner(stmts, suspending, seen, names, &mut 0, expr_types);
}

/// Time: O(N) where N = AST nodes in `stmts`  Space: O(D) recursion depth
fn collect_for_loop_synthetic_crossings_inner(
    stmts: &[Stmt],
    suspending: &std::collections::HashSet<&str>,
    seen: &mut std::collections::HashSet<String>,
    names: &mut Vec<String>,
    counter: &mut usize,
    expr_types: &HashMap<(usize, usize), Type>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::For {
                var,
                iter,
                body,
                map_destructure_pattern,
                ..
            } if block_contains_wait(body)
                || block_contains_inferred_suspension(body, suspending, expr_types) =>
            {
                let syn_name = format!("__ynz_for_idx_{counter}");
                *counter += 1;
                if seen.insert(syn_name.clone()) {
                    names.push(syn_name);
                }
                // Add the loop variable as a crossing local so it survives suspension,
                // UNLESS the iterator is a map type. Map loops create a fresh {key,value}
                // entry struct on each body-bb entry from ynz_map_iter_get — the entry
                // var does NOT need a frame slot because it is rebound from the runtime
                // on each resume-call's body-bb pass. Adding `var` to crossing_names for
                // map loops causes a conflicting alloca: codegen pre-creates an i64 alloca
                // (misclassified from the Int fallback), then the SM map body creates a
                // fresh {i64,i64} struct alloca and overwrites cg.locals[var] — the
                // reload then writes to the wrong alloca with the wrong type. Map-entry
                // field accesses after a wait are caught separately by UnsupportedCrossingLocalType.
                //
                // Destructure loops (`for ((k,v) in m)`) use the synthetic `__entry` var,
                // which has the same {i64,i64} struct issue and is also excluded.
                let is_map_destructure = map_destructure_pattern.is_some();
                let is_map_iter = {
                    let key = (iter.span().start, iter.span().end);
                    matches!(expr_types.get(&key), Some(Type::BuiltinMap { .. }))
                };
                if !is_map_destructure && !is_map_iter && seen.insert(var.clone()) {
                    names.push(var.clone());
                }
                // Recurse into body for nested suspending for-loops.
                collect_for_loop_synthetic_crossings_inner(
                    &body.stmts,
                    suspending,
                    seen,
                    names,
                    counter,
                    expr_types,
                );
            }
            Stmt::If { body, .. } => {
                collect_for_loop_synthetic_crossings_inner(
                    &body.stmts,
                    suspending,
                    seen,
                    names,
                    counter,
                    expr_types,
                );
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                collect_for_loop_synthetic_crossings_inner(
                    &body.stmts,
                    suspending,
                    seen,
                    names,
                    counter,
                    expr_types,
                );
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    collect_for_loop_synthetic_crossings_inner(
                        &arm.body.stmts,
                        suspending,
                        seen,
                        names,
                        counter,
                        expr_types,
                    );
                }
                if let Some(eb) = else_arm {
                    collect_for_loop_synthetic_crossings_inner(
                        &eb.stmts, suspending, seen, names, counter, expr_types,
                    );
                }
            }
            _ => {}
        }
    }
}

/// Scan `stmts` for `for` loops whose body contains an explicit `wait` AND whose
/// iterator resolves to a `fixed<T>` array (an `Expr::Ident` with `Type::BuiltinFixed`).
///
/// `fixed<T>` arrays are stack-allocated in the resume function's stack frame. After
/// suspension the stack frame is freed; the pointer stored in the crossing-local frame
/// slot becomes dangling. Returns the span of the first such `for`, or `None`.
pub(crate) fn find_fixed_array_iter_wait_in_for(
    stmts: &[Stmt],
    expr_types: &std::collections::HashMap<(usize, usize), Type>,
) -> Option<SourceSpan> {
    for stmt in stmts {
        match stmt {
            Stmt::For {
                iter, body, span, ..
            } if block_contains_wait(body) => {
                // A fixed-array iterator is an identifier whose expr_types entry is BuiltinFixed.
                if let Expr::Ident(_, ident_span) = iter {
                    let key = (ident_span.start, ident_span.end);
                    if matches!(expr_types.get(&key), Some(Type::BuiltinFixed { .. })) {
                        return Some(span.clone());
                    }
                }
                // An inline literal `[...]` annotated as fixed<T> is also a BuiltinFixed.
                // Check the array literal's own span.
                if let Expr::ArrayLit { span: lit_span, .. } = iter {
                    let key = (lit_span.start, lit_span.end);
                    if matches!(expr_types.get(&key), Some(Type::BuiltinFixed { .. })) {
                        return Some(span.clone());
                    }
                }
                if let Some(s) = find_fixed_array_iter_wait_in_for(&body.stmts, expr_types) {
                    return Some(s);
                }
            }
            Stmt::If { body, .. } => {
                if let Some(s) = find_fixed_array_iter_wait_in_for(&body.stmts, expr_types) {
                    return Some(s);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                if let Some(s) = find_fixed_array_iter_wait_in_for(&body.stmts, expr_types) {
                    return Some(s);
                }
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let Some(s) = find_fixed_array_iter_wait_in_for(&arm.body.stmts, expr_types)
                    {
                        return Some(s);
                    }
                }
                if let Some(eb) = else_arm {
                    if let Some(s) = find_fixed_array_iter_wait_in_for(&eb.stmts, expr_types) {
                        return Some(s);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Scan `stmts` for `for` loops whose body contains an explicit `wait` AND whose
/// iterator is a stored range variable (an `Expr::Ident` with `Type::Range`).
///
/// Returns the span of the first such `for` statement found, or `None`.
/// The caller emits `StoredRangeWithWait` when `Some`.
pub(crate) fn find_stored_range_wait_in_for(
    stmts: &[Stmt],
    expr_types: &std::collections::HashMap<(usize, usize), Type>,
) -> Option<SourceSpan> {
    for stmt in stmts {
        match stmt {
            Stmt::For {
                iter, body, span, ..
            } if block_contains_wait(body) => {
                // A stored range variable is an `Ident` whose expr_types entry is Range.
                if let Expr::Ident(_, ident_span) = iter {
                    let key = (ident_span.start, ident_span.end);
                    if matches!(expr_types.get(&key), Some(Type::Range { .. })) {
                        return Some(span.clone());
                    }
                }
                // Recurse into body.
                if let Some(s) = find_stored_range_wait_in_for(&body.stmts, expr_types) {
                    return Some(s);
                }
            }
            Stmt::If { body, .. } => {
                if let Some(s) = find_stored_range_wait_in_for(&body.stmts, expr_types) {
                    return Some(s);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                if let Some(s) = find_stored_range_wait_in_for(&body.stmts, expr_types) {
                    return Some(s);
                }
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let Some(s) = find_stored_range_wait_in_for(&arm.body.stmts, expr_types) {
                        return Some(s);
                    }
                }
                if let Some(eb) = else_arm {
                    if let Some(s) = find_stored_range_wait_in_for(&eb.stmts, expr_types) {
                        return Some(s);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Scan `stmts` for `for` loops whose body contains an explicit `wait` AND whose
/// iterator is a call expression (not a plain identifier).
///
/// A call-expression iterator is re-evaluated by the SM codegen on every loop header
/// visit, producing N+1 evaluations instead of 1. Returns the span of the first such
/// `for` statement found, or `None`. The caller emits `ExpressionIterWithWait`.
pub(crate) fn find_expr_iter_wait_in_for(stmts: &[Stmt]) -> Option<SourceSpan> {
    for stmt in stmts {
        match stmt {
            Stmt::For {
                iter, body, span, ..
            } if block_contains_wait(body) => {
                // An expression iterator is anything other than a plain identifier or a
                // literal `range(...)` call. Plain identifiers are frame-backed crossing
                // locals — stable across resumes. Stored range idents are caught by
                // `find_stored_range_wait_in_for`. Inline `range(...)` calls ARE supported
                // by the SM codegen (extract_range_bounds handles them directly).
                let is_unsupported_call_expr = if let Expr::Call(c) = iter {
                    // Exclude `range(...)` — handled by extract_range_bounds in SM codegen.
                    !matches!(&c.callee, Expr::Ident(name, _) if name == "range")
                } else {
                    false
                };
                if is_unsupported_call_expr {
                    return Some(span.clone());
                }
                // Recurse into body.
                if let Some(s) = find_expr_iter_wait_in_for(&body.stmts) {
                    return Some(s);
                }
            }
            Stmt::If { body, .. } => {
                if let Some(s) = find_expr_iter_wait_in_for(&body.stmts) {
                    return Some(s);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                if let Some(s) = find_expr_iter_wait_in_for(&body.stmts) {
                    return Some(s);
                }
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let Some(s) = find_expr_iter_wait_in_for(&arm.body.stmts) {
                        return Some(s);
                    }
                }
                if let Some(eb) = else_arm {
                    if let Some(s) = find_expr_iter_wait_in_for(&eb.stmts) {
                        return Some(s);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Scan `stmts` for `for (entry in map)` loops whose body contains a `wait` AND
/// reads `entry.key` or `entry.value` AFTER the wait.
///
/// Map-iteration loop variables are bound fresh on each body-bb entry from
/// ynz_map_iter_get and do NOT have a crossing-local frame slot. If a `wait`
/// suspends the function mid-body, the entry struct lives on the now-freed resume
/// function's stack — reading entry fields after resume is a dangling-pointer
/// access (SIGSEGV). Returns the span of the offending `for` statement, or `None`.
fn find_map_entry_field_after_wait(
    stmts: &[Stmt],
    expr_types: &HashMap<(usize, usize), Type>,
) -> Option<SourceSpan> {
    for stmt in stmts {
        match stmt {
            Stmt::For {
                var,
                iter,
                body,
                map_destructure_pattern,
                span,
                ..
            } if map_destructure_pattern.is_none() && block_contains_wait(body) => {
                // Check if the iterator is a map type.
                let iter_key = (iter.span().start, iter.span().end);
                let is_map = matches!(expr_types.get(&iter_key), Some(Type::BuiltinMap { .. }));
                if is_map && body_reads_field_after_wait(&body.stmts, var.as_str()) {
                    return Some(span.clone());
                }
                // Recurse into body for nested for-loops.
                if let Some(s) = find_map_entry_field_after_wait(&body.stmts, expr_types) {
                    return Some(s);
                }
            }
            Stmt::If { body, .. } => {
                if let Some(s) = find_map_entry_field_after_wait(&body.stmts, expr_types) {
                    return Some(s);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                if let Some(s) = find_map_entry_field_after_wait(&body.stmts, expr_types) {
                    return Some(s);
                }
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let Some(s) = find_map_entry_field_after_wait(&arm.body.stmts, expr_types) {
                        return Some(s);
                    }
                }
                if let Some(eb) = else_arm {
                    if let Some(s) = find_map_entry_field_after_wait(&eb.stmts, expr_types) {
                        return Some(s);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Returns `true` if `stmts` (a for-loop body) contains a `wait` followed by a
/// field access of `entry_var` (i.e., `entry_var.key` or `entry_var.value`).
/// A wait appears before a field-access if at least one `wait` statement precedes
/// any statement that reads from `entry_var` via field access.
fn body_reads_field_after_wait(stmts: &[Stmt], entry_var: &str) -> bool {
    // stmt_contains_wait_anywhere recurses through if/while/match/for bodies, so a
    // `wait` nested inside `if (c) { wait sleep(5) }` is correctly detected. A flat
    // Stmt::Expr(Wait) match would miss nested waits and allow the SIGSEGV path.
    let mut seen_wait = false;
    for stmt in stmts {
        if !seen_wait && stmt_contains_wait_anywhere(stmt) {
            seen_wait = true;
        }
        if seen_wait && stmt_reads_field_of(stmt, entry_var) {
            return true;
        }
    }
    false
}

/// Scans `crossing_names` for any local whose initializer is an `array<Shape>` literal
/// with at least one struct element having a runtime-computed field value (not a
/// compile-time `IntLit` or `BoolLit`).
///
/// Returns the span of the first such crossing local and its name, or `None` if no
/// dangerous runtime-field `array<Shape>` crossing local is found.
///
/// The guard fires on the full `crossing_names` set: any name the crossing-analysis
/// considers in-scope across a suspension boundary (declared before one AND referenced
/// after one — including via an iterator expression in a for-loop). This is
/// intentionally conservative — some after-last-wait constructions also end up in
/// `crossing_names` because the crossing-analysis tracks them as reachable by the
/// subsequent for-loop iterator scan. The guard rejects those too, which is the safe
/// direction (loud over silent). The m3c-array-by-value milestone removes this guard
/// entirely by making runtime-field elements safe across any suspension.
///
/// All-literal struct elements (fields that are all `IntLit` or `BoolLit`) are safe:
/// codegen emits them as LLVM module-level globals with stable, eternal addresses.
/// Runtime-computed fields fall back to stack allocas that dangle after suspension.
fn find_array_shape_runtime_field_crossing(
    crossing_names: &[String],
    stmts: &[Stmt],
) -> Option<(SourceSpan, String)> {
    for name in crossing_names {
        // `crossing_names` is the conservative set from crossing_local_names: names
        // that are declared before a suspension AND referenced afterward (including
        // via iterator expressions in for-loops). The conservative scope means some
        // after-last-wait constructions can appear here too (safe direction: loud over
        // silent). The m3c-array-by-value milestone removes this guard entirely.
        if let Some(Expr::ArrayLit { elements, .. }) =
            find_let_initializer_in_stmts(stmts, name.as_str())
        {
            for elem in elements {
                if let Expr::StructLit { fields, .. } = elem {
                    if fields
                        .iter()
                        .any(|f| !expr_is_compile_time_literal(&f.value))
                    {
                        let span = find_crossing_local_span(stmts, name.as_str())
                            .unwrap_or_else(|| SourceSpan::new("", 0, 0));
                        return Some((span, name.clone()));
                    }
                }
            }
        }
    }
    None
}

/// Walk `stmts` to find the initializer expression of the first `let`/`const` binding
/// named `target`. Returns a reference to the value expression, or `None` if `target`
/// is not declared as a `let` in `stmts` (e.g., it is a for-loop var or a parameter).
fn find_let_initializer_in_stmts<'a>(stmts: &'a [Stmt], target: &str) -> Option<&'a Expr> {
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, value, .. } if name == target => {
                return Some(value);
            }
            Stmt::If { body, .. } => {
                if let Some(e) = find_let_initializer_in_stmts(&body.stmts, target) {
                    return Some(e);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                if let Some(e) = find_let_initializer_in_stmts(&body.stmts, target) {
                    return Some(e);
                }
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let Some(e) = find_let_initializer_in_stmts(&arm.body.stmts, target) {
                        return Some(e);
                    }
                }
                if let Some(eb) = else_arm {
                    if let Some(e) = find_let_initializer_in_stmts(&eb.stmts, target) {
                        return Some(e);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Returns `true` if `expr` is a struct-element field value that codegen can fold into
/// a stable LLVM module-level global via `try_build_shape_global`. Only `IntLit` and
/// `BoolLit` are handled by that function — all other forms produce a stack alloca that
/// dangles after suspension.
///
/// This predicate mirrors `try_build_shape_global`'s match arms exactly so the guard
/// fires for precisely the cases that would otherwise silently miscompile.
fn expr_is_compile_time_literal(expr: &Expr) -> bool {
    matches!(expr, Expr::IntLit(_, _) | Expr::BoolLit(_, _))
}

/// Returns `true` if `stmt` (or any sub-expression) reads a field of `target`.
/// Detects `target.key` and `target.value` — any FieldAccess on an Ident matching
/// `target`.
fn stmt_reads_field_of(stmt: &Stmt, target: &str) -> bool {
    match stmt {
        Stmt::Expr(e) => expr_reads_field_of(e, target),
        Stmt::Return { value: Some(e), .. } => expr_reads_field_of(e, target),
        Stmt::Return { value: None, .. } => false,
        Stmt::Let { value, .. } => expr_reads_field_of(value, target),
        Stmt::Assign { value, .. } => expr_reads_field_of(value, target),
        Stmt::If { cond, body, .. } => {
            expr_reads_field_of(cond, target)
                || body.stmts.iter().any(|s| stmt_reads_field_of(s, target))
        }
        Stmt::While { cond, body, .. } => {
            expr_reads_field_of(cond, target)
                || body.stmts.iter().any(|s| stmt_reads_field_of(s, target))
        }
        Stmt::For { body, .. } => body.stmts.iter().any(|s| stmt_reads_field_of(s, target)),
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            expr_reads_field_of(scrutinee, target)
                || arms
                    .iter()
                    .any(|a| a.body.stmts.iter().any(|s| stmt_reads_field_of(s, target)))
                || else_arm
                    .as_ref()
                    .is_some_and(|eb| eb.stmts.iter().any(|s| stmt_reads_field_of(s, target)))
        }
        _ => false,
    }
}

/// Returns `true` if `expr` contains a `target.field` field-access at any depth.
fn expr_reads_field_of(expr: &Expr, target: &str) -> bool {
    match expr {
        Expr::FieldAccess { receiver, .. } => {
            // Direct: `entry.key` — receiver is an Ident matching target.
            if let Expr::Ident(name, _) = receiver.as_ref() {
                if name == target {
                    return true;
                }
            }
            expr_reads_field_of(receiver, target)
        }
        Expr::Wait(inner, _) => expr_reads_field_of(inner, target),
        Expr::PostfixOp { receiver, .. } => expr_reads_field_of(receiver, target),
        Expr::BinOp { lhs, rhs, .. } => {
            expr_reads_field_of(lhs, target) || expr_reads_field_of(rhs, target)
        }
        Expr::UnaryOp { operand, .. } => expr_reads_field_of(operand, target),
        Expr::Call(c) => {
            expr_reads_field_of(&c.callee, target)
                || c.args.iter().any(|a| expr_reads_field_of(a, target))
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_reads_field_of(receiver, target)
                || args.iter().any(|a| expr_reads_field_of(a, target))
        }
        Expr::IndexAccess {
            receiver, index, ..
        } => expr_reads_field_of(receiver, target) || expr_reads_field_of(index, target),
        Expr::StructLit { fields, .. } => {
            fields.iter().any(|f| expr_reads_field_of(&f.value, target))
        }
        Expr::ArrayLit { elements, .. } => elements.iter().any(|e| expr_reads_field_of(e, target)),
        Expr::MapLit { entries, .. } => entries
            .iter()
            .any(|(k, v)| expr_reads_field_of(k, target) || expr_reads_field_of(v, target)),
        Expr::InterpolatedString(parts, _) => parts.iter().any(|p| {
            if let ynz_ast::nodes::StringPart::Expr(e, _) = p {
                expr_reads_field_of(e, target)
            } else {
                false
            }
        }),
        _ => false,
    }
}

/// Scan `stmts` for local (`let`/`const`) bindings declared before any reachable
/// suspension point that are then referenced after it.
///
/// Suspension points include both explicit `wait` AST nodes AND inferred-suspension
/// calls — bare calls to functions whose `suspends` flag is set by the may-block
/// fixpoint (or to M2 may-block intrinsics like `sleep`). Both forms compile to
/// state-machine resume steps in M2 codegen; without a frame slot the local's value
/// is undefined after the step, producing an LLVM SSA dominance failure.
///
/// Concrete suspend-point forms detected:
/// - `Stmt::Expr(Expr::Wait(...))` — top-level bare explicit wait
/// - `Stmt::Let { value: Expr::Wait(...), .. }` — top-level let with explicit wait
/// - `Stmt::Expr(Expr::Call(c))` where `is_suspending_call(c, suspending)` is true
/// - `Stmt::Let { value: Expr::Call(c), .. }` where `is_suspending_call(c, …)` is true
/// - An `if` branch whose body contains any of the above forms
///
/// `wait`-in-`for`/`while`/`match` are all now supported (P2 lifted `while`, P3 lifts
/// `for`/`match`). Loops are handled by recursing into the loop body during analysis;
/// back-edge reads of outer locals are caught by the per-type scan in
/// `collect_crossings_in_stmts`.
///
/// Function parameters are excluded: the SM codegen gives every parameter a frame
/// slot and reloads it at each resume point, so they are always safe.
///
/// Time: O(N) where N = AST nodes scanned  Space: O(C) where C = crossing locals collected
pub fn locals_crossing_wait(
    stmts: &[Stmt],
    param_names: &[&str],
    suspending: &std::collections::HashSet<&str>,
    cpu_supported: &std::collections::HashSet<&str>,
    expr_types: &HashMap<(usize, usize), Type>,
) -> Vec<LocalCrossesWait> {
    let mut problems = Vec::new();
    collect_crossings_in_stmts(
        stmts,
        param_names,
        suspending,
        cpu_supported,
        expr_types,
        &mut Vec::new(),
        &mut problems,
    );
    problems
}

/// Whether `stmt` binds a non-suspending CPU-result-ABI call — the statement shape that
/// can be one member of an M3d CPU spike group.
///
/// A spike group member is `let x = callee(...)` (or a bare `callee(...)`) where `callee`
/// is NOT in `suspending` (it is pure CPU, not a state machine) and IS in `cpu_supported`
/// (its return class fits the join's 16-byte result ABI). Mirrors the codegen eligibility
/// filter in `spike_pair_in_block` so the two sides recognize the same members.
///
/// Time: O(1)  Space: O(1)
fn stmt_is_cpu_spike_member(
    stmt: &Stmt,
    suspending: &std::collections::HashSet<&str>,
    cpu_supported: &std::collections::HashSet<&str>,
) -> bool {
    let c = match stmt {
        Stmt::Let {
            value: Expr::Call(c),
            ..
        }
        | Stmt::Expr(Expr::Call(c)) => c,
        _ => return false,
    };
    match call_name(c) {
        Some(name) => !suspending.contains(name.as_str()) && cpu_supported.contains(name.as_str()),
        None => false,
    }
}

/// Index of the first member of an adjacent CPU spike pair in `stmts`, or `None`.
///
/// The M3d codegen spike-hosts the FIRST adjacent pair of eligible CPU members in a block
/// (`spike_pair_in_block`); the join after that pair is the block's suspension point. Returns
/// the index of the first member so the crossing analysis can mark every prior local as
/// crossing and treat the two member binds as result-bindings (safe across their own join).
///
/// Time: O(n) where n = stmts length  Space: O(1).
fn cpu_spike_pair_first_index(
    stmts: &[Stmt],
    suspending: &std::collections::HashSet<&str>,
    cpu_supported: &std::collections::HashSet<&str>,
) -> Option<usize> {
    if cpu_supported.is_empty() {
        return None;
    }
    stmts.windows(2).position(|w| {
        stmt_is_cpu_spike_member(&w[0], suspending, cpu_supported)
            && stmt_is_cpu_spike_member(&w[1], suspending, cpu_supported)
    })
}

/// Whether `block` contains an adjacent CPU spike pair (at its top level, or inside a nested
/// `if` body), making the block a suspension point for outer crossing analysis.
///
/// Parallels [`block_contains_inferred_suspension`] for the CPU-join suspension class. Loop
/// and match bodies are scanned by the caller's own recursion in `collect_crossings_in_stmts`;
/// this helper covers the `if`-body shape the inferred-suspension predicate also handles.
///
/// Time: O(N) where N = AST nodes in `block` (recurses through nested `if` bodies)  Space: O(D)
/// recursion depth
fn block_contains_cpu_spike_pair(
    block: &Block,
    suspending: &std::collections::HashSet<&str>,
    cpu_supported: &std::collections::HashSet<&str>,
) -> bool {
    if cpu_supported.is_empty() {
        return false;
    }
    if cpu_spike_pair_first_index(&block.stmts, suspending, cpu_supported).is_some() {
        return true;
    }
    block.stmts.iter().any(|stmt| match stmt {
        Stmt::If { body, .. } => block_contains_cpu_spike_pair(body, suspending, cpu_supported),
        _ => false,
    })
}

/// Whether `block` suspends through any M3d-recognized boundary: an explicit `wait`, an
/// inferred-suspension call, OR a CPU spike-group join.
///
/// Time: O(N) where N = AST nodes in `block`  Space: O(D) recursion depth
fn block_suspends_m3d(
    block: &Block,
    suspending: &std::collections::HashSet<&str>,
    cpu_supported: &std::collections::HashSet<&str>,
    expr_types: &HashMap<(usize, usize), Type>,
) -> bool {
    block_contains_wait(block)
        || block_contains_inferred_suspension(block, suspending, expr_types)
        || block_contains_cpu_spike_pair(block, suspending, cpu_supported)
}

/// Recursive crossing-analysis kernel.
///
/// `declared` accumulates all local names declared before any suspension point seen
/// so far in this statement sequence. When a suspension point (explicit `wait` node
/// OR an inferred-suspension call) is encountered, subsequent statements are scanned
/// for references to those accumulated names.
///
/// Time: O(N) where N = AST nodes scanned  Space: O(D) recursion depth + O(L) declared locals
#[allow(clippy::too_many_arguments)]
fn collect_crossings_in_stmts(
    stmts: &[Stmt],
    param_names: &[&str],
    suspending: &std::collections::HashSet<&str>,
    cpu_supported: &std::collections::HashSet<&str>,
    expr_types: &HashMap<(usize, usize), Type>,
    declared: &mut Vec<String>,
    out: &mut Vec<LocalCrossesWait>,
) {
    // Index of the first member of an adjacent CPU spike pair in THIS statement list, if any.
    // The join after that pair is a suspension point: locals declared before it cross it, and
    // the two member binds are result-bindings safe across their own join. Empty `cpu_supported`
    // (every non-spike-host caller) makes this `None`, preserving the original behavior.
    let spike_first_idx = cpu_spike_pair_first_index(stmts, suspending, cpu_supported);
    // Whether a reachable suspension point has been seen in this statement list.
    let mut past_wait = false;
    // Result-binding names from the most-recent suspension step, not yet flushed
    // into `declared`. A result-binding is safe across its OWN producing suspension
    // (the state machine stores it in the frame and resumes with it), but becomes
    // a crossing candidate for any LATER suspension. We defer adding it to
    // `declared` until the next suspension so that reads between the producing
    // suspension and the next one are not falsely flagged.
    let mut pending_result_bindings: Vec<String> = Vec::new();

    for (idx, stmt) in stmts.iter().enumerate() {
        // CPU spike-group join: the first member of the adjacent pair marks the suspension.
        // Before the pair, mark `past_wait` so every prior `declared` local is checked against
        // post-join reads (the accumulator bug). The two member binds are deferred into
        // `pending_result_bindings` (safe across their own join, crossing candidates only for a
        // LATER suspension), and the second member is consumed here so it is not re-processed as
        // a plain `let` that would push it into `declared` prematurely.
        if !past_wait && spike_first_idx == Some(idx) {
            past_wait = true;
            for member_idx in [idx, idx + 1] {
                if let Stmt::Let { name, .. } = &stmts[member_idx] {
                    if !param_names.contains(&name.as_str())
                        && !pending_result_bindings.contains(name)
                    {
                        pending_result_bindings.push(name.clone());
                    }
                }
            }
            continue;
        }
        // Consume the second member of the spike pair (already handled above).
        if spike_first_idx == Some(idx.wrapping_sub(1)) {
            continue;
        }
        if past_wait {
            // Before checking references, flush any result-binding names from the
            // PREVIOUS suspension that were deferred. At this point we are inside a
            // body that already has `past_wait = true`, which means there could be
            // another suspension ahead — so these names are now real crossing
            // candidates if read after that next suspension.
            //
            // We check for a NEW suspension first; if this statement IS a suspension,
            // flush before scanning so the binding isn't falsely flagged for being
            // read by its own producing step.
            //
            // ROOT-CAUSE FIX (v0.3-M3g): a control-flow statement (`if`/`while`/`for`/`match`)
            // whose BODY suspends is ITSELF a reachable suspension point for THIS statement
            // sequence — exactly like a direct top-level `wait`/suspending-call statement. The
            // three extra arms below were MISSING, so a group's pending result-binding names
            // (or any earlier-suspension's) never flushed into `declared` before such a nested
            // suspension, and a read after it (e.g. `print(a)` following
            // `if (flag > 0) { wait sleep(0) }`) was silently never marked crossing — codegen
            // then emitted an alloca that does not dominate the read (LLVM SSA verification
            // failure, "Instruction does not dominate all uses"). Reproduced and root-caused via
            // a minimized fixture (a top-level 2-member CPU group followed by
            // `if (flag > 0) { wait sleep(0) }` then reads of both group members) and the
            // pre-existing corpus fixture `v0_3_m3d_spike_n_nested_wait.ynz`. `admitted_cpu_group`
            // used to carry a narrow residual decline for this shape
            // (`stmt_control_flow_body_suspends`); that decline is removed now that the real
            // crossing-analysis gap is fixed here directly.
            let this_stmt_suspends = match stmt {
                Stmt::Expr(Expr::Wait(_, _)) => true,
                Stmt::Expr(Expr::Call(c)) if is_suspending_call(c, suspending) => true,
                // v0.3-M4: conduit-method suspension statements.
                s if stmt_is_conduit_suspend(s, expr_types) => true,
                Stmt::Let {
                    value: Expr::Wait(_, _),
                    ..
                } => true,
                Stmt::Let {
                    value: Expr::Call(c),
                    ..
                } if is_suspending_call(c, suspending) => true,
                Stmt::If { body, .. }
                    if block_suspends_m3d(body, suspending, cpu_supported, expr_types) =>
                {
                    true
                }
                Stmt::While { body, .. } | Stmt::For { body, .. }
                    if block_suspends_m3d(body, suspending, cpu_supported, expr_types) =>
                {
                    true
                }
                Stmt::Match { arms, else_arm, .. }
                    if arms.iter().any(|a| {
                        block_suspends_m3d(&a.body, suspending, cpu_supported, expr_types)
                    }) || else_arm.as_ref().is_some_and(|eb| {
                        block_suspends_m3d(eb, suspending, cpu_supported, expr_types)
                    }) =>
                {
                    true
                }
                _ => false,
            };
            if this_stmt_suspends {
                // Flush pending result-bindings from the prior suspension into
                // `declared` so they are live for any suspension AFTER this one.
                for name in pending_result_bindings.drain(..) {
                    if !declared.contains(&name) {
                        declared.push(name);
                    }
                }
                match stmt {
                    // Collect the new result-binding (if any) into pending.
                    // The MethodCall arm covers v0.3-M4 conduit-suspend bindings
                    // (`let x = ch.receive()`) — reachable here only when
                    // `this_stmt_suspends` already classified the statement.
                    Stmt::Let {
                        name,
                        value: Expr::Wait(_, _),
                        ..
                    }
                    | Stmt::Let {
                        name,
                        value: Expr::Call(_),
                        ..
                    }
                    | Stmt::Let {
                        name,
                        value: Expr::MethodCall { .. },
                        ..
                    } if !param_names.contains(&name.as_str())
                        && !pending_result_bindings.contains(name) =>
                    {
                        pending_result_bindings.push(name.clone());
                    }
                    // A control-flow statement whose body suspends: scan the condition for
                    // references to already-`declared` (pre-suspension) locals FIRST — unlike
                    // the not-yet-suspended top-level `Stmt::If` handling below (which
                    // deliberately skips this scan because NOTHING has suspended yet at that
                    // point, so no read there could possibly be crossing), we are HERE only
                    // because an EARLIER suspension already happened in this sequence, so the
                    // condition genuinely runs strictly after that prior suspension and a read
                    // of a pre-suspension local in it IS a real crossing (caught by a real
                    // regression this session: `v0_3_m3a_p1_disjoint_sibling_scope_shadow.ynz`'s
                    // second `if (flag2)`, where `flag2` is declared before the FIRST if's wait
                    // and read only in the SECOND if's condition — the codegen-emitted alloca
                    // for `flag2` correctly disappeared from the frame and the second if-arm's
                    // print silently never ran until this scan was added back). Mirrors the
                    // pre-existing "else" (not-yet-suspended) branch, which unconditionally
                    // called `collect_ident_refs_in_stmt` on every statement type BEFORE the
                    // now-removed dead-code nested-suspension recursion. Then recurse into the
                    // suspending sub-block with the now-FLUSHED `declared` (sub-case (b) — a
                    // local declared INSIDE the branch, before the branch's OWN inner
                    // suspension, still needs its own crossing detection).
                    Stmt::If { body, .. } => {
                        collect_ident_refs_in_stmt(stmt, declared, out);
                        let mut branch_declared = declared.clone();
                        collect_crossings_in_stmts(
                            &body.stmts,
                            param_names,
                            suspending,
                            cpu_supported,
                            expr_types,
                            &mut branch_declared,
                            out,
                        );
                    }
                    Stmt::While { body, .. } | Stmt::For { body, .. } => {
                        collect_ident_refs_in_stmt(stmt, declared, out);
                        let mut branch_declared = declared.clone();
                        collect_crossings_in_stmts(
                            &body.stmts,
                            param_names,
                            suspending,
                            cpu_supported,
                            expr_types,
                            &mut branch_declared,
                            out,
                        );
                    }
                    Stmt::Match { arms, else_arm, .. } => {
                        collect_ident_refs_in_stmt(stmt, declared, out);
                        for arm in arms {
                            if block_suspends_m3d(&arm.body, suspending, cpu_supported, expr_types)
                            {
                                let mut branch_declared = declared.clone();
                                collect_crossings_in_stmts(
                                    &arm.body.stmts,
                                    param_names,
                                    suspending,
                                    cpu_supported,
                                    expr_types,
                                    &mut branch_declared,
                                    out,
                                );
                            }
                        }
                        if let Some(eb) = else_arm {
                            if block_suspends_m3d(eb, suspending, cpu_supported, expr_types) {
                                let mut branch_declared = declared.clone();
                                collect_crossings_in_stmts(
                                    &eb.stmts,
                                    param_names,
                                    suspending,
                                    cpu_supported,
                                    expr_types,
                                    &mut branch_declared,
                                    out,
                                );
                            }
                        }
                    }
                    _ => {}
                }
            } else {
                // Non-suspension statement after a prior suspension: scan for references to
                // already-declared (pre-suspension) locals. Pending result-bindings are NOT
                // yet in `declared`, so reads of the just-produced binding are not flagged.
                collect_ident_refs_in_stmt(stmt, declared, out);
                // A new `let` binding introduced BETWEEN two suspension points is itself
                // a crossing candidate for any suspension that follows it. Add it to
                // `declared` so the next suspension will catch any reads after it.
                if let Stmt::Let { name, .. } = stmt {
                    if !declared.contains(name) && !param_names.contains(&name.as_str()) {
                        declared.push(name.clone());
                    }
                }
                // NOTE: a control-flow statement (`if`/`while`/`for`/`match`) only reaches
                // this branch when its body does NOT suspend (`block_suspends_m3d` is false)
                // — every suspending-body case is now caught by `this_stmt_suspends` above,
                // so no nested-suspension recursion is needed here (see that arm's doc
                // comment for the root-cause history).
            }
        } else {
            match stmt {
                // Explicit bare top-level wait — all currently-declared locals cross.
                Stmt::Expr(Expr::Wait(_, _)) => {
                    past_wait = true;
                }
                // Inferred-suspension bare call (no `wait` keyword) at statement position.
                // Under M2 inference this lowers to the same state-machine step as an
                // explicit `wait`, so locals declared before it must not be read after.
                Stmt::Expr(Expr::Call(c)) if is_suspending_call(c, suspending) => {
                    past_wait = true;
                }
                // v0.3-M4: bare conduit-method suspension (`ch.send(v)` / `ch.receive()`).
                Stmt::Expr(e) if expr_is_conduit_suspend(e, expr_types) => {
                    past_wait = true;
                }
                // v0.3-M4: conduit-suspend result binding (`let x = ch.receive()`) — safe
                // across its OWN producing suspension; a crossing candidate for later ones.
                Stmt::Let { name, value, .. } if expr_is_conduit_suspend(value, expr_types) => {
                    past_wait = true;
                    if !declared.contains(name)
                        && !param_names.contains(&name.as_str())
                        && !pending_result_bindings.contains(name)
                    {
                        pending_result_bindings.push(name.clone());
                    }
                }
                // `let name = wait expr` — name is safe across its OWN producing suspension
                // (the state machine writes it to the frame, resumes with it available), but
                // is a crossing candidate for every LATER suspension. Defer tracking to
                // `pending_result_bindings`; it flushes into `declared` when the next
                // suspension is encountered, making it catchable only then.
                Stmt::Let {
                    name,
                    value: Expr::Wait(_, _),
                    ..
                } => {
                    past_wait = true;
                    if !declared.contains(name)
                        && !param_names.contains(&name.as_str())
                        && !pending_result_bindings.contains(name)
                    {
                        pending_result_bindings.push(name.clone());
                    }
                }
                // `let name = suspending_call()` — same semantics as the explicit-wait form.
                // Safe across its OWN producing suspension; a crossing candidate only for
                // subsequent suspensions. Defer into `pending_result_bindings` for the same
                // reason as the `wait` arm above.
                Stmt::Let {
                    name,
                    value: Expr::Call(c),
                    ..
                } if is_suspending_call(c, suspending) => {
                    past_wait = true;
                    if !declared.contains(name)
                        && !param_names.contains(&name.as_str())
                        && !pending_result_bindings.contains(name)
                    {
                        pending_result_bindings.push(name.clone());
                    }
                }
                Stmt::Let { name, value, .. } => {
                    if !declared.contains(name) && !param_names.contains(&name.as_str()) {
                        declared.push(name.clone());
                    }
                    // The RHS could contain an `if`-nested wait in theory (rare), recurse.
                    // In practice the parser rejects `wait` used as an expression nested
                    // under `let`; this is a conservative safety net.
                    if expr_contains_wait_anywhere(value) {
                        past_wait = true;
                    }
                }
                Stmt::Assign { target: name, .. }
                    if !declared.contains(name) && !param_names.contains(&name.as_str()) =>
                {
                    declared.push(name.clone());
                }
                // An `if` block that contains a suspension point (explicit or inferred)
                // is itself a reachable suspension point for the outer statement sequence.
                // Two sub-cases:
                //   (a) Locals declared BEFORE this `if` and read AFTER it cross the
                //       potential suspension inside the branch.
                //   (b) Locals declared INSIDE the branch, before the suspension, and
                //       read after it IN THE SAME BRANCH — handled via recursive call.
                Stmt::If { body, .. }
                    if block_suspends_m3d(body, suspending, cpu_supported, expr_types) =>
                {
                    // Sub-case (b): recurse into the branch, seeding with the outer
                    // `declared` set so outer-scope names are also tracked inside.
                    let mut branch_declared = declared.clone();
                    collect_crossings_in_stmts(
                        &body.stmts,
                        param_names,
                        suspending,
                        cpu_supported,
                        expr_types,
                        &mut branch_declared,
                        out,
                    );
                    // Sub-case (a): mark suspension seen for the outer sequence so
                    // post-if statements are checked against pre-if declared locals.
                    past_wait = true;
                }
                // A `while` body that contains a suspension is also a reachable suspension
                // point from the outer sequence — the same two sub-cases apply:
                //   (a) Locals declared BEFORE the while and read AFTER it are crossing
                //       locals (the while loop may suspend and resume between them).
                //   (b) Locals declared INSIDE the body, before the inner suspension, and
                //       read after it WITHIN THE SAME BODY — found by recursive call.
                //
                // Back-edge crossing: any outer-declared local referenced in the condition
                // OR anywhere in the body must be treated as a crossing local immediately,
                // even if textually the write/read precedes the `wait` inside the body.
                // On every iteration after the first, the condition re-reads the local
                // AFTER the prior iteration's suspension has completed — so the value must
                // survive each `wait` via the frame slot. A purely forward textual scan
                // misses this because the write and the condition-read both appear before
                // the `wait` in textual order, yet execution cycles back through them.
                Stmt::While { body, .. }
                    if block_suspends_m3d(body, suspending, cpu_supported, expr_types) =>
                {
                    // Scan the condition and body for reads of outer-declared locals.
                    // This catches the back-edge case: counter/accumulator locals are
                    // read by the condition on each iteration, which comes AFTER the
                    // suspension from the previous iteration's `wait`.
                    collect_ident_refs_in_stmt(stmt, declared, out);
                    let mut branch_declared = declared.clone();
                    collect_crossings_in_stmts(
                        &body.stmts,
                        param_names,
                        suspending,
                        cpu_supported,
                        expr_types,
                        &mut branch_declared,
                        out,
                    );
                    past_wait = true;
                }
                // A `for` body that contains a suspension: same two sub-cases as `while`.
                // The iterator expression (`for (x in iter)`) is re-evaluated structurally
                // on each iteration but iter itself is not a back-edge read in the same sense
                // (the collection pointer/count is stable). Outer locals READ inside the body
                // are still crossing locals — a forward scan seeded with the outer `declared`
                // set catches them. Mark past_wait so post-for statements are scanned.
                Stmt::For { body, iter, .. }
                    if block_suspends_m3d(body, suspending, cpu_supported, expr_types) =>
                {
                    // Scan the iter expression and body for reads of outer-declared locals.
                    // The iter expression may reference an outer local (e.g., the collection
                    // variable itself) — treat it as a back-edge read like the while condition.
                    collect_ident_refs_in_stmt(stmt, declared, out);
                    let _ = iter; // already scanned via collect_ident_refs_in_stmt above
                    let mut branch_declared = declared.clone();
                    collect_crossings_in_stmts(
                        &body.stmts,
                        param_names,
                        suspending,
                        cpu_supported,
                        expr_types,
                        &mut branch_declared,
                        out,
                    );
                    past_wait = true;
                }
                // A `match` arm containing a suspension: each arm with a wait is its own
                // sub-case. Outer locals read in the scrutinee or in any arm body are crossing.
                Stmt::Match {
                    arms,
                    else_arm,
                    scrutinee,
                    ..
                } if arms.iter().any(|a| {
                    block_suspends_m3d(&a.body, suspending, cpu_supported, expr_types)
                }) || else_arm.as_ref().is_some_and(|eb| {
                    block_suspends_m3d(eb, suspending, cpu_supported, expr_types)
                }) =>
                {
                    // Scrutinee may reference outer-declared locals.
                    let _ = scrutinee; // scanned via collect_ident_refs_in_stmt
                    collect_ident_refs_in_stmt(stmt, declared, out);
                    for arm in arms {
                        if block_suspends_m3d(&arm.body, suspending, cpu_supported, expr_types) {
                            let mut arm_declared = declared.clone();
                            collect_crossings_in_stmts(
                                &arm.body.stmts,
                                param_names,
                                suspending,
                                cpu_supported,
                                expr_types,
                                &mut arm_declared,
                                out,
                            );
                        }
                    }
                    if let Some(eb) = else_arm {
                        if block_suspends_m3d(eb, suspending, cpu_supported, expr_types) {
                            let mut eb_declared = declared.clone();
                            collect_crossings_in_stmts(
                                &eb.stmts,
                                param_names,
                                suspending,
                                cpu_supported,
                                expr_types,
                                &mut eb_declared,
                                out,
                            );
                        }
                    }
                    past_wait = true;
                }
                _ => {}
            }
        }
    }
}

/// Recursively scan `stmt` and accumulate `LocalCrossesWait` entries for any
/// `Expr::Ident` whose name is in `targets`.
fn collect_ident_refs_in_stmt(stmt: &Stmt, targets: &[String], out: &mut Vec<LocalCrossesWait>) {
    match stmt {
        Stmt::Expr(e) => collect_ident_refs_in_expr(e, targets, out),
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            collect_ident_refs_in_expr(value, targets, out);
        }
        Stmt::If { cond, body, .. } => {
            collect_ident_refs_in_expr(cond, targets, out);
            for s in &body.stmts {
                collect_ident_refs_in_stmt(s, targets, out);
            }
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            collect_ident_refs_in_expr(scrutinee, targets, out);
            for arm in arms {
                for s in &arm.body.stmts {
                    collect_ident_refs_in_stmt(s, targets, out);
                }
            }
            if let Some(b) = else_arm {
                for s in &b.stmts {
                    collect_ident_refs_in_stmt(s, targets, out);
                }
            }
        }
        Stmt::While { cond, body, .. }
        | Stmt::For {
            iter: cond, body, ..
        } => {
            collect_ident_refs_in_expr(cond, targets, out);
            for s in &body.stmts {
                collect_ident_refs_in_stmt(s, targets, out);
            }
        }
        Stmt::Return { value, .. } => {
            if let Some(e) = value {
                collect_ident_refs_in_expr(e, targets, out);
            }
        }
        Stmt::FieldAssign { target, value, .. } => {
            collect_ident_refs_in_expr(target, targets, out);
            collect_ident_refs_in_expr(value, targets, out);
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            collect_ident_refs_in_expr(receiver, targets, out);
            collect_ident_refs_in_expr(index, targets, out);
            collect_ident_refs_in_expr(value, targets, out);
        }
    }
}

fn collect_ident_refs_in_expr(expr: &Expr, targets: &[String], out: &mut Vec<LocalCrossesWait>) {
    match expr {
        Expr::Ident(name, span) => {
            if targets.contains(name) {
                out.push(LocalCrossesWait {
                    name: name.clone(),
                    use_span: span.clone(),
                });
            }
        }
        Expr::Wait(inner, _) | Expr::Background(inner, _) => {
            collect_ident_refs_in_expr(inner, targets, out);
        }
        Expr::Call(c) => {
            collect_ident_refs_in_expr(&c.callee, targets, out);
            for a in &c.args {
                collect_ident_refs_in_expr(a, targets, out);
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_ident_refs_in_expr(lhs, targets, out);
            collect_ident_refs_in_expr(rhs, targets, out);
        }
        Expr::UnaryOp { operand, .. } => collect_ident_refs_in_expr(operand, targets, out),
        Expr::MethodCall { receiver, args, .. } => {
            collect_ident_refs_in_expr(receiver, targets, out);
            for a in args {
                collect_ident_refs_in_expr(a, targets, out);
            }
        }
        Expr::FieldAccess { receiver, .. } => collect_ident_refs_in_expr(receiver, targets, out),
        Expr::IndexAccess {
            receiver, index, ..
        } => {
            collect_ident_refs_in_expr(receiver, targets, out);
            collect_ident_refs_in_expr(index, targets, out);
        }
        Expr::StructLit { fields, .. } => {
            for f in fields {
                collect_ident_refs_in_expr(&f.value, targets, out);
            }
        }
        Expr::ArrayLit { elements, .. } => {
            for e in elements {
                collect_ident_refs_in_expr(e, targets, out);
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_ident_refs_in_expr(k, targets, out);
                collect_ident_refs_in_expr(v, targets, out);
            }
        }
        Expr::PostfixOp { receiver, .. } => collect_ident_refs_in_expr(receiver, targets, out),
        Expr::Is { expr: inner, .. } => collect_ident_refs_in_expr(inner, targets, out),
        Expr::InterpolatedString(parts, _) => {
            for p in parts {
                if let ynz_ast::nodes::StringPart::Expr(e, _) = p {
                    collect_ident_refs_in_expr(e, targets, out);
                }
            }
        }
        // Leaf nodes — no identifiers to find other than Ident handled above.
        Expr::StringLit(_, _)
        | Expr::IntLit(_, _)
        | Expr::NumberLit(_, _)
        | Expr::BoolLit(_, _)
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. }
        | Expr::Error(_) => {}
    }
}

// ── Check 3: suspending call in a sub-expression position ────────────────────
//
// v0.3-M2 codegen handles suspending calls only at the direct-statement level:
//   - `foo()`            (Stmt::Expr — bare call)
//   - `let x = foo()`   (Stmt::Let with call as entire RHS)
//   - `return foo()`    (Stmt::Return with call as the return value)
//   - the `wait`-wrapped forms of each of the above
//
// Any suspending call nested deeper in an expression is NOT handled: codegen
// falls through to the wrapper path (block_on) which panics on Tokio worker
// threads. Catching this at typeck emits a clean teaching error before the
// panic ever fires.

/// Describes a suspending call found in a sub-expression position.
struct SubExprSuspendViolation {
    /// Source span of the nested call.
    span: SourceSpan,
    /// The callee name (for the error message).
    callee_name: String,
}

/// Walk `stmts` and collect all suspending calls that appear in sub-expression
/// positions (not the direct-statement forms the M2 codegen handles).
///
/// `suspending` is the set of user-defined function names whose `suspends == true`
/// in the current compilation unit. Base suspension intrinsics (`sleep`,
/// `__testFallibleAsync`) are included via `is_base_suspension_intrinsic`.
pub(crate) fn suspending_calls_in_subexpr_position(
    stmts: &[Stmt],
    suspending: &std::collections::HashSet<&str>,
) -> Vec<(SourceSpan, String)> {
    let mut out = Vec::new();
    collect_subexpr_violations_in_stmts(stmts, suspending, &mut out);
    out.into_iter().map(|v| (v.span, v.callee_name)).collect()
}

/// Probe whether ANY state-machine suspension guard would fire on `f` if it were a
/// state machine, under the effective suspending-function set `suspending_fns`.
///
/// This is the v0.3-M3d **decline-to-promote** predicate (Decision Record item 8c):
/// the CPU-promotion query marks a candidate's whole transitive-SM closure as
/// suspending, then calls this on every newly-SM function. A `true` verdict means
/// promoting that function would turn previously-compiling code into a guard error,
/// so the promotion is rolled back (declines to sequential lowering — never a new
/// compile error).
///
/// It REUSES the exact guard predicates the diagnostic-emitting `check_function`
/// path runs (`suspending_calls_in_subexpr_position`, `crossing_local_names`,
/// `find_stored_range_wait_in_for`, `find_fixed_array_iter_wait_in_for`,
/// `find_expr_iter_wait_in_for`, `find_map_entry_field_after_wait`,
/// `find_array_shape_runtime_field_crossing`, the wide-return classifier, the
/// nested-shape / unsupported-crossing-type / shadow checks). The gating mirrors
/// `check_function` exactly: the function is treated as `is_suspending_fn = true`,
/// and `has_explicit_waits` is read from its body. No guard logic is re-derived —
/// only the boolean verdict is consumed here instead of a formatted diagnostic.
///
/// `expr_types` MUST be the map from the baseline `check` pass (the guards read
/// resolved crossing-local types from it). `kernel_mode` short-circuits to `false`
/// because every guard above is `!kernel_mode`-gated in `check_function` (kernel
/// mode never promotes, so the probe is moot there, but the parameter keeps the
/// contract explicit).
///
/// Time: O(stmts · crossings)  Space: O(crossings)
pub(crate) fn suspension_guards_fire_for_fn(
    f: &ynz_ast::nodes::FunctionDecl,
    ret_ty: &Type,
    suspending_fns: &std::collections::HashSet<&str>,
    shape_table: &ShapeTable,
    union_aliases: &HashMap<String, Type>,
    expr_types: &HashMap<(usize, usize), Type>,
    kernel_mode: bool,
) -> bool {
    if kernel_mode {
        return false;
    }
    let has_explicit_waits = block_contains_wait(&f.body);

    // WideValueSuspendingReturn: a suspending fn returning a bare shape or `Shape errors`.
    let is_wide_return = match ret_ty {
        Type::Shape { .. } => true,
        Type::ErrorsCapable { inner } => matches!(inner.as_ref(), Type::Shape { .. }),
        _ => false,
    };
    if is_wide_return {
        return true;
    }

    // Suspending call in a sub-expression position.
    if !suspending_calls_in_subexpr_position(&f.body.stmts, suspending_fns).is_empty() {
        return true;
    }

    // StoredRangeWithWait / FixedArrayIterWithWait / ExpressionIterWithWait — explicit-wait gated.
    if has_explicit_waits {
        if find_stored_range_wait_in_for(&f.body.stmts, expr_types).is_some() {
            return true;
        }
        if find_fixed_array_iter_wait_in_for(&f.body.stmts, expr_types).is_some() {
            return true;
        }
        if find_expr_iter_wait_in_for(&f.body.stmts).is_some() {
            return true;
        }
    }

    // Crossing-local guards (nested-shape, UnsupportedCrossingLocalType, shadow).
    let param_names_ref: Vec<&str> = f.params.iter().map(|p| p.name.as_str()).collect();
    let crossings =
        crossing_local_names(&f.body.stmts, &param_names_ref, suspending_fns, expr_types);

    for crossing_name in &crossings {
        // Nested-shape crossing.
        let resolved_ty = find_crossing_local_typeck_type_in_map(
            &f.body.stmts,
            crossing_name.as_str(),
            expr_types,
        )
        .or_else(|| {
            find_for_loop_var_type_in_stmts(&f.body.stmts, crossing_name.as_str(), expr_types)
        });
        if let Some(Type::Shape { name: shape_name }) = &resolved_ty {
            let has_nested_shape = shape_table
                .shapes
                .get(shape_name.as_str())
                .is_some_and(|def| {
                    def.fields
                        .iter()
                        .any(|field| matches!(&field.ty, Type::Shape { .. }))
                });
            if has_nested_shape {
                return true;
            }
        }

        // UnsupportedCrossingLocalType: union/maybe/dynamic/fixed/range crossing.
        let rhs_ty = find_crossing_local_typeck_type_in_map(
            &f.body.stmts,
            crossing_name.as_str(),
            expr_types,
        );
        let ann_ty = find_let_annotation_type_in_stmts(&f.body.stmts, crossing_name.as_str())
            .and_then(|ast_ty| resolve_type_for_guard_free(&ast_ty, union_aliases));
        let for_var_ty =
            find_for_loop_var_type_in_stmts(&f.body.stmts, crossing_name.as_str(), expr_types);
        let unsupported = [&ann_ty, &rhs_ty, &for_var_ty].iter().any(|opt| {
            opt.as_ref().is_some_and(|ty| {
                matches!(
                    ty,
                    Type::Union { .. }
                        | Type::Maybe { .. }
                        | Type::Dynamic { .. }
                        | Type::BuiltinFixed { .. }
                        | Type::Range { .. }
                )
            })
        });
        if unsupported {
            return true;
        }

        // ShadowsCrossingLocal: nested or top-level redeclaration of the crossing name.
        if outer_is_genuine_crossing_local(
            &f.body.stmts,
            crossing_name.as_str(),
            suspending_fns,
            expr_types,
        ) {
            if find_shadow_in_stmts(&f.body.stmts, crossing_name.as_str()) {
                return true;
            }
            if has_top_level_let_after_suspension(
                &f.body.stmts,
                crossing_name.as_str(),
                suspending_fns,
                expr_types,
            ) {
                return true;
            }
        }
    }

    // MapEntryFieldAfterWait.
    if find_map_entry_field_after_wait(&f.body.stmts, expr_types).is_some() {
        return true;
    }

    // ArrayShapeRuntimeFieldWithWait.
    if find_array_shape_runtime_field_crossing(&crossings, &f.body.stmts).is_some() {
        return true;
    }

    // Parameter-shadow guard — mirrors `check_function`'s Check 3b EXACTLY (the probe
    // must decline any host the real checker would later reject once it becomes SM).
    //   Shape (a): a nested `let <param>` in ANY block body fires unconditionally — the
    //     name-keyed frame slot is shared even when the inner binding does not itself
    //     cross a suspension. This is the case the CPU join exposes: a promoted host has
    //     no explicit top-level `wait`, so the suspension-gated shape (b) below would not
    //     run, but the slot collision is real once the host is a state machine.
    //   Shape (b): a top-level `let <param>` redeclaration, gated behind a top-level
    //     suspension (matches `check_function` line ~1049).
    for p in &f.params {
        if param_has_nested_let_shadow(&f.body.stmts, p.name.as_str()) {
            return true;
        }
    }
    if first_top_level_suspension_idx(&f.body.stmts, suspending_fns, expr_types).is_some() {
        for p in &f.params {
            if has_top_level_let_in_stmts(&f.body.stmts, p.name.as_str()) {
                return true;
            }
        }
    }

    false
}

/// Free-function form of `Checker::resolve_type_for_guard` — resolves a crossing
/// local's annotation type for the UnsupportedCrossingLocalType probe. Reuses the
/// same union-alias map the checker uses; no other checker state is consulted.
fn resolve_type_for_guard_free(
    ast_ty: &ynz_ast::nodes::Type,
    union_aliases: &HashMap<String, Type>,
) -> Option<Type> {
    use ynz_ast::nodes::Type as AstType;
    match ast_ty {
        AstType::Named(n, _) if union_aliases.contains_key(n) => Some(union_aliases[n].clone()),
        AstType::Union { variants, .. } if variants.len() >= 2 => Some(Type::Union {
            variants: vec![Type::Int; variants.len()],
        }),
        AstType::Maybe { .. } => Some(Type::Maybe {
            inner: Box::new(Type::Int),
        }),
        AstType::Dynamic { contract, .. } => Some(Type::Dynamic {
            contract: contract.clone(),
        }),
        _ => None,
    }
}

fn collect_subexpr_violations_in_stmts(
    stmts: &[Stmt],
    suspending: &std::collections::HashSet<&str>,
    out: &mut Vec<SubExprSuspendViolation>,
) {
    for stmt in stmts {
        collect_subexpr_violations_in_stmt(stmt, suspending, out);
    }
}

fn collect_subexpr_violations_in_stmt(
    stmt: &Stmt,
    suspending: &std::collections::HashSet<&str>,
    out: &mut Vec<SubExprSuspendViolation>,
) {
    match stmt {
        // Direct-statement call: `foo()` — the whole expression IS the call. Safe.
        Stmt::Expr(Expr::Call(c)) if is_suspending_call(c, suspending) => {
            // Allowed. But check the arguments for nested suspending calls.
            for arg in &c.args {
                collect_subexpr_violations_in_expr(arg, suspending, out);
            }
        }
        // Direct-statement wait-of-call: `wait foo()` — the whole expression is Wait(Call). Safe.
        Stmt::Expr(Expr::Wait(inner, _)) => {
            match inner.as_ref() {
                Expr::Call(c) if is_suspending_call(c, suspending) => {
                    // Allowed. Check args.
                    for arg in &c.args {
                        collect_subexpr_violations_in_expr(arg, suspending, out);
                    }
                }
                // `wait expr` where inner is not a direct call — scan inner for violations.
                other => collect_subexpr_violations_in_expr(other, suspending, out),
            }
        }
        // Non-wait bare expression: scan for nested suspending calls.
        Stmt::Expr(expr) => collect_subexpr_violations_in_expr(expr, suspending, out),

        // `let x = foo()` — the whole RHS IS the call. Safe.
        Stmt::Let {
            value: Expr::Call(c),
            ..
        } if is_suspending_call(c, suspending) => {
            for arg in &c.args {
                collect_subexpr_violations_in_expr(arg, suspending, out);
            }
        }
        // `let x = wait foo()` — Safe.
        Stmt::Let {
            value: Expr::Wait(inner, _),
            ..
        } => match inner.as_ref() {
            Expr::Call(c) if is_suspending_call(c, suspending) => {
                for arg in &c.args {
                    collect_subexpr_violations_in_expr(arg, suspending, out);
                }
            }
            other => collect_subexpr_violations_in_expr(other, suspending, out),
        },
        // `let x = <complex expr>` — scan the RHS.
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            collect_subexpr_violations_in_expr(value, suspending, out);
        }

        // `return foo()` — Safe.
        Stmt::Return {
            value: Some(Expr::Call(c)),
            ..
        } if is_suspending_call(c, suspending) => {
            for arg in &c.args {
                collect_subexpr_violations_in_expr(arg, suspending, out);
            }
        }
        // `return wait foo()` — Safe.
        Stmt::Return {
            value: Some(Expr::Wait(inner, _)),
            ..
        } => match inner.as_ref() {
            Expr::Call(c) if is_suspending_call(c, suspending) => {
                for arg in &c.args {
                    collect_subexpr_violations_in_expr(arg, suspending, out);
                }
            }
            other => collect_subexpr_violations_in_expr(other, suspending, out),
        },
        // `return <complex>` — scan.
        Stmt::Return {
            value: Some(expr), ..
        } => {
            collect_subexpr_violations_in_expr(expr, suspending, out);
        }
        Stmt::Return { value: None, .. } => {}

        // Control flow — recurse into bodies.
        Stmt::If { cond, body, .. } => {
            collect_subexpr_violations_in_expr(cond, suspending, out);
            collect_subexpr_violations_in_stmts(&body.stmts, suspending, out);
        }
        Stmt::While { cond, body, .. }
        | Stmt::For {
            iter: cond, body, ..
        } => {
            collect_subexpr_violations_in_expr(cond, suspending, out);
            collect_subexpr_violations_in_stmts(&body.stmts, suspending, out);
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            collect_subexpr_violations_in_expr(scrutinee, suspending, out);
            for arm in arms {
                collect_subexpr_violations_in_stmts(&arm.body.stmts, suspending, out);
            }
            if let Some(b) = else_arm {
                collect_subexpr_violations_in_stmts(&b.stmts, suspending, out);
            }
        }
        Stmt::FieldAssign { target, value, .. } => {
            collect_subexpr_violations_in_expr(target, suspending, out);
            collect_subexpr_violations_in_expr(value, suspending, out);
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            collect_subexpr_violations_in_expr(receiver, suspending, out);
            collect_subexpr_violations_in_expr(index, suspending, out);
            collect_subexpr_violations_in_expr(value, suspending, out);
        }
    }
}

/// Scan an expression for suspending calls in ANY position (sub-expression).
///
/// Called when we are already inside a "disallowed" expression context — any
/// suspending call found here is a violation.
fn collect_subexpr_violations_in_expr(
    expr: &Expr,
    suspending: &std::collections::HashSet<&str>,
    out: &mut Vec<SubExprSuspendViolation>,
) {
    match expr {
        // A call here is in sub-expression position (the caller already established
        // we're NOT at the direct-statement level).
        Expr::Call(c) => {
            if is_suspending_call(c, suspending) {
                if let Some(name) = call_name(c) {
                    out.push(SubExprSuspendViolation {
                        span: c.span.clone(),
                        callee_name: name,
                    });
                    // Don't recurse into arguments of a reported call — one error per call site.
                    return;
                }
            }
            // Not suspending — but its arguments might be.
            collect_subexpr_violations_in_expr(&c.callee, suspending, out);
            for arg in &c.args {
                collect_subexpr_violations_in_expr(arg, suspending, out);
            }
        }
        // `wait expr` in sub-expression position: the inner is already inside the larger expr.
        Expr::Wait(inner, _) => collect_subexpr_violations_in_expr(inner, suspending, out),
        // `background foo(a, b)`: the spawn target (`foo`) becomes its own state machine
        // and is a call-graph cut for suspension propagation.  BUT the arguments `a` and `b`
        // evaluate in the CALLING context before the spawn — exactly like the non-background
        // `foo(a, b)` form.  A suspending call nested inside an argument therefore runs on the
        // caller's thread in a sub-expression position, triggering the same nested-block_on
        // abort that this guard exists to prevent.
        //
        // Rule: scan the spawned call's arguments for violations; do NOT flag the direct
        // spawn callee itself (that is the legal route-to-I/O-pool pattern).
        Expr::Background(inner, _) => match inner.as_ref() {
            Expr::Call(c) => {
                // Direct-spawn callee is a graph cut — skip `c.callee`. Scan args only.
                for arg in &c.args {
                    collect_subexpr_violations_in_expr(arg, suspending, out);
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                // Same reasoning: receiver + args evaluate in the caller.
                collect_subexpr_violations_in_expr(receiver, suspending, out);
                for arg in args {
                    collect_subexpr_violations_in_expr(arg, suspending, out);
                }
            }
            // Unexpected inner shape (rare — background is statement-position-only in M2).
            // Recurse conservatively to catch any nested violations.
            other => collect_subexpr_violations_in_expr(other, suspending, out),
        },
        Expr::BinOp { lhs, rhs, .. } => {
            collect_subexpr_violations_in_expr(lhs, suspending, out);
            collect_subexpr_violations_in_expr(rhs, suspending, out);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_subexpr_violations_in_expr(operand, suspending, out)
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_subexpr_violations_in_expr(receiver, suspending, out);
            for a in args {
                collect_subexpr_violations_in_expr(a, suspending, out);
            }
        }
        Expr::FieldAccess { receiver, .. } => {
            collect_subexpr_violations_in_expr(receiver, suspending, out)
        }
        Expr::IndexAccess {
            receiver, index, ..
        } => {
            collect_subexpr_violations_in_expr(receiver, suspending, out);
            collect_subexpr_violations_in_expr(index, suspending, out);
        }
        Expr::StructLit { fields, .. } => {
            for f in fields {
                collect_subexpr_violations_in_expr(&f.value, suspending, out);
            }
        }
        Expr::ArrayLit { elements, .. } => {
            for e in elements {
                collect_subexpr_violations_in_expr(e, suspending, out);
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_subexpr_violations_in_expr(k, suspending, out);
                collect_subexpr_violations_in_expr(v, suspending, out);
            }
        }
        Expr::PostfixOp { receiver, .. } => {
            collect_subexpr_violations_in_expr(receiver, suspending, out)
        }
        Expr::Is { expr: inner, .. } => collect_subexpr_violations_in_expr(inner, suspending, out),
        Expr::InterpolatedString(parts, _) => {
            for p in parts {
                if let ynz_ast::nodes::StringPart::Expr(e, _) = p {
                    collect_subexpr_violations_in_expr(e, suspending, out);
                }
            }
        }
        // Leaf nodes — no calls.
        Expr::Ident(_, _)
        | Expr::StringLit(_, _)
        | Expr::IntLit(_, _)
        | Expr::NumberLit(_, _)
        | Expr::BoolLit(_, _)
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. }
        | Expr::Error(_) => {}
    }
}

/// Return true if `c` is a call to a suspending function or may-block intrinsic.
fn is_suspending_call(
    c: &ynz_ast::nodes::CallExpr,
    suspending: &std::collections::HashSet<&str>,
) -> bool {
    if let Some(name) = call_name(c) {
        return suspending.contains(name.as_str()) || is_base_suspension_intrinsic(name.as_str());
    }
    false
}

/// Extract the function name from a `CallExpr`'s callee, if it's a bare `Ident`.
fn call_name(c: &ynz_ast::nodes::CallExpr) -> Option<String> {
    if let Expr::Ident(name, _) = &c.callee {
        Some(name.clone())
    } else {
        None
    }
}

fn body_has_error_node(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Expr(e) => expr_has_error(e),
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_has_error(value),
        Stmt::If { cond, body, .. } => expr_has_error(cond) || body_has_error_node(&body.stmts),
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            expr_has_error(scrutinee)
                || arms.iter().any(|arm| body_has_error_node(&arm.body.stmts))
                || else_arm
                    .as_ref()
                    .is_some_and(|b| body_has_error_node(&b.stmts))
        }
        Stmt::While { cond, body, .. }
        | Stmt::For {
            iter: cond, body, ..
        } => expr_has_error(cond) || body_has_error_node(&body.stmts),
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_has_error),
        // M4 P3a: field assignment — not yet type-checked.
        Stmt::FieldAssign { target, value, .. } => expr_has_error(target) || expr_has_error(value),
        // M5 P1: index assignment — parser does not construct in P1; reached only if
        // out-of-sequence change happens. Walk sub-expressions for safety.
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => expr_has_error(receiver) || expr_has_error(index) || expr_has_error(value),
    })
}

fn expr_has_error(expr: &Expr) -> bool {
    match expr {
        Expr::Error(_) => true,
        Expr::Call(c) => expr_has_error(&c.callee) || c.args.iter().any(expr_has_error),
        Expr::BinOp { lhs, rhs, .. } => expr_has_error(lhs) || expr_has_error(rhs),
        Expr::UnaryOp { operand, .. } => expr_has_error(operand),
        Expr::MethodCall { receiver, args, .. } => {
            expr_has_error(receiver) || args.iter().any(expr_has_error)
        }
        Expr::Ident(_, _)
        | Expr::StringLit(_, _)
        | Expr::IntLit(_, _)
        | Expr::NumberLit(_, _)
        | Expr::BoolLit(_, _)
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. } => false,
        // M4 P3a: shape expressions — propagate error check into sub-expressions.
        Expr::FieldAccess { receiver, .. } | Expr::PostfixOp { receiver, .. } => {
            expr_has_error(receiver)
        }
        Expr::StructLit { fields, .. } => fields.iter().any(|f| expr_has_error(&f.value)),
        // M5 P1: bracket-index — propagate error check into receiver + index.
        Expr::IndexAccess {
            receiver, index, ..
        } => expr_has_error(receiver) || expr_has_error(index),
        // M5 P3b: array literal — propagate error check into all elements.
        Expr::ArrayLit { elements, .. } => elements.iter().any(expr_has_error),
        // M5 P3c: map literal — propagate error check into all keys and values.
        Expr::MapLit { entries, .. } => entries
            .iter()
            .any(|(k, v)| expr_has_error(k) || expr_has_error(v)),
        // M6: is-expression — propagate into the scrutinee.
        Expr::Is { expr, .. } => expr_has_error(expr),
        // M7: interpolated string — propagate into each interpolated sub-expression.
        Expr::InterpolatedString(parts, _) => parts.iter().any(|p| match p {
            ynz_ast::nodes::StringPart::Lit(_, _) => false,
            ynz_ast::nodes::StringPart::Expr(e, _) => expr_has_error(e),
        }),
        // M8 P5: wait/background — propagate into inner expression.
        Expr::Wait(inner, _) | Expr::Background(inner, _) => expr_has_error(inner),
    }
}

fn binop_display(op: &BinOpKind) -> &'static str {
    use BinOpKind::*;
    match op {
        Add => "+",
        Sub => "-",
        Mul => "*",
        Div => "/",
        Rem => "%",
        Lt => "<",
        LtEq => "<=",
        Gt => ">",
        GtEq => ">=",
        EqEq => "==",
        NotEq => "!=",
        And => "&&",
        Or => "||",
        BitAnd => "&",
        BitOr => "|",
        BitXor => "^",
        Shl => "<<",
        Shr => ">>",
    }
}

fn suggest_conversion(lhs: &Type, rhs: &Type) -> String {
    match (lhs, rhs) {
        (Type::Int, Type::Number { .. }) => {
            "Convert the `int` to `number`: `myInt.toNumber() + myNumber`".to_string()
        }
        (Type::Number { .. }, Type::Int) => {
            "Convert the `int` to `number`: `myNumber + myInt.toNumber()`".to_string()
        }
        (Type::Int, Type::Float) => {
            "Convert the `int` to `float`: `myInt.toFloat() + myFloat`".to_string()
        }
        (Type::Float, Type::Int) => {
            "Convert the `int` to `float`: `myFloat + myInt.toFloat()`".to_string()
        }
        (Type::Number { .. }, Type::Float) | (Type::Float, Type::Number { .. }) => {
            "Converting between `number` and `float` can lose precision either way. \
             Use `.toFloat()` to convert `number` → `float` (loses decimal precision), \
             or `.toNumber()` to convert `float` → `number` (may lose binary precision)."
                .to_string()
        }
        _ => "Check that both sides have the same type.".to_string(),
    }
}

/// Build a "not defined" diagnostic.
///
/// Searches `candidates` for a Levenshtein-close alternative.  When one is
/// found the `what_instead` reads "Did you mean `close`?"; otherwise
/// `fallback_what_instead` is used.
/// Tier 3 lint: emit a Warning at each use site when the same inline shape
/// appears 2 or more times in the module. Suggests extracting to a named `shape`.
///
/// Per `.claude/rules/auto-promotion.md`: threshold = 2+ uses, Warning severity,
/// emit at EVERY use site so the IDE underlines each one.
///
/// Synthetic compiler-generated shapes (non-`__anon__*`) are skipped — only
/// user-written inline shapes trigger the lint.
///
/// Time: O(n) where n = number of type-annotation nodes in the module.
/// Space: O(k) where k = number of distinct inline shapes.
fn lint_repeated_inline_shapes(module: &Module, diags: &mut DiagnosticBucket) {
    use crate::shapes::canonical_anon_name;

    // Collect (canonical_name, rendered_fields, span) for every AnonShape in the module.
    let mut uses: Vec<(String, String, SourceSpan)> = Vec::new();

    collect_anon_uses_in_module(module, &mut uses, &canonical_anon_name);

    // Count occurrences per canonical name.
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (name, _, _) in &uses {
        *counts.entry(name.clone()).or_insert(0) += 1;
    }

    // Emit a Warning at each use site for shapes that appear 2+ times.
    for (canonical, rendered, span) in &uses {
        let n = *counts.get(canonical).unwrap_or(&0);
        if n < 2 {
            continue;
        }
        let fields_block = render_fields_block(rendered);
        diags.push(Diagnostic::warning(
            span.clone(),
            format!("Inline shape `{rendered}` is used in {n} places."),
            format!(
                "Consider extracting to a named shape:\nshape SuggestedName {{\n{fields_block}\n}}\n\
                 Then reference `SuggestedName` at each of the {n} use sites."
            ),
            "Inline shapes are the right tool for one-off types — they keep the type definition \
             next to its only use. Repeated identical inline shapes duplicate the definition, so \
             a future change has to be made in multiple places. A named shape gives one source \
             of truth and a meaningful identifier in diagnostics.".to_string(),
        ));
    }
}

/// Walk a module and collect `(canonical_name, rendered, span)` for each `AnonShape`.
fn collect_anon_uses_in_module(
    module: &Module,
    out: &mut Vec<(String, String, SourceSpan)>,
    canonical: &impl Fn(&[ynz_ast::nodes::FieldDecl]) -> String,
) {
    for item in &module.items {
        match item {
            Item::Function(f) => {
                collect_anon_uses_in_type(&f.return_type, out, canonical);
                for param in &f.params {
                    collect_anon_uses_in_type(&param.ty, out, canonical);
                }
                collect_anon_uses_in_stmts(&f.body.stmts, out, canonical);
            }
            Item::ShapeDecl(s) => {
                for field in &s.fields {
                    collect_anon_uses_in_type(&field.ty, out, canonical);
                }
            }
            Item::ConstDecl(c) => {
                if let Some(ty) = &c.ty {
                    collect_anon_uses_in_type(ty, out, canonical);
                }
            }
            Item::ImportDecl(_) | Item::OptionsDecl(_) | Item::ReExport(_) => {}
        }
    }
}

fn collect_anon_uses_in_type(
    ty: &ynz_ast::nodes::Type,
    out: &mut Vec<(String, String, SourceSpan)>,
    canonical: &impl Fn(&[ynz_ast::nodes::FieldDecl]) -> String,
) {
    use ynz_ast::nodes::Type as AstType;
    match ty {
        AstType::AnonShape { fields, span } => {
            let name = canonical(fields);
            let rendered = render_inline_shape(fields);
            out.push((name, rendered, span.clone()));
            for field in fields {
                collect_anon_uses_in_type(&field.ty, out, canonical);
            }
        }
        AstType::Maybe { inner, .. } => collect_anon_uses_in_type(inner, out, canonical),
        AstType::Union { variants, .. } => {
            for v in variants {
                collect_anon_uses_in_type(v, out, canonical);
            }
        }
        AstType::Generic { args, .. } => {
            for a in args {
                collect_anon_uses_in_type(a, out, canonical);
            }
        }
        AstType::ErrorCapable { inner, .. } => collect_anon_uses_in_type(inner, out, canonical),
        AstType::Sensitive(inner) => collect_anon_uses_in_type(inner, out, canonical),
        _ => {}
    }
}

fn collect_anon_uses_in_stmts(
    stmts: &[Stmt],
    out: &mut Vec<(String, String, SourceSpan)>,
    canonical: &impl Fn(&[ynz_ast::nodes::FieldDecl]) -> String,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Let { ty: Some(ty), .. } => collect_anon_uses_in_type(ty, out, canonical),
            Stmt::Let { .. }
            | Stmt::Assign { .. }
            | Stmt::Return { .. }
            | Stmt::Expr(_)
            | Stmt::If { .. }
            | Stmt::Match { .. }
            | Stmt::While { .. }
            | Stmt::For { .. }
            | Stmt::FieldAssign { .. }
            | Stmt::IndexAssign { .. } => {}
        }
    }
}

/// Render an `AnonShape`'s fields as `{ field: type, ... }` for the WHAT message.
fn render_inline_shape(fields: &[ynz_ast::nodes::FieldDecl]) -> String {
    let mut sorted: Vec<_> = fields.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let parts: Vec<String> = sorted
        .iter()
        .map(|f| format!("{}: {}", f.name, render_ast_type(&f.ty)))
        .collect();
    format!("{{ {} }}", parts.join(", "))
}

/// Render an `AnonShape`'s fields as a `shape` body block (2-space indent per field).
fn render_fields_block(rendered: &str) -> String {
    // `rendered` is like `{ a: int, b: string }` — strip outer braces, split on `, `.
    let inner = rendered.trim_start_matches("{ ").trim_end_matches(" }");
    inner
        .split(", ")
        .map(|f| format!("  {f}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_ast_type(ty: &ynz_ast::nodes::Type) -> String {
    use ynz_ast::nodes::Type as AstType;
    match ty {
        AstType::Int => "int".to_string(),
        AstType::Float => "float".to_string(),
        AstType::Number { .. } => "number".to_string(),
        AstType::Bool => "bool".to_string(),
        AstType::Nothing => "nothing".to_string(),
        AstType::Named(n, _) => n.clone(),
        AstType::Maybe { inner, .. } => format!("maybe {}", render_ast_type(inner)),
        AstType::AnonShape { fields, .. } => render_inline_shape(fields),
        AstType::Generic { name, args, .. } => {
            let arg_str: Vec<_> = args.iter().map(render_ast_type).collect();
            format!("{}<{}>", name, arg_str.join(", "))
        }
        AstType::Union { variants, .. } => {
            let parts: Vec<_> = variants.iter().map(render_ast_type).collect();
            parts.join(" | ")
        }
        AstType::ErrorCapable { inner, .. } => format!("{} errors", render_ast_type(inner)),
        AstType::Sensitive(inner) => format!("sensitive {}", render_ast_type(inner)),
        _ => "?".to_string(),
    }
}

fn make_not_defined_diag(
    name: &str,
    span: SourceSpan,
    candidates: &[&str],
    fallback_what_instead: String,
    why: &str,
) -> Diagnostic {
    let suggestion = find_closest_name(name, candidates);
    let what_instead = match suggestion {
        Some(close) => format!("Did you mean `{close}`?"),
        None => fallback_what_instead,
    };
    Diagnostic::error(span, format!("`{name}` is not defined."), what_instead, why)
        .with_kind(ynz_diagnostics::DiagnosticKind::NotDefined)
}

/// Find the closest name using Levenshtein distance — for "did you mean?" suggestions.
pub fn find_closest_name<'a>(target: &str, candidates: &[&'a str]) -> Option<&'a str> {
    let threshold = match target.len() {
        0..=2 => 0,
        3..=4 => 1,
        _ => 2,
    };
    candidates
        .iter()
        .filter_map(|&c| {
            let dist = levenshtein(target, c);
            if dist <= threshold {
                Some((dist, c))
            } else {
                None
            }
        })
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

/// Levenshtein distance between two strings.
///
/// Time: O(m×n) where m, n = string lengths. Space: O(min(m, n)) — two-row
/// rolling DP instead of an m×n matrix. Operates on bytes (identifiers are ASCII).
fn levenshtein(a: &str, b: &str) -> usize {
    let a = a.as_bytes();
    let b = b.as_bytes();
    let (m, n) = (a.len(), b.len());
    // Early exit when length difference already exceeds any useful threshold.
    if m.abs_diff(n) > 2 {
        return m.abs_diff(n);
    }
    // Keep the shorter string in `b` so the inner row is as small as possible.
    let (a, b, m, n) = if m < n { (b, a, n, m) } else { (a, b, m, n) };
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            curr[j] = if a[i - 1] == b[j - 1] {
                prev[j - 1]
            } else {
                1 + prev[j].min(curr[j - 1]).min(prev[j - 1])
            };
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Return the identifier name if `expr` is a bare `Ident` or `SelfValue`.
///
/// Used for ownership checking: ownership enforcement only applies to direct
/// binding references, not to computed expressions like `foo()` or `a + b`.
fn simple_ident_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name, _) => Some(name.as_str()),
        Expr::SelfValue { .. } => Some("self"),
        _ => None,
    }
}

/// Walk a field-access chain to find the root binding name.
///
/// `player.inner.health` → `Some("player")`
/// `self.field` → `Some("self")`
/// Anything not rooted in a simple identifier → `None`.
fn root_binding_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name, _) => Some(name.as_str()),
        Expr::SelfValue { .. } => Some("self"),
        Expr::FieldAccess { receiver, .. } => root_binding_name(receiver),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ynz_ast::nodes::{Block, CallExpr, Expr, FunctionDecl, Item, Module, Stmt};
    use ynz_diagnostics::SourceSpan;

    fn span(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new("test.ynz", start, end)
    }

    #[test]
    fn type_mismatch_produces_three_part_diagnostic() {
        // WHY: this is the load-bearing test for the type-mismatch code path.
        // The test uses a test-only intrinsic to avoid needing full M2 types.
        let module = Module {
            items: vec![Item::Function(FunctionDecl {
                name: "entrypoint".into(),
                generics: vec![],
                params: vec![],
                return_type: AstType::Nothing,
                body: Block {
                    stmts: vec![Stmt::Expr(Expr::Call(Box::new(CallExpr {
                        callee: Expr::Ident("_test_takes_nothing".into(), span(29, 48)),
                        type_args: None,
                        args: vec![Expr::StringLit(b"hi".to_vec(), span(49, 53))],
                        span: span(29, 54),
                    })))],
                    span: span(28, 56),
                },
                span: span(0, 57),
                name_span: span(9, 13),
                // test-ratchet: M7 P1 adds errors_capable field to FunctionDecl
                errors_capable: false,
                // test-ratchet: M8 P1 adds is_exported
                is_exported: false,
                // test-ratchet: M8 P3 adds doc
                doc: None,
            })],
            span: span(0, 57),
        };

        let intrinsics = PrimitiveIntrinsicTable::m3().with_test_intrinsic(
            "_test_takes_nothing",
            vec![Type::Nothing],
            Type::Nothing,
        );
        let shape_table = crate::shapes::collect_shapes(
            &module,
            &Default::default(),
            &Default::default(),
            &mut DiagnosticBucket::new(),
        );
        let generic_shape_table =
            crate::shapes::collect_generic_shapes(&module, &mut DiagnosticBucket::new());
        let sig_table = crate::signatures::collect_signatures(
            &module,
            &mut DiagnosticBucket::new(),
            &shape_table,
        );
        let generic_fn_table = crate::signatures::collect_generic_signatures(
            &module,
            &mut DiagnosticBucket::new(),
            &shape_table,
        );
        let (_, _, diags, _) = check(
            &module,
            &sig_table,
            &shape_table,
            &generic_fn_table,
            &generic_shape_table,
            &intrinsics,
            &std::collections::HashMap::new(),
        );
        let diags: Vec<_> = diags.into_iter().collect();
        assert_eq!(
            diags.len(),
            1,
            "Expected 1 type-mismatch diagnostic, got: {diags:#?}"
        );

        let d = &diags[0];
        assert!(!d.what.is_empty(), "what must be non-empty");
        assert!(!d.what_instead.is_empty(), "what_instead must be non-empty");
        assert!(!d.why.is_empty(), "why must be non-empty");
    }
}
