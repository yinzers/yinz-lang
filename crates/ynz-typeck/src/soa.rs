//! Auto-SoA candidate analysis (v0.3-M5 Phase 4 step 1).
//!
//! Walks each concrete function body and classifies every `array<Shape>` binding
//! (and every `array<Shape>` parameter) as an SoA candidate with an explicit
//! [`SoaVerdict`] — either `Admitted` (provably-safe large hot-loop array) or
//! `Declined` with the specific [`SoaDeclineReason`]. The walk consumes
//! `TypedModule::expr_types` as its type oracle (the S2 spike's proven approach —
//! no re-inference, no second type derivation).
//!
//! Declining is ALWAYS safe (the M3d discipline): when a use pattern is outside the
//! supported hot-loop model, the binding declines with a human-readable reason rather
//! than admitting a case codegen cannot prove.
//!
//! The layout AUTHORITY that resolves these candidates against the false-sharing
//! padded set (E3's one-source mitigation, recorded decision D11) lands as Phase 4
//! step 2 in this same module; codegen consumes ONLY that authority, never this
//! candidate list directly.

use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::{Block, Expr, FunctionDecl, Item, OwnershipModifier, Stmt, StringPart};
use ynz_diagnostics::SourceSpan;

use crate::{check::TypedModule, shapes::ShapeTable, types::Type};

/// Minimum compile-time-provable element count for SoA admission (strict `>`).
///
/// Guessed constant — provenance pending Phase 6 benchmark calibration (plan
/// `2026-07-03-v0-3-m5-auto-soa` risk E2: the hazard is false confidence, not the
/// number). Phase 6 replaces or re-confirms this WITH a committed provenance record.
pub const SOA_SIZE_THRESHOLD: i64 = 64;

/// Array intrinsic methods whose receiver-position use does NOT escape the binding.
///
/// `.add` is additionally a growth signal (declines via [`SoaDeclineReason::Grown`]);
/// `.set` is mutation, not growth — a set-mutated array stays admitted. Any method
/// NOT in this list is a UFCS user call (`arr.foo()` IS `foo(arr)`) and escapes (D4).
const KNOWN_ARRAY_INTRINSICS: &[&str] = &[
    "get", "set", "add", "count", "first", "last", "contains", "copy",
];

/// Harness-only layout override, parsed from `YNZ_SOA_FORCE` (recorded decision D8).
///
/// NEVER user-facing: no CLI flag exposes it and no diagnostic mentions it. It exists
/// solely so the Phase 6 calibration harness can pin each compiled workload to one
/// layout and measure the two on equal footing. Precedence is absolute: kernel mode
/// and `YNZ_NO_AUTO_PARALLEL` outrank both values (the "no-auto-parallel disables
/// SoA" gate stays a hard invariant).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoaForce {
    /// Skip ONLY the `BelowSizeThreshold` arm of the verdict chain. Every safety
    /// decline (escape / growth / length-not-provable / lend-self / field-union)
    /// stays live — forcing SoA onto an unsafe pattern is never possible.
    Soa,
    /// Empty the candidate set entirely (mirrors the no-auto-parallel gate);
    /// codegen then lowers plain AoS — the same lowering as an authority-declined
    /// row, since the SoA interception keys on `LayoutKind::Soa` only.
    Aos,
}

/// Why an `array<Shape>` binding was declined for SoA layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SoaDeclineReason {
    /// The element shape is cross-thread padded — padding wins, always (recorded
    /// decision D11). Assigned ONLY by the layout authority (`resolve_layout`),
    /// never by the candidate walk in this file.
    CrossThreadPadded,
    /// The initializer is not an array literal, so the length is not
    /// compile-time provable (D3).
    LengthNotProvable,
    /// Provable length is not strictly greater than [`SOA_SIZE_THRESHOLD`].
    BelowSizeThreshold { len: i64 },
    /// A growth operation (`.add`) appears on the binding (D3).
    Grown { op: String },
    /// The binding escapes the single-function hot-loop model (D4).
    Escapes { how: String },
    /// A standalone function takes `lend self: <Shape>` for the element shape —
    /// SoA would split storage that function expects contiguous (roadmap risk E6).
    /// Shape-level: fires regardless of call sites.
    LendSelfMethod { function: String },
    /// The per-field loop-access union across all for-in loops exceeds 2 fields (D5).
    FieldUnionTooWide { fields: Vec<String> },
    /// No for-in loop over the binding reads any element field — nothing to
    /// segment for; AoS stays optimal.
    NoPerFieldLoopAccess,
}

/// The admission verdict for one candidate.
#[derive(Clone, Debug, PartialEq)]
pub enum SoaVerdict {
    Admitted {
        provable_len: i64,
        /// Fields the hot loops actually read/write, in the shape's declared
        /// field order.
        hot_fields: Vec<String>,
    },
    Declined(SoaDeclineReason),
}

/// One `array<Shape>` binding or parameter, with its analysis verdict.
///
/// Params are recorded as declined rows (`Escapes` — cross-function by definition,
/// S2 finding 5's explicit posture), never silently absent: Phase 8's enumeration
/// report needs the row.
#[derive(Clone, Debug, PartialEq)]
pub struct SoaCandidate {
    pub array_name: String,
    pub shape_name: String,
    pub decl_span: SourceSpan,
    pub verdict: SoaVerdict,
    /// Static count of whole-element uses inside hot loops (`process(v)`,
    /// `v.method()`). NOT a decline and NOT in the field union — the cold
    /// fan-out input Phase 5's lowering consumes.
    pub whole_value_uses: usize,
}

/// One field's slot in an SoA-laid-out array buffer, in the shape's DECLARED
/// field order (`segment_index` = declared position).
///
/// Segments carry field ORDER + NAMES only — the E10 forward-compat surface
/// (v0.8's compile-time serializer codegen reconstructs unified values from it).
/// Byte offsets are CODEGEN-time values derived from `cap × shape_abi_sizes`
/// (the ONE ABI-size source, P2); typeck MUST NOT re-derive ABI sizes here
/// (CCIR-2 / authoritative-derivation.md).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldSegment {
    pub field: String,
    pub segment_index: usize,
}

/// The resolved layout for one `array<Shape>` binding.
#[derive(Clone, Debug, PartialEq)]
pub enum LayoutKind {
    /// Struct-of-Arrays: ONE allocation, one segment per declared field (D2).
    Soa { segments: Vec<FieldSegment> },
    /// Array-of-Structs (the by-value default), with the reason SoA declined.
    Aos { declined: SoaDeclineReason },
}

/// One array's resolved layout decision.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutDecision {
    pub array_name: String,
    pub shape_name: String,
    pub decl_span: SourceSpan,
    pub kind: LayoutKind,
}

/// THE one authoritative layout source (E3 B1 mitigation).
///
/// Every layout consumer — codegen's shape-type emission, the padded-alloca
/// alignment read, `frame_layouts_query`'s sizing pass, and Phase 5's SoA
/// lowering — reads layout answers HERE, never from a second derivation.
/// `padded_shapes` is threaded (cloned) from
/// `TypedModule::cross_thread_padded_shapes` — that field stays the raw
/// finalize record; consumers read THIS set.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutDecisions {
    pub padded_shapes: HashSet<String>,
    pub arrays: Vec<LayoutDecision>,
}

/// Resolve the SoA candidate list against the false-sharing padded set into
/// the one layout authority.
///
/// Precedence per recorded decision D11: **padding wins, always** — an
/// `Admitted` candidate whose shape is cross-thread padded resolves to
/// `Aos { declined: CrossThreadPadded }`; its arrays are never SoA'd
/// (correctness under false sharing outranks cache locality). Admitted
/// survivors get `Soa` segments = ALL declared fields in declared order
/// (cold fields ride their own segments — Phase 5's fan-out input).
///
/// Time: O(C · F) where C = candidates, F = fields per shape.  Space: O(C · F).
pub fn resolve_layout(
    padded: &HashSet<String>,
    candidates: &[SoaCandidate],
    shape_table: &ShapeTable,
) -> LayoutDecisions {
    let arrays = candidates
        .iter()
        .map(|c| {
            let kind = match &c.verdict {
                SoaVerdict::Admitted { .. } if padded.contains(&c.shape_name) => LayoutKind::Aos {
                    declined: SoaDeclineReason::CrossThreadPadded,
                },
                SoaVerdict::Admitted { .. } => LayoutKind::Soa {
                    segments: segment_all_declared_fields(&c.shape_name, shape_table),
                },
                SoaVerdict::Declined(reason) => LayoutKind::Aos {
                    declined: reason.clone(),
                },
            };
            LayoutDecision {
                array_name: c.array_name.clone(),
                shape_name: c.shape_name.clone(),
                decl_span: c.decl_span.clone(),
                kind,
            }
        })
        .collect();
    LayoutDecisions {
        padded_shapes: padded.clone(),
        arrays,
    }
}

/// ALL declared fields, declared order, `segment_index` = declared position (D2).
fn segment_all_declared_fields(shape_name: &str, shape_table: &ShapeTable) -> Vec<FieldSegment> {
    match shape_table.get(shape_name) {
        Some(def) => def
            .fields
            .iter()
            .enumerate()
            .map(|(i, fd)| FieldSegment {
                field: fd.name.clone(),
                segment_index: i,
            })
            .collect(),
        // Admitted candidates always have table-known shapes (the walk keys on
        // `Type::Shape` the checker resolved); defensive empty keeps this total.
        None => Vec::new(),
    }
}

/// Per-binding signal accumulator for one function body walk.
struct BindingSignals {
    shape_name: String,
    decl_span: SourceSpan,
    /// `Some(n)` when the initializer is an `ArrayLit` with n elements.
    provable_len: Option<i64>,
    /// First escape seen (first occurrence wins — deterministic).
    escape: Option<String>,
    /// First growth op seen.
    growth: Option<String>,
    /// Unique element fields accessed in for-in loops, insertion-ordered
    /// (re-ordered to declared field order at verdict time).
    field_union: Vec<String>,
    whole_value_uses: usize,
}

/// Pure core of the SoA candidate analysis.
///
/// Takes the gate flags and the harness-only force override explicitly (the
/// `compute_cpu_promotions` pattern) so unit tests can drive the decline paths
/// deterministically; the salsa entry (`soa_candidate_query`) threads
/// `no_auto_parallel_env()` — the ONE predicate — today's always-false production
/// kernel flag (D1), and `soa_force_env()` (D8).
///
/// FFI check (D7): vacuously true — v0.3 Yinz has no FFI/export ABI surface an
/// array could be reachable from (FFI is v2+ per A5/D7); when one exists, an
/// FFI-reachable array must decline here.
///
/// Time: O(F · B) where F = functions, B = body nodes.  Space: O(bindings).
pub fn analyze(
    typed: &TypedModule,
    shape_table: &ShapeTable,
    kernel_mode: bool,
    no_auto_parallel: bool,
    force: Option<SoaForce>,
) -> Vec<SoaCandidate> {
    // Kernel mode has no scheduler-facing perf model to serve, and
    // --no-auto-parallel selects the sequential oracle posture: both yield the
    // empty candidate set (mirrors finalize_false_sharing's gate). Checked BEFORE
    // the force override — kernel / no-auto-parallel outrank YNZ_SOA_FORCE (D8).
    if kernel_mode || no_auto_parallel {
        return Vec::new();
    }
    if force == Some(SoaForce::Aos) {
        return Vec::new();
    }

    let lend_self_shapes = collect_lend_self_shapes(&typed.module.items, shape_table);
    let force_soa = force == Some(SoaForce::Soa);

    let mut out = Vec::new();
    for item in &typed.module.items {
        if let Item::Function(f) = item {
            // Generic functions are outside the P4 admission model: their bodies
            // type-check per-instantiation via the mono table, so a single
            // AST-walk classification has no one concrete element shape to key on.
            if !f.generics.is_empty() {
                continue;
            }
            analyze_function(
                f,
                typed,
                shape_table,
                &lend_self_shapes,
                force_soa,
                &mut out,
            );
        }
    }
    out
}

/// Shape name → first standalone `lend self` function taking that shape (E6 filter).
///
/// Mirrors `finalize_false_sharing`'s declaration scan: first match in item order
/// wins (deterministic).
fn collect_lend_self_shapes(items: &[Item], shape_table: &ShapeTable) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    for item in items {
        let Item::Function(f) = item else { continue };
        let Some(first) = f.params.first() else {
            continue;
        };
        if first.ownership != Some(OwnershipModifier::Lend) {
            continue;
        }
        if let Type::Shape { name } = shape_table.resolve_ast_type(&first.ty) {
            map.entry(name).or_insert_with(|| f.name.clone());
        }
    }
    map
}

fn analyze_function(
    f: &FunctionDecl,
    typed: &TypedModule,
    shape_table: &ShapeTable,
    lend_self_shapes: &HashMap<String, String>,
    force_soa: bool,
    out: &mut Vec<SoaCandidate>,
) {
    // Params first: an `array<Shape>` param is cross-function by definition (D4) —
    // recorded as a declined row, never silently absent (S2 finding 5 posture).
    for p in &f.params {
        if let Type::BuiltinArray { elem } = shape_table.resolve_ast_type(&p.ty) {
            if let Type::Shape { name } = *elem {
                out.push(SoaCandidate {
                    array_name: p.name.clone(),
                    shape_name: name,
                    decl_span: p.span.clone(),
                    verdict: SoaVerdict::Declined(SoaDeclineReason::Escapes {
                        how: "function parameter — cross-function by definition (D4)".to_string(),
                    }),
                    whole_value_uses: 0,
                });
            }
        }
    }

    // Pass 1 — collect tracked Let bindings (walk order = output order).
    let mut st = ScanState {
        typed,
        bindings: HashMap::new(),
        order: Vec::new(),
        loop_stack: Vec::new(),
    };
    collect_bindings_block(&f.body, &mut st);
    if st.order.is_empty() {
        return;
    }

    // Pass 2 — whole-body use scan (escape / growth / per-field loop union).
    scan_block(&f.body, &mut st);

    // Verdicts, in the banked decline precedence (each fixture trips exactly ONE
    // reason): escape → growth → length-not-provable → below-threshold →
    // lend-self → empty-union → union>2 → Admitted.
    for name in std::mem::take(&mut st.order) {
        let sig = st.bindings.remove(&name).expect("ordered binding exists");
        let reason = if let Some(how) = sig.escape {
            Some(SoaDeclineReason::Escapes { how })
        } else if let Some(op) = sig.growth {
            Some(SoaDeclineReason::Grown { op })
        } else if sig.provable_len.is_none() {
            Some(SoaDeclineReason::LengthNotProvable)
        } else if !force_soa && sig.provable_len.unwrap_or(0) <= SOA_SIZE_THRESHOLD {
            // `YNZ_SOA_FORCE=soa` (D8, harness-only) skips ONLY this threshold arm;
            // every other decline in the chain is a safety filter and stays live.
            Some(SoaDeclineReason::BelowSizeThreshold {
                len: sig.provable_len.unwrap_or(0),
            })
        } else if let Some(function) = lend_self_shapes.get(&sig.shape_name) {
            Some(SoaDeclineReason::LendSelfMethod {
                function: function.clone(),
            })
        } else if sig.field_union.is_empty() {
            Some(SoaDeclineReason::NoPerFieldLoopAccess)
        } else if sig.field_union.len() > 2 {
            Some(SoaDeclineReason::FieldUnionTooWide {
                fields: declared_order(&sig.field_union, &sig.shape_name, shape_table),
            })
        } else {
            None
        };

        let verdict = match reason {
            Some(r) => SoaVerdict::Declined(r),
            None => SoaVerdict::Admitted {
                provable_len: sig.provable_len.unwrap_or(0),
                hot_fields: declared_order(&sig.field_union, &sig.shape_name, shape_table),
            },
        };
        out.push(SoaCandidate {
            array_name: name,
            shape_name: sig.shape_name,
            decl_span: sig.decl_span,
            verdict,
            whole_value_uses: sig.whole_value_uses,
        });
    }
}

/// Re-order an accessed-field set to the shape's declared field order
/// (deterministic; matches the segment derivation the authority uses).
fn declared_order(fields: &[String], shape_name: &str, shape_table: &ShapeTable) -> Vec<String> {
    match shape_table.get(shape_name) {
        Some(def) => def
            .fields
            .iter()
            .filter(|fd| fields.iter().any(|f| f == &fd.name))
            .map(|fd| fd.name.clone())
            .collect(),
        // Shape unknown to the table (an earlier error already diagnosed it):
        // keep insertion order rather than invent one.
        None => fields.to_vec(),
    }
}

struct ScanState<'a> {
    typed: &'a TypedModule,
    bindings: HashMap<String, BindingSignals>,
    order: Vec<String>,
    /// Active for-in element contexts, innermost last: `(elem_var, binding_name)`.
    loop_stack: Vec<(String, String)>,
}

impl ScanState<'_> {
    fn is_tracked(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    /// The binding a loop element variable belongs to, if `name` is an active
    /// element var (innermost context wins — handles shadowing).
    fn elem_binding(&self, name: &str) -> Option<&str> {
        self.loop_stack
            .iter()
            .rev()
            .find(|(var, _)| var == name)
            .map(|(_, b)| b.as_str())
    }

    fn escape(&mut self, binding: &str, how: String) {
        if let Some(sig) = self.bindings.get_mut(binding) {
            if sig.escape.is_none() {
                sig.escape = Some(how);
            }
        }
    }

    fn growth(&mut self, binding: &str, op: &str) {
        if let Some(sig) = self.bindings.get_mut(binding) {
            if sig.growth.is_none() {
                sig.growth = Some(op.to_string());
            }
        }
    }

    fn field_access(&mut self, binding: &str, field: &str) {
        if let Some(sig) = self.bindings.get_mut(binding) {
            if !sig.field_union.iter().any(|f| f == field) {
                sig.field_union.push(field.to_string());
            }
        }
    }

    fn whole_value_use(&mut self, binding: &str) {
        if let Some(sig) = self.bindings.get_mut(binding) {
            sig.whole_value_uses += 1;
        }
    }
}

/// Pass 1 — register every `Stmt::Let` whose initializer types as
/// `array<Shape>` via the `expr_types` oracle (keyed `(span.start, span.end)` —
/// no re-inference, S2 finding 1).
fn collect_bindings_block(block: &Block, st: &mut ScanState<'_>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let {
                name, value, span, ..
            } => {
                // A same-name re-`let` (legal shadowing in non-suspending
                // functions — `check_let` just overwrites the scope entry)
                // rebinds the name: every signal recorded from the FIRST
                // initializer (provable_len above all) describes the ORIGINAL
                // array only, so an Admitted verdict would carry a stale
                // provable_len into Phase 5's buffer sizing (the E3/E7
                // silent-miscompile class — the re-`let` sibling of the Assign
                // arm's reassign-staleness decline in `scan_stmt`). Conservative
                // decline; fires regardless of the shadow's own type, because a
                // non-array shadow rebinds the name just the same.
                if st.bindings.contains_key(name) {
                    st.escape(name, "rebound by a later let".to_string());
                    continue;
                }
                let key = (value.span().start, value.span().end);
                if let Some(Type::BuiltinArray { elem }) = st.typed.expr_types.get(&key) {
                    if let Type::Shape { name: shape } = elem.as_ref() {
                        let provable_len = match value {
                            Expr::ArrayLit { elements, .. } => Some(elements.len() as i64),
                            _ => None,
                        };
                        st.order.push(name.clone());
                        st.bindings.insert(
                            name.clone(),
                            BindingSignals {
                                shape_name: shape.clone(),
                                decl_span: span.clone(),
                                provable_len,
                                escape: None,
                                growth: None,
                                field_union: Vec::new(),
                                whole_value_uses: 0,
                            },
                        );
                    }
                }
            }
            Stmt::If { body, .. } | Stmt::While { body, .. } | Stmt::For { body, .. } => {
                collect_bindings_block(body, st);
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    collect_bindings_block(&arm.body, st);
                }
                if let Some(else_body) = else_arm {
                    collect_bindings_block(else_body, st);
                }
            }
            Stmt::Expr(_)
            | Stmt::Assign { .. }
            | Stmt::Return { .. }
            | Stmt::FieldAssign { .. }
            | Stmt::IndexAssign { .. } => {}
        }
    }
}

/// Pass 2 — classify every use of a tracked binding / loop element var.
fn scan_block(block: &Block, st: &mut ScanState<'_>) {
    for stmt in &block.stmts {
        scan_stmt(stmt, st);
    }
}

fn scan_stmt(stmt: &Stmt, st: &mut ScanState<'_>) {
    match stmt {
        Stmt::Expr(e) => scan_expr(e, st),
        Stmt::Let { value, .. } => {
            scan_ident_in_value_position(value, st, "stored into another binding");
        }
        Stmt::Assign { target, value, .. } => {
            // Reassigning a tracked binding invalidates every signal recorded from
            // the ORIGINAL initializer (provable_len / growth / escape) — by verdict
            // time the name may hold a different array, so an Admitted verdict would
            // carry a stale provable_len into Phase 5's buffer sizing (the E3/E7
            // silent-miscompile class). Conservative decline, first signal wins.
            if st.elem_binding(target).is_none() && st.is_tracked(target) {
                st.escape(target, "reassigned after initial binding".to_string());
            }
            scan_ident_in_value_position(value, st, "stored into another binding");
        }
        Stmt::If { cond, body, .. } => {
            scan_expr(cond, st);
            scan_block(body, st);
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            scan_expr(scrutinee, st);
            for arm in arms {
                scan_block(&arm.body, st);
            }
            if let Some(else_body) = else_arm {
                scan_block(else_body, st);
            }
        }
        Stmt::While { cond, body, .. } => {
            scan_expr(cond, st);
            scan_block(body, st);
        }
        Stmt::For {
            var,
            iter,
            body,
            destructure_pattern,
            ..
        } => {
            let mut pushed = false;
            if let Expr::Ident(n, _) = iter {
                if st.elem_binding(n).is_none() && st.is_tracked(n) {
                    let binding = n.clone();
                    // Shape-destructuring (`for ({ x } in pts)`) IS per-field
                    // access: each destructured field joins the union directly.
                    if let Some(bindings) = destructure_pattern {
                        for b in bindings {
                            st.field_access(&binding, &b.field);
                        }
                    }
                    st.loop_stack.push((var.clone(), binding));
                    pushed = true;
                }
            }
            if !pushed {
                scan_expr(iter, st);
            }
            scan_block(body, st);
            if pushed {
                st.loop_stack.pop();
            }
        }
        Stmt::Return { value, .. } => {
            if let Some(e) = value {
                if let Expr::Ident(n, _) = e {
                    if st.elem_binding(n).is_none() && st.is_tracked(n) {
                        st.escape(&n.clone(), "returned from the function".to_string());
                        return;
                    }
                }
                scan_expr(e, st);
            }
        }
        Stmt::FieldAssign { target, value, .. } => {
            // A field WRITE through the loop element var is a per-field access
            // (mutation stays admitted — only growth/escape decline).
            scan_expr(target, st);
            scan_ident_in_value_position(value, st, "stored into a shape field");
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            // `arr[i] = v` is `.set` sugar — mutation, NOT growth; receiver
            // position is an allowed use.
            if !matches!(receiver.as_ref(), Expr::Ident(n, _)
                if st.elem_binding(n).is_none() && st.is_tracked(n))
            {
                scan_expr(receiver, st);
            }
            scan_expr(index, st);
            scan_ident_in_value_position(value, st, "stored into an array element");
        }
    }
}

/// A tracked binding appearing as a stored VALUE (RHS of a field/element write,
/// a struct-literal field, a map/array literal element) aliases the array → escape.
fn scan_ident_in_value_position(e: &Expr, st: &mut ScanState<'_>, how: &str) {
    if let Expr::Ident(n, _) = e {
        if st.elem_binding(n).is_none() && st.is_tracked(n) {
            st.escape(&n.clone(), format!("{how} (aliases the array)"));
            return;
        }
    }
    scan_expr(e, st);
}

fn scan_expr(e: &Expr, st: &mut ScanState<'_>) {
    match e {
        Expr::Ident(n, _) => {
            // Innermost element context wins over a same-named tracked binding.
            if let Some(binding) = st.elem_binding(n).map(str::to_string) {
                st.whole_value_use(&binding);
            } else if st.is_tracked(n) {
                st.escape(
                    &n.clone(),
                    "used outside the supported hot-loop pattern".to_string(),
                );
            }
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            match receiver.as_ref() {
                Expr::Ident(n, _) if st.elem_binding(n).is_none() && st.is_tracked(n) => {
                    let binding = n.clone();
                    if method == "add" {
                        st.growth(&binding, ".add()");
                    } else if !KNOWN_ARRAY_INTRINSICS.contains(&method.as_str()) {
                        // UFCS: `arr.foo()` IS `foo(arr)` — a user call escapes (D4).
                        st.escape(
                            &binding,
                            format!("passed to `{method}()` via UFCS dot-call (D4)"),
                        );
                    }
                }
                Expr::Ident(n, _) if st.elem_binding(n).is_some() => {
                    // `v.method()` on the loop element — whole-value use
                    // (cold fan-out input), not a decline (spike semantics kept).
                    let binding = st.elem_binding(n).map(str::to_string);
                    if let Some(b) = binding {
                        st.whole_value_use(&b);
                    }
                }
                _ => scan_expr(receiver, st),
            }
            for arg in args {
                scan_arg(arg, st, &format!("passed to method `{method}()`"));
            }
        }
        Expr::FieldAccess {
            receiver, field, ..
        } => match receiver.as_ref() {
            Expr::Ident(n, _) if st.elem_binding(n).is_some() => {
                let binding = st.elem_binding(n).map(str::to_string);
                if let Some(b) = binding {
                    st.field_access(&b, field);
                }
            }
            // Defensive (S2 finding 3): `pts[i].x` — index-form per-field access
            // attributes to the array binding directly.
            Expr::IndexAccess {
                receiver: inner,
                index,
                ..
            } if matches!(inner.as_ref(), Expr::Ident(n, _)
                if st.elem_binding(n).is_none() && st.is_tracked(n)) =>
            {
                if let Expr::Ident(n, _) = inner.as_ref() {
                    st.field_access(&n.clone(), field);
                }
                scan_expr(index, st);
            }
            _ => scan_expr(receiver, st),
        },
        Expr::IndexAccess {
            receiver, index, ..
        } => {
            // `arr[i]` receiver position is an allowed use (`.get` sugar).
            if !matches!(receiver.as_ref(), Expr::Ident(n, _)
                if st.elem_binding(n).is_none() && st.is_tracked(n))
            {
                scan_expr(receiver, st);
            }
            scan_expr(index, st);
        }
        Expr::Call(call) => {
            let callee_name = match &call.callee {
                Expr::Ident(n, _) => n.clone(),
                _ => "<expr>".to_string(),
            };
            // The callee itself is never a tracked-array use (arrays aren't
            // callable); only scan it when it's a non-ident expression.
            if !matches!(&call.callee, Expr::Ident(_, _)) {
                scan_expr(&call.callee, st);
            }
            for arg in &call.args {
                scan_arg(arg, st, &format!("passed to `{callee_name}()`"));
            }
        }
        Expr::PostfixOp { receiver, .. } => {
            // `.copy()` on the array produces a NEW owned array (tracked as its
            // own binding if let-bound) — an allowed use of the receiver.
            // `.freeze()` locks the binding; also not an escape.
            if !matches!(receiver.as_ref(), Expr::Ident(n, _)
                if st.elem_binding(n).is_none() && st.is_tracked(n))
            {
                if let Expr::Ident(n, _) = receiver.as_ref() {
                    if let Some(b) = st.elem_binding(n).map(str::to_string) {
                        st.whole_value_use(&b);
                        return;
                    }
                }
                scan_expr(receiver, st);
            }
        }
        Expr::Background(inner, _) | Expr::Wait(inner, _) => scan_expr(inner, st),
        Expr::BinOp { lhs, rhs, .. } => {
            scan_expr(lhs, st);
            scan_expr(rhs, st);
        }
        Expr::UnaryOp { operand, .. } => scan_expr(operand, st),
        Expr::StructLit { fields, .. } => {
            for f in fields {
                scan_ident_in_value_position(&f.value, st, "stored into a shape field");
            }
        }
        Expr::ArrayLit { elements, .. } => {
            for el in elements {
                scan_ident_in_value_position(el, st, "stored into another array");
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                scan_ident_in_value_position(k, st, "stored into a map");
                scan_ident_in_value_position(v, st, "stored into a map");
            }
        }
        Expr::InterpolatedString(parts, _) => {
            for part in parts {
                if let StringPart::Expr(inner, _) = part {
                    scan_expr(inner, st);
                }
            }
        }
        Expr::Is { .. }
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. }
        | Expr::StringLit(_, _)
        | Expr::IntLit(_, _)
        | Expr::NumberLit(_, _)
        | Expr::BoolLit(_, _)
        | Expr::Error(_) => {}
    }
}

/// An argument position: a tracked binding escapes; a loop element var is a
/// whole-value use (the cold fan-out signal, not a decline).
fn scan_arg(arg: &Expr, st: &mut ScanState<'_>, how: &str) {
    if let Expr::Ident(n, _) = arg {
        if let Some(b) = st.elem_binding(n).map(str::to_string) {
            st.whole_value_use(&b);
            return;
        }
        if st.is_tracked(n) {
            st.escape(&n.clone(), format!("{how} (D4)"));
            return;
        }
    }
    scan_expr(arg, st);
}
