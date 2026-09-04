//! Grammar-constrained `.ynz` program generator (v0.3-M8 Phase 4b, structured fuzzing).
//!
//! WHAT: a combinator generator that emits whole `.ynz` programs which are **type-valid by
//! construction** — it never emits a construct it has not already typed. Every statement is
//! drawn from a typed environment (`ints`, `arrays`, `maps`) and each
//! emission updates that environment, so a generated program compiles or the GENERATOR has a
//! bug. Token-level fuzzing is explicitly out of scope: it would spend its whole budget
//! re-proving that typeck rejects garbage.
//!
//! WHY it lives in `tests/` and not a crate: it is a test-only oracle input. The oracle itself
//! (byte-identical stdout/stderr/exit-code across the compilation-mode matrix) already exists in
//! `../cross_impl_consistency.rs`; this module supplies programs to it and re-derives none of it.
//!
//! The scope of the grammar, the mode matrix, the CI budget, and the replay-corpus mechanism are
//! documented in `README.md` beside this file.

use std::collections::BTreeSet;

// ── Deterministic RNG ─────────────────────────────────────────────────────────
//
// splitmix64. Deterministic from a seed so any generated program is reproducible from
// (generator revision, seed) alone — which is what makes the replay mechanism in README.md
// cheap: a finding is recorded as a seed plus the saved source, never as a binary blob.

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xDEAD_BEEF_CAFE_F00D)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n`. `n == 0` is a generator bug, not a runtime condition — every call
    /// site guards its pool for emptiness before drawing.
    fn below(&mut self, n: usize) -> usize {
        assert!(
            n > 0,
            "Rng::below(0) — caller failed to guard an empty pool"
        );
        (self.next_u64() % (n as u64)) as usize
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        debug_assert!(hi > lo);
        lo + (self.next_u64() % ((hi - lo) as u64)) as i64
    }

    fn one_in(&mut self, n: u64) -> bool {
        self.next_u64() % n == 0
    }

    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        let i = self.below(xs.len());
        &xs[i]
    }
}

// ── Channel element kinds (v0.3-M8 Phase 8 fix round 3 — the owned-heap widening) ─────────────
//
// v0.3-M8's transfer rules (`ConsumedBySend`, `TransferNeedsCopy`, `ParamNeedsGive`) and fr12's
// copy-through `number` marshalling were, until this round, exercised only by six hand-written
// fixtures. The claim that emitting them "would produce correctly-REJECTED programs" (the prior
// README wording) was checked directly against the generator's own idiom and was FALSE — a
// task-local `let` built and immediately sent needs no `.copy()`/`give` plumbing at all, because
// nothing else in the program ever reads it again. The only real bookkeeping is keeping a sent
// binding OUT of the pool other statements draw from (`Builder::arrays`/`Builder::maps`) — this
// generator achieves that by never registering a send-only local into either pool in the first
// place, and by removing a POOLED binding from its pool at the moment it is chosen to be sent
// (see `stmt_channel_inline_kind`'s reuse branch below).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ElemKind {
    Int,
    Array,
    Map,
    Number,
}

impl ElemKind {
    const ALL: [ElemKind; 4] = [
        ElemKind::Int,
        ElemKind::Array,
        ElemKind::Map,
        ElemKind::Number,
    ];

    fn type_name(self) -> &'static str {
        match self {
            ElemKind::Int => "int",
            ElemKind::Array => "array<int>",
            ElemKind::Map => "map<string, int>",
            ElemKind::Number => "number",
        }
    }

    fn channel_type(self) -> String {
        format!("channel<{}>", self.type_name())
    }
}

// ── Generated program ─────────────────────────────────────────────────────────

/// One generated program plus the facts a caller needs without re-parsing it.
pub struct Program {
    /// The `.ynz` source text.
    pub source: String,
    /// The seed it was generated from (the whole replay key, with the generator revision).
    pub seed: u64,
    /// True when the program spawns `background` work. NOT consulted by the oracle — the
    /// oracle derives scheduler-dependence from the SOURCE via
    /// `cross_impl_consistency::output_order_is_scheduler_dependent`, per
    /// `authoritative-derivation.md`. This field exists only so the harness can REPORT the
    /// concurrent/sequential split of a corpus, and it is asserted to agree with the
    /// authoritative classifier in this module's own tests.
    pub uses_background: bool,
    /// Number of TOP-LEVEL statements emitted into `entrypoint`'s body (the seed `let` plus
    /// every `emit_statement()` draw) — NOT a line count. A composite statement (the drain
    /// loop, the two-spawn topology) emits many source lines for ONE statement; counting lines
    /// instead of statements let this field average 43.9 against a 10-19 draw, so `>= 10` could
    /// never fail regardless of what the generator actually does. This field is now tied to the
    /// real invariant `build_body` guarantees (`stmt_count` is always `10..=19`).
    pub stmt_count: usize,
    /// The names of every suspending (`fetchN`) helper this program declared — the authoritative
    /// list the wait-prefix contract test checks against, instead of a hardcoded guess at what
    /// `declare_helpers` might have produced.
    pub io_fn_names: Vec<String>,
    /// True when `stmt_channel_inline_kind`'s optional close-then-receive-past-end-of-stream tail
    /// fired (the "close() in body" construct the per-construct floor test tracks).
    pub fired_inline_close: bool,
    /// True when the taught end-of-stream drain-loop composite fired (menu arm 12).
    pub fired_drain_loop: bool,
    /// True when the two-spawn Auto-Arc sharing topology fired (menu arm 13).
    pub fired_two_spawn_arc: bool,
    /// True when a standalone `map<string, int>` declaration statement fired (menu arm 7).
    pub fired_map_decl: bool,
    /// True when an array intrinsic statement (`.add()`/`.count()`/index) fired (menu arm 6).
    pub fired_array_intrinsic: bool,
    /// True when a shape-literal-plus-field-read statement fired (menu arm 9).
    pub fired_shape_field_read: bool,
    /// True when this program actually RUNS (not merely declares) a `channel<array<int>>`,
    /// `channel<map<string, int>>`, or `channel<number>` composite — i.e. the owned-heap/fr12
    /// widening this round adds was genuinely exercised, not just present in an uncalled
    /// helper's signature.
    pub fired_owned_heap_channel: bool,
    /// True when `take_or_make_array`/`take_or_make_map` actually REUSED a pooled binding
    /// (rather than building a fresh one) for at least one channel send — v0.3-M8 Phase 8 fix
    /// round 4, BLOCKER 1. Until this round, the reuse branch was unreachable dead code (see
    /// `take_or_make_array`'s doc comment); this counter and its floor in
    /// `per_construct_floors_hold_over_a_fixed_corpus` are what keep it from silently dying
    /// again.
    pub fired_pool_reuse: bool,
}

/// Generate one type-valid program from `seed`.
pub fn generate(seed: u64) -> Program {
    let mut b = Builder::new(seed);
    b.declare_shapes();
    b.declare_helpers();
    b.build_body();
    b.finish()
}

// ── Builder ───────────────────────────────────────────────────────────────────

struct ShapeDecl {
    name: String,
    int_field: String,
    str_field: String,
}

/// A feeder function: sends a small run of values of `kind` into a `lend channel<kind>` then
/// closes it. `body_lines` are pre-rendered (everything between the signature's `{` and the
/// trailing `wire.close()`) because a feeder function is its OWN scope — it has no access to the
/// entrypoint's `arrays`/`maps` pools, so every value it sends is a fresh local built and sent
/// immediately, never registered anywhere a later statement could read it back.
struct FeedFn {
    name: String,
    kind: ElemKind,
    body_lines: Vec<String>,
    /// How many `wire.send(...)` calls `body_lines` contains. `stmt_background_drain_loop`
    /// reads this to keep the consumer's channel capacity `>= send_count` — see the long
    /// comment there for why: a CONFIRMED, reproducible defect (backpressure-blocked
    /// `channel<number>` sends losing a value under `--no-optimize`/`--no-auto-parallel`),
    /// bisected to `send_count > capacity` specifically, general to Int too by construction
    /// (untested, not confirmed safe) so the cap floor applies to every kind.
    send_count: usize,
}

/// A shape-reading function: `(S, channel<int>) -> nothing`, sends one int derived from the
/// shape's int field. Used for the two-spawn Auto-Arc topology.
struct ReaderFn {
    name: String,
    shape: String,
    bump: i64,
}

struct Builder {
    rng: Rng,
    seed: u64,
    shapes: Vec<ShapeDecl>,
    cpu_fns: Vec<(String, i64)>, // name, constant addend
    io_fns: Vec<(String, i64)>,  // name, constant addend (suspends via `wait sleep`)
    feed_fns: Vec<FeedFn>,
    reader_fns: Vec<ReaderFn>,
    body: Vec<String>,
    ints: Vec<String>,
    arrays: Vec<(String, usize)>, // name, current length
    maps: Vec<String>,
    next_id: usize,
    stmt_count: usize,
    uses_background: bool,
    fired_inline_close: bool,
    fired_drain_loop: bool,
    fired_two_spawn_arc: bool,
    fired_map_decl: bool,
    fired_array_intrinsic: bool,
    fired_shape_field_read: bool,
    fired_owned_heap_channel: bool,
    /// True once `take_or_make_array`/`take_or_make_map` has actually reused a pooled binding —
    /// see `Program::fired_pool_reuse`.
    fired_pool_reuse: bool,
    /// v0.3-M8 Phase 8 fix round 3 — a CONFIRMED runtime defect, not a speculative avoidance (see
    /// `take_or_make_array`'s doc comment for the full repro). Set true the moment ANY statement
    /// introduces a genuine suspension point in `entrypoint`'s own frame (`wait`, a
    /// `background`-handle `.receive()`, a channel `.receive()`). Consulted ONLY by
    /// `take_or_make_array`/`take_or_make_map`, which refuse to reuse a PRE-EXISTING pooled
    /// binding for a channel send once this is true (a FRESH local, built and sent in the same
    /// composite with no suspension in between, is always safe and unaffected).
    suspension_seen: bool,
}

const MAP_KEYS: [&str; 3] = ["alice", "bob", "cam"];

impl Builder {
    fn new(seed: u64) -> Self {
        Builder {
            rng: Rng::new(seed),
            seed,
            shapes: Vec::new(),
            cpu_fns: Vec::new(),
            io_fns: Vec::new(),
            feed_fns: Vec::new(),
            reader_fns: Vec::new(),
            body: Vec::new(),
            ints: Vec::new(),
            arrays: Vec::new(),
            maps: Vec::new(),
            next_id: 0,
            stmt_count: 0,
            uses_background: false,
            fired_inline_close: false,
            fired_drain_loop: false,
            fired_two_spawn_arc: false,
            fired_map_decl: false,
            fired_array_intrinsic: false,
            fired_shape_field_read: false,
            fired_owned_heap_channel: false,
            fired_pool_reuse: false,
            suspension_seen: false,
        }
    }

    fn fresh(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{prefix}{}", self.next_id)
    }

    fn push(&mut self, line: String) {
        self.body.push(format!("  {line}"));
    }

    // ── Declarations ──────────────────────────────────────────────────────────

    fn declare_shapes(&mut self) {
        // Always at least one shape. The pre-widening draw (`below(3)`, i.e. 0..=2) left ~1/3 of
        // the corpus with ZERO shapes — and two whole menu arms (9, the shape-field-read
        // statement; 13, the two-spawn Auto-Arc topology) silently fall through to
        // `stmt_int_let` whenever `shapes` is empty. A shapeless program isn't a distinct code
        // path anywhere else in this grammar, so that 1/3 was starving both arms for no
        // compensating coverage value. 1..=2 keeps the "how many shapes" variety without ever
        // zeroing it out.
        let n = 1 + self.rng.below(2);
        for i in 0..n {
            self.shapes.push(ShapeDecl {
                name: format!("Crate{i}"),
                int_field: "weight".to_string(),
                str_field: "label".to_string(),
            });
        }
    }

    fn declare_helpers(&mut self) {
        for i in 0..1 + self.rng.below(2) {
            self.cpu_fns
                .push((format!("tally{i}"), self.rng.range(1, 9)));
        }
        for i in 0..1 + self.rng.below(2) {
            self.io_fns
                .push((format!("fetch{i}"), self.rng.range(1, 9)));
        }
        for i in 0..1 + self.rng.below(2) {
            let kind = *self.rng.pick(&ElemKind::ALL);
            let (body_lines, send_count) = self.build_feed_body(kind);
            self.feed_fns.push(FeedFn {
                name: format!("feed{i}"),
                kind,
                body_lines,
                send_count,
            });
        }
        // A reader per declared shape (needs a shape to read).
        for (i, s) in self.shapes.iter().enumerate() {
            self.reader_fns.push(ReaderFn {
                name: format!("weigh{i}"),
                shape: s.name.clone(),
                bump: 0,
            });
        }
        for i in 0..self.reader_fns.len() {
            self.reader_fns[i].bump = self.rng.range(1, 9);
        }
    }

    /// Build a feeder function's body (everything before the always-present `wire.close()`):
    /// 1-3 values of `kind`, each sent the moment it exists. Every binding here is LOCAL to this
    /// function and consumed by its own send — there is no cross-statement pool to bookkeep
    /// inside a fresh function scope. Returns the body lines plus the number of `wire.send(...)`
    /// calls emitted, so the caller (`stmt_background_drain_loop`) can size the consumer's
    /// channel capacity to never fall below it — see that method's doc comment for the confirmed
    /// defect this avoids.
    fn build_feed_body(&mut self, kind: ElemKind) -> (Vec<String>, usize) {
        let count = 1 + self.rng.below(3);
        let lines = match kind {
            ElemKind::Int => (0..count)
                .map(|_| format!("wire.send({})", self.rng.range(1, 12)))
                .collect(),
            ElemKind::Number => {
                // `number` is copy-through (fr12) — the SAME binding survives repeated sends,
                // which is the property this composite exists to exercise, not merely "some
                // number values got sent."
                let v = self.fresh("price");
                let whole = self.rng.range(0, 9);
                let frac = self.rng.range(0, 9);
                let mut lines = vec![format!("let {v}: number = {whole}.{frac}")];
                for _ in 0..count {
                    lines.push(format!("wire.send({v})"));
                }
                lines
            }
            ElemKind::Array => {
                let mut lines = Vec::new();
                for _ in 0..count {
                    let (a, line, _n) = self.fresh_array_literal();
                    lines.push(line);
                    lines.push(format!("wire.send({a})"));
                }
                lines
            }
            ElemKind::Map => {
                let mut lines = Vec::new();
                for _ in 0..count {
                    let (m, line) = self.fresh_map_literal();
                    lines.push(line);
                    lines.push(format!("wire.send({m})"));
                }
                lines
            }
        };
        (lines, count)
    }

    // ── Expressions ───────────────────────────────────────────────────────────

    /// A small non-negative int literal. Kept in `0..10` so a `*` never overflows and a
    /// value divergence stays legible in a failure report.
    fn int_lit(&mut self) -> String {
        self.rng.range(0, 10).to_string()
    }

    /// An int-typed expression. Depth is capped at one binary operation, and `*` is only
    /// emitted between two LITERALS — a var-var product would compound across statements and
    /// eventually overflow `int`, which is a compile error this generator has no business
    /// producing.
    fn int_expr(&mut self) -> String {
        let has_var = !self.ints.is_empty();
        match self.rng.below(if has_var { 5 } else { 3 }) {
            0 => self.int_lit(),
            1 => {
                let a = self.int_lit();
                let b = self.int_lit();
                format!("{a} * {b}")
            }
            2 => {
                let a = self.int_lit();
                let b = self.int_lit();
                let op = if self.rng.one_in(2) { "+" } else { "-" };
                format!("{a} {op} {b}")
            }
            3 => self.rng.pick(&self.ints).clone(),
            _ => {
                let v = self.rng.pick(&self.ints).clone();
                let b = self.int_lit();
                let op = if self.rng.one_in(2) { "+" } else { "-" };
                format!("{v} {op} {b}")
            }
        }
    }

    // ── Body ──────────────────────────────────────────────────────────────────

    fn build_body(&mut self) {
        // Seed the environment so the first statements always have something to draw from.
        let v = self.fresh("v");
        let e = self.int_lit();
        self.push(format!("let {v} = {e}"));
        self.ints.push(v);
        self.stmt_count += 1;

        let target = 9 + self.rng.below(10); // 9..=18 statements after the seed
        for _ in 0..target {
            self.emit_statement();
            self.stmt_count += 1;
        }
    }

    fn emit_statement(&mut self) {
        // Weighted menu. Each arm re-checks its own preconditions and falls through to a
        // pure-int statement when the environment cannot support it, so no draw is wasted.
        match self.rng.below(14) {
            0 => self.stmt_int_let(),
            1 => self.stmt_print_int(),
            2 => self.stmt_print_interpolated(),
            3 => self.stmt_cpu_call(),
            4 => self.stmt_wait_io_call(),
            5 => self.stmt_array_decl(),
            6 => self.stmt_array_op(),
            7 => self.stmt_map_decl(),
            8 => self.stmt_map_op(),
            9 => self.stmt_shape_decl(),
            10 => self.stmt_channel_inline(),
            11 => self.stmt_background_handle(),
            12 => self.stmt_background_drain_loop(),
            _ => self.stmt_background_two_spawn_shape(),
        }
    }

    fn stmt_int_let(&mut self) {
        let v = self.fresh("v");
        let e = self.int_expr();
        self.push(format!("let {v} = {e}"));
        self.ints.push(v);
    }

    fn stmt_print_int(&mut self) {
        if self.ints.is_empty() {
            return self.stmt_int_let();
        }
        let v = self.rng.pick(&self.ints).clone();
        self.push(format!("print({v}.toString())"));
    }

    fn stmt_print_interpolated(&mut self) {
        if self.ints.is_empty() {
            return self.stmt_int_let();
        }
        let v = self.rng.pick(&self.ints).clone();
        self.push(format!("print(`{v} is ${{{v}.toString()}}`)"));
    }

    fn stmt_cpu_call(&mut self) {
        if self.cpu_fns.is_empty() {
            return self.stmt_int_let();
        }
        let f = self.rng.pick(&self.cpu_fns).0.clone();
        let a = self.int_expr();
        let b = self.int_lit();
        let v = self.fresh("v");
        self.push(format!("let {v} = {f}({a}, {b})"));
        self.ints.push(v);
    }

    fn stmt_wait_io_call(&mut self) {
        if self.io_fns.is_empty() {
            return self.stmt_int_let();
        }
        let f = self.rng.pick(&self.io_fns).0.clone();
        let a = self.int_lit();
        let v = self.fresh("v");
        // ALWAYS `wait`-prefixed. An un-prefixed adjacent pair of suspending calls is an
        // auto-parallel I/O group whose members may finish in either order; that is a
        // documented Model-A intended reorder (`IMP-concurrency.md`), not a miscompile, and
        // this generator has no business manufacturing one for the oracle to flag.
        self.push(format!("let {v} = wait {f}({a})"));
        // A genuine suspension point in `entrypoint`'s own frame — see `take_or_make_array`'s
        // doc comment.
        self.suspension_seen = true;
        self.ints.push(v);
    }

    fn stmt_array_decl(&mut self) {
        let (a, line, n) = self.fresh_array_literal();
        self.push(line);
        self.arrays.push((a, n));
    }

    fn stmt_array_op(&mut self) {
        if self.arrays.is_empty() {
            return self.stmt_array_decl();
        }
        self.fired_array_intrinsic = true;
        let idx = self.rng.below(self.arrays.len());
        let name = self.arrays[idx].0.clone();
        let len = self.arrays[idx].1;
        match self.rng.below(3) {
            0 => {
                let e = self.int_lit();
                self.push(format!("{name}.add({e})"));
                self.arrays[idx].1 = len + 1;
            }
            1 => {
                let v = self.fresh("v");
                self.push(format!("let {v} = {name}.count()"));
                self.ints.push(v);
            }
            _ => {
                // In-range by construction; `.or(-1)` is still required because indexing
                // yields `maybe<int>`.
                let i = self.rng.below(len);
                let v = self.fresh("v");
                self.push(format!("let {v} = {name}[{i}].or(-1)"));
                self.ints.push(v);
            }
        }
    }

    fn stmt_map_decl(&mut self) {
        self.fired_map_decl = true;
        let (m, line) = self.fresh_map_literal();
        self.push(line);
        self.maps.push(m);
    }

    fn stmt_map_op(&mut self) {
        if self.maps.is_empty() {
            return self.stmt_map_decl();
        }
        let m = self.rng.pick(&self.maps).clone();
        let k = (*self.rng.pick(&MAP_KEYS)).to_string();
        match self.rng.below(3) {
            0 => {
                let v = self.fresh("v");
                // A key that may or may not be present — `.or(0)` is total either way, and the
                // ANSWER is deterministic, which is all the oracle compares.
                self.push(format!("let {v} = {m}[`{k}`].or(0)"));
                self.ints.push(v);
            }
            1 => {
                let e = self.int_lit();
                self.push(format!("{m}[`{k}`] = {e}"));
            }
            _ => {
                let v = self.fresh("v");
                self.push(format!("let {v} = {m}.count()"));
                self.ints.push(v);
            }
        }
    }

    fn stmt_shape_decl(&mut self) {
        if self.shapes.is_empty() {
            return self.stmt_int_let();
        }
        self.fired_shape_field_read = true;
        let i = self.rng.below(self.shapes.len());
        let name = self.shapes[i].name.clone();
        let int_field = self.shapes[i].int_field.clone();
        let str_field = self.shapes[i].str_field.clone();
        let s = self.fresh("crate");
        let w = self.int_lit();
        self.push(format!(
            "let {s}: {name} = {{ {int_field}: {w}, {str_field}: `pierogi` }}"
        ));
        // Bind a field read so the shape actually feeds the int environment.
        let v = self.fresh("v");
        self.push(format!("let {v} = {s}.{int_field}"));
        self.ints.push(v);
    }

    /// An inline (no `background`) channel: construct, send, receive. Fully sequential and
    /// therefore byte-order-strict under the oracle. Element kind is drawn from the full
    /// `{int, array<int>, map<string,int>, number}` space (v0.3-M8 Phase 8 fix round 3). Every
    /// element kind fires unconditionally here — the hazard this widening found (see
    /// `take_or_make_array`/`take_or_make_map`'s doc comment) is narrower than "this composite,"
    /// and is contained at the one place it can actually occur: reusing a PRE-EXISTING pooled
    /// binding for a send.
    fn stmt_channel_inline(&mut self) {
        let kind = *self.rng.pick(&ElemKind::ALL);
        self.stmt_channel_inline_kind(kind);
    }

    fn stmt_channel_inline_kind(&mut self, kind: ElemKind) {
        if kind != ElemKind::Int {
            self.fired_owned_heap_channel = true;
        }
        let c = self.fresh("wire");
        let cap = 1 + self.rng.below(4);
        let chan_ty = kind.channel_type();
        self.push(format!("let {c}: {chan_ty} = {chan_ty}({cap})"));

        match kind {
            ElemKind::Int => {
                let sends = 1 + self.rng.below(cap.min(3));
                for _ in 0..sends {
                    let e = self.int_lit();
                    self.push(format!("{c}.send({e})"));
                }
                for _ in 0..sends {
                    let m = self.fresh("got");
                    let v = self.fresh("v");
                    self.push(format!("let {m} = {c}.receive()"));
                    self.push(format!("let {v} = {m}.or(0)"));
                    self.ints.push(v);
                }
            }
            ElemKind::Number => {
                // Same copy-through property as the feeder: one binding, sent more than once,
                // and STILL readable afterward — the fr12 "number never joins the give set"
                // contract, exercised inline rather than across a task boundary.
                let v = self.fresh("price");
                let whole = self.rng.range(0, 9);
                let frac = self.rng.range(0, 9);
                self.push(format!("let {v}: number = {whole}.{frac}"));
                let sends = 1 + self.rng.below(cap.min(2));
                for _ in 0..sends {
                    self.push(format!("{c}.send({v})"));
                }
                self.emit_receive_prints(&c, sends, |got| format!("{got}.value.toString()"));
                // Prove copy-through: `v` is still usable after being sent — a compile error
                // here (`ConsumedBySend`) would mean `number` wrongly joined the give set.
                self.push(format!("print({v}.toString())"));
            }
            ElemKind::Array => {
                let sends = 1 + self.rng.below(cap.min(2));
                for _ in 0..sends {
                    let name = self.take_or_make_array();
                    self.push(format!("{c}.send({name})"));
                }
                self.emit_receive_prints(&c, sends, |got| {
                    format!("{got}.value.count().toString()")
                });
            }
            ElemKind::Map => {
                let sends = 1 + self.rng.below(cap.min(2));
                for _ in 0..sends {
                    let name = self.take_or_make_map();
                    self.push(format!("{c}.send({name})"));
                }
                self.emit_receive_prints(&c, sends, |got| {
                    format!("{got}.value.count().toString()")
                });
            }
        }

        // Every receive above (Number/Array/Map's `emit_receive_prints`, Int's own inline
        // receive loop) is a genuine suspension point in `entrypoint`'s own frame. Set AFTER the
        // composite's sends and receives complete, not before: setting it before the composite's
        // OWN body ran made `take_or_make_array`/`take_or_make_map`'s reuse branch permanently
        // unreachable (this exact statement is the ONLY caller of either), because their `!self.
        // suspension_seen` guard was already false by the time the Array/Map arm above called
        // them. Placing the assignment here means THIS composite's own sends still see whatever
        // `suspension_seen` was BEFORE this statement ran (correct — reuse is fine for a pool
        // entry that predates any suspension), while any LATER composite correctly sees this
        // composite's own receives as a suspension that happened first.
        self.suspension_seen = true;

        // Close, then one receive past end-of-stream: the v0.3-M8 Phase 4 contract says the
        // drained-and-closed channel answers `none`, never a hang and never an error. Same
        // contract for every element kind.
        if self.rng.one_in(2) {
            self.fired_inline_close = true;
            self.push(format!("{c}.close()"));
            let m = self.fresh("got");
            self.push(format!("let {m} = {c}.receive()"));
            self.push(format!("if ({m}.exists()) {{"));
            self.push("  print(`unexpected value after end of stream`)".to_string());
            self.push("}".to_string());
        }
    }

    /// Emit `sends` `receive()`-then-print-if-present blocks against channel `c` — the shared
    /// tail of `stmt_channel_inline_kind`'s Number/Array/Map arms (v0.3-M8 Phase 8 fix round 4,
    /// should-fix 5: these three were near-identical five-line blocks, differing only in what
    /// gets printed). `render` turns the bound `got` name into the printed expression:
    /// `number`/`int` print the value directly (`.value.toString()`); `array`/`map` print
    /// `.count()` instead of the payload itself.
    fn emit_receive_prints(&mut self, c: &str, sends: usize, render: impl Fn(&str) -> String) {
        for _ in 0..sends {
            let got = self.fresh("got");
            self.push(format!("let {got} = {c}.receive()"));
            self.push(format!("if ({got}.exists()) {{"));
            self.push(format!("  print({})", render(&got)));
            self.push("}".to_string());
        }
    }

    /// Build one `array<int>` literal: a fresh binding name, its `let NAME: array<int> = [...]`
    /// source line, and the element count (needed by `stmt_array_decl`, which tracks pool
    /// lengths in `self.arrays` — the other two callers discard it). v0.3-M8 Phase 8 fix round 4,
    /// should-fix 5: this was three separately-maintained copies (`build_feed_body`'s Array arm,
    /// `stmt_array_decl`, and this function's own fresh-branch), and they had already drifted —
    /// `build_feed_body`'s copy drew `1 + below(3)` while the other two drew `1 + below(4)`. One
    /// source now; the element-count draw below is the one every caller gets.
    fn fresh_array_literal(&mut self) -> (String, String, usize) {
        let a = self.fresh("rows");
        let n = 1 + self.rng.below(4);
        let elems: Vec<String> = (0..n).map(|_| self.int_lit()).collect();
        let line = format!("let {a}: array<int> = [{}]", elems.join(", "));
        (a, line, n)
    }

    /// The `map<string, int>` sibling of `fresh_array_literal` — one source for the three
    /// previously-separate copies (`build_feed_body`'s Map arm, `stmt_map_decl`,
    /// `take_or_make_map`'s fresh-branch). These three had not drifted from each other (all three
    /// already drew `1 + below(MAP_KEYS.len())`), but leaving three copies standing is exactly
    /// the shape that let the array trio drift silently.
    fn fresh_map_literal(&mut self) -> (String, String) {
        let m = self.fresh("book");
        let n = 1 + self.rng.below(MAP_KEYS.len());
        let mut entries = Vec::new();
        for k in MAP_KEYS.iter().take(n) {
            let v = self.int_lit();
            entries.push(format!("`{k}`: {v}"));
        }
        let line = format!("let {m}: map<string, int> = {{ {} }}", entries.join(", "));
        (m, line)
    }

    /// When `suspension_seen` is false (no suspension — `wait`, a `background`-handle
    /// `.receive()`, or a channel `.receive()` — has happened yet in `entrypoint`'s frame) and
    /// the pool has at least one entry, a coin flip (`one_in(2)`) decides whether to REUSE an
    /// array ALREADY declared earlier in the body instead of building a fresh one. Once
    /// `suspension_seen` is true, reuse is refused unconditionally and every array sent here is
    /// fresh. Same real bookkeeping either way: a pooled binding must leave `self.arrays` the
    /// moment it is chosen to be sent, or a later `stmt_array_op` draw would read it back and the
    /// compiler would correctly refuse the program with `ConsumedBySend`.
    ///
    /// The `suspension_seen` gate is not speculative: it is the direct product of a reproducible
    /// finding this same fix round made BY doing the widening. A generated program (seed 3 of
    /// the widened corpus) crashed with `RUNTIME ERROR: killed by signal 6 (SIGABRT)` —
    /// `crates/ynz-runtime/src/lib.rs:1058`/`:1411`, a null/misaligned pointer dereference in
    /// `ynz_map_count`/`ynz_array_count`. Bisected to a minimal, deterministic repro
    /// (`.scratch-repro/min21` vs `min20` in this fix round's session — not committed, see the
    /// audit entry for the exact source): a plain `array<int>`/`map<string,int>` LOCAL declared
    /// BEFORE any suspension point (`wait`, a `background`-handle `.receive()`, a channel
    /// `.receive()`) in the SAME function, later `.send()`-ed into a channel AFTER that
    /// suspension, corrupts on read. Declaring AFTER the suspension and sending immediately is
    /// safe (confirmed both orders directly); crossing a suspension while merely READ (never
    /// sent into a channel) was not implicated — this is specific to the channel-transfer path.
    /// This is a genuine runtime defect — not a generator/grammar bug — and per this plan's CCIR
    /// item 5 (R5) it is NOT fixed inline here. See Future Requirements #11 (`plan.md`) and the
    /// v0.3-M8 plan's `audit.md`, FRAGO 015, for the full repro and the tracked follow-up. Gating
    /// ONLY the reuse-from-pool branch (not the whole element kind, and not the whole composite)
    /// keeps the fuzz corpus green while preserving the widening's actual value: a FRESH local
    /// built and sent immediately never crosses a suspension by construction, so it fires
    /// regardless of what happened earlier in the program.
    ///
    /// v0.3-M8 Phase 8 fix round 4, BLOCKER 1: until this round, `suspension_seen` was set at the
    /// TOP of `stmt_channel_inline_kind` — the ONLY caller of this function — so by the time this
    /// function's `!self.suspension_seen` check ran, the flag this SAME statement had just set
    /// was already true. The reuse branch below was dead code; every generated program built a
    /// fresh array here regardless of the coin flip. Confirmed both ways: replacing the reuse
    /// branch's body with `panic!` and generating 8,192 programs never fired it; with the flag's
    /// assignment moved to `stmt_channel_inline_kind`'s end (see that function), the same probe
    /// fires on the first seeds. `per_construct_floors_hold_over_a_fixed_corpus` now floors
    /// `fired_pool_reuse` so this cannot silently die again without a red test.
    fn take_or_make_array(&mut self) -> String {
        if !self.suspension_seen && !self.arrays.is_empty() && self.rng.one_in(2) {
            self.fired_pool_reuse = true;
            let idx = self.rng.below(self.arrays.len());
            self.arrays.remove(idx).0
        } else {
            let (a, line, _n) = self.fresh_array_literal();
            self.push(line);
            a
        }
    }

    /// The `map<string, int>` sibling of `take_or_make_array` — same reuse-or-fresh choice, same
    /// `suspension_seen` gate, same pool-removal-on-send bookkeeping, same BLOCKER-1 history (see
    /// that function's doc comment).
    fn take_or_make_map(&mut self) -> String {
        if !self.suspension_seen && !self.maps.is_empty() && self.rng.one_in(2) {
            self.fired_pool_reuse = true;
            let idx = self.rng.below(self.maps.len());
            self.maps.remove(idx)
        } else {
            let (m, line) = self.fresh_map_literal();
            self.push(line);
            m
        }
    }

    /// Handle form: `let h = background fetchN(k)`, then a BLOCKING `h.receive()`.
    /// `ynz_handle_recv_poll` parks until the task publishes its result, so the value is
    /// deterministic without a timing margin.
    fn stmt_background_handle(&mut self) {
        if self.io_fns.is_empty() {
            return self.stmt_int_let();
        }
        let f = self.rng.pick(&self.io_fns).0.clone();
        let a = self.int_lit();
        let h = self.fresh("h");
        let r = self.fresh("got");
        let v = self.fresh("v");
        self.push(format!("let {h} = background {f}({a})"));
        self.push(format!("let {r} = {h}.receive()"));
        // `.receive()` is a genuine suspension point in `entrypoint`'s own frame — see
        // `take_or_make_array`'s doc comment for why later composites need to know about it.
        self.suspension_seen = true;
        self.push(format!("let {v} = {r}.or(0)"));
        self.ints.push(v);
        self.uses_background = true;
    }

    /// The taught end-of-stream idiom: a `background` producer sends then closes; the consumer
    /// drains with `.exists()`/`.value` until the stream ends. `total` accumulates the printed
    /// count deterministically regardless of interleaving. The producer's element kind is drawn
    /// from the full `{int, array<int>, map<string,int>, number}` space, UNRESTRICTED — a
    /// feeder's array/map/number locals live entirely inside the FEEDER's own function, which
    /// never suspends internally (no `wait` call in `build_feed_body`), so they cannot cross a
    /// suspension the way an `entrypoint`-local reused by `take_or_make_array`/`take_or_make_map`
    /// can. The drain arithmetic switches on the picked kind (an owned-heap payload contributes
    /// its `.count()`, not its bytes; `number` contributes itself, exactly like `int`).
    fn stmt_background_drain_loop(&mut self) {
        if self.feed_fns.is_empty() {
            return self.stmt_int_let();
        }
        self.fired_drain_loop = true;
        let i = self.rng.below(self.feed_fns.len());
        let kind = self.feed_fns[i].kind;
        if kind != ElemKind::Int {
            self.fired_owned_heap_channel = true;
        }
        // The `while`/`.exists()` loop below calls `.receive()` — a genuine suspension point in
        // `entrypoint`'s own frame. See `take_or_make_array`'s doc comment.
        self.suspension_seen = true;
        let f = self.feed_fns[i].name.clone();
        let c = self.fresh("wire");
        let total = self.fresh("total");
        let open = self.fresh("open");
        let nx = self.fresh("nx");
        // CONFIRMED, reproducible defect avoided: a `background` producer sending more values
        // into a channel than its capacity (`send_count > capacity`, forcing the producer to
        // actually BLOCK on a full buffer) can read back a HEAP ADDRESS where a sent value
        // belongs — an uninitialized-or-freed read, not an arithmetic shortfall, non-
        // deterministic (~17-30% of runs across a fix round's own measurement), and general to
        // BOTH `int` and `number` (v0.3-M8 Phase 8 fix round 4 corrected all three of these
        // against the round-3 record, which claimed `number`-only, deterministic, "loses one
        // value" — none of the three held up). This is a genuine runtime defect in the blocked-
        // send path (`send_count > capacity`), not a generator bug and not `number_to_heap_cell`
        // specifically, and per this plan's CCIR item 5 (R5) it is not fixed inline here — see
        // Future Requirements #11 (`plan.md`) and the v0.3-M8 plan's `audit.md`, FRAGO 015. The
        // floor below applies to every kind (never let this composite's producer block) because
        // the defect is general to the blocked-send path, not one element kind.
        let cap = self.feed_fns[i].send_count.max(1 + self.rng.below(4));
        let chan_ty = kind.channel_type();
        self.push(format!("let {c}: {chan_ty} = {chan_ty}({cap})"));
        self.push(format!("background {f}({c})"));
        let total_init = if kind == ElemKind::Number { "0.0" } else { "0" };
        self.push(format!("let {total} = {total_init}"));
        self.push(format!("let {open} = true"));
        self.push(format!("while ({open}) {{"));
        self.push(format!("  let {nx} = {c}.receive()"));
        self.push(format!("  if ({nx}.exists()) {{"));
        let addend = match kind {
            ElemKind::Int | ElemKind::Number => format!("{nx}.value"),
            ElemKind::Array | ElemKind::Map => format!("{nx}.value.count()"),
        };
        self.push(format!("    {total} = {total} + {addend}"));
        self.push("  }".to_string());
        self.push(format!("  {open} = {nx}.exists()"));
        self.push("}".to_string());
        self.push(format!("print({total}.toString())"));
        // `total` is `number`-typed when the feeder is `number`-kinded — do NOT register it
        // into `self.ints` (an int-typed pool) or a later `int_expr()` draw would emit a
        // number-plus-int expression the compiler correctly refuses.
        if kind != ElemKind::Number {
            self.ints.push(total);
        }
        self.uses_background = true;
    }

    /// Two `background` spawns handing the SAME read-only shape binding to the same reader,
    /// with nothing writing it in between — the Auto-Arc sharing topology this milestone's
    /// Phase 5 emits for. Each task sends one int; main drains exactly two.
    fn stmt_background_two_spawn_shape(&mut self) {
        if self.reader_fns.is_empty() {
            return self.stmt_int_let();
        }
        self.fired_two_spawn_arc = true;
        let i = self.rng.below(self.reader_fns.len());
        let reader = self.reader_fns[i].name.clone();
        let shape = self.reader_fns[i].shape.clone();
        let sd = self
            .shapes
            .iter()
            .find(|s| s.name == shape)
            .expect("reader references a declared shape");
        let int_field = sd.int_field.clone();
        let str_field = sd.str_field.clone();

        let s = self.fresh("crate");
        let c = self.fresh("wire");
        let w = self.int_lit();
        self.push(format!(
            "let {s}: {shape} = {{ {int_field}: {w}, {str_field}: `primanti` }}"
        ));
        self.push(format!("let {c}: channel<int> = channel<int>(4)"));
        self.push(format!("background {reader}({s}, {c})"));
        self.push(format!("background {reader}({s}, {c})"));
        let total = self.fresh("total");
        self.push(format!("let {total} = 0"));
        for _ in 0..2 {
            let m = self.fresh("got");
            self.push(format!("let {m} = {c}.receive()"));
            // A genuine suspension point in `entrypoint`'s own frame — see
            // `take_or_make_array`'s doc comment.
            self.suspension_seen = true;
            self.push(format!("if ({m}.exists()) {{"));
            self.push(format!("  {total} = {total} + {m}.value"));
            self.push("}".to_string());
        }
        self.push(format!("print({total}.toString())"));
        // The caller keeps its own view of the shape after the spawns — the read that the
        // Arc topology must not disturb.
        let v = self.fresh("v");
        self.push(format!("let {v} = {s}.{int_field}"));
        self.push(format!("print({v}.toString())"));
        self.ints.push(total);
        self.ints.push(v);
        self.uses_background = true;
    }

    // ── Assembly ──────────────────────────────────────────────────────────────

    fn finish(mut self) -> Program {
        let mut out = String::new();
        out.push_str(&format!(
            "// GENERATED by crates/ynz-driver/tests/fuzz_grammar (v0.3-M8 Phase 8).\n\
             // seed = {}. Regenerate with `fuzz_grammar::generate({})`.\n\
             // Do not hand-edit: this file is an oracle input, not a fixture.\n\n",
            self.seed, self.seed
        ));

        for s in &self.shapes {
            out.push_str(&format!(
                "shape {} {{\n  {}: int\n  {}: string\n}}\n\n",
                s.name, s.int_field, s.str_field
            ));
        }
        for (name, k) in &self.cpu_fns {
            out.push_str(&format!(
                "function {name}(a: int, b: int) -> int {{\n  return a + b + {k}\n}}\n\n"
            ));
        }
        for (name, k) in &self.io_fns {
            out.push_str(&format!(
                "function {name}(n: int) -> int {{\n  wait sleep(1)\n  return n + {k}\n}}\n\n"
            ));
        }
        let io_fn_names: Vec<String> = self.io_fns.iter().map(|(n, _)| n.clone()).collect();
        for f in &self.feed_fns {
            out.push_str(&format!(
                "function {}(lend wire: {}) -> nothing {{\n",
                f.name,
                f.kind.channel_type()
            ));
            for l in &f.body_lines {
                out.push_str(&format!("  {l}\n"));
            }
            out.push_str("  wire.close()\n}\n\n");
        }
        for r in &self.reader_fns {
            let sd = self
                .shapes
                .iter()
                .find(|s| s.name == r.shape)
                .expect("reader references a declared shape");
            out.push_str(&format!(
                "function {}(item: {}, out: channel<int>) -> nothing {{\n  out.send(item.{} + {})\n}}\n\n",
                r.name, r.shape, sd.int_field, r.bump
            ));
        }

        out.push_str("function entrypoint() -> nothing {\n");
        for line in &self.body {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("}\n");

        // Prune helpers the body never called? No — an uncalled helper is itself coverage
        // (it still lexes, parses, typechecks, and lowers), and pruning would make the
        // program a function of the body draw rather than of the seed.
        let uses_background = self.uses_background;
        let seed = self.seed;
        let stmt_count = self.stmt_count;
        let fired_inline_close = self.fired_inline_close;
        let fired_drain_loop = self.fired_drain_loop;
        let fired_two_spawn_arc = self.fired_two_spawn_arc;
        let fired_map_decl = self.fired_map_decl;
        let fired_array_intrinsic = self.fired_array_intrinsic;
        let fired_shape_field_read = self.fired_shape_field_read;
        let fired_owned_heap_channel = self.fired_owned_heap_channel;
        let fired_pool_reuse = self.fired_pool_reuse;
        self.body.clear();
        Program {
            source: out,
            seed,
            uses_background,
            stmt_count,
            io_fn_names,
            fired_inline_close,
            fired_drain_loop,
            fired_two_spawn_arc,
            fired_map_decl,
            fired_array_intrinsic,
            fired_shape_field_read,
            fired_owned_heap_channel,
            fired_pool_reuse,
        }
    }
}

/// Distinct-source count of a seed range — the anti-triviality check the STOP-condition needs
/// (a hit rate propped up by re-emitting one program is not a hit rate).
pub fn distinct_sources(seeds: impl IntoIterator<Item = u64>) -> usize {
    let set: BTreeSet<String> = seeds.into_iter().map(|s| generate(s).source).collect();
    set.len()
}

// ── Generator contract tests ──────────────────────────────────────────────────
//
// These lock the properties the harness's SAFETY depends on. They are pure string/structure
// checks — no compiler invocation — so they run in milliseconds and fail loudly the moment the
// grammar drifts into a shape the oracle cannot interpret.

#[cfg(test)]
mod generator_contract {
    use super::*;

    /// The replay entry point, documented in `README.md`: reproduce and READ any program the
    /// sweep reported, from its seed alone.
    ///
    ///   YNZ_FUZZ_SEED=1003 cargo test -p ynz-driver --test cross_impl_consistency \
    ///     print_generated_program -- --ignored --nocapture
    #[test]
    #[ignore = "developer replay tool — prints one generated program, asserts nothing"]
    fn print_generated_program() {
        let seed: u64 = std::env::var("YNZ_FUZZ_SEED")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let p = generate(seed);
        println!(
            "// seed {seed} — {} statements, background = {}\n{}",
            p.stmt_count, p.uses_background, p.source
        );
    }

    #[test]
    fn generation_is_deterministic_from_the_seed() {
        for seed in [1u64, 7, 4242, u64::MAX] {
            assert_eq!(
                generate(seed).source,
                generate(seed).source,
                "seed {seed} produced two different programs — the replay key is worthless"
            );
        }
    }

    #[test]
    fn a_seed_range_produces_many_distinct_programs() {
        // The STOP-condition's anti-triviality half: a generator that emits one program with a
        // 100% compile rate has proven nothing.
        let n = distinct_sources(0..128u64);
        assert!(
            n >= 120,
            "only {n}/128 distinct programs — the grammar collapsed onto a narrow shape"
        );
    }

    #[test]
    fn every_program_declares_an_entrypoint_and_a_non_trivial_body() {
        for seed in 0..64u64 {
            let p = generate(seed);
            assert!(
                p.source.contains("function entrypoint() -> nothing {"),
                "seed {seed} emitted no entrypoint"
            );
            // `build_body` GUARANTEES `stmt_count` is `10..=19` (the seed `let` plus a
            // `9 + below(10)` draw) — this asserts the real invariant, not a line count that
            // happened to always be large regardless of the generator's actual statement mix.
            assert!(
                (10..=19).contains(&p.stmt_count),
                "seed {seed} emitted {} statements, outside the guaranteed 10..=19 range",
                p.stmt_count
            );
        }
    }

    #[test]
    fn uses_background_agrees_with_the_source_text() {
        // The oracle classifies scheduler-dependence from the SOURCE
        // (`output_order_is_scheduler_dependent`). `Program::uses_background` is a reporting
        // convenience and must never become a second, drifting derivation of the same
        // question (`authoritative-derivation.md`).
        for seed in 0..256u64 {
            let p = generate(seed);
            assert_eq!(
                p.uses_background,
                p.source.contains("background"),
                "seed {seed}: reported flag disagrees with the authoritative source text"
            );
        }
    }

    #[test]
    fn a_meaningful_share_of_programs_exercise_concurrency() {
        let concurrent = (0..256u64).filter(|s| generate(*s).uses_background).count();
        assert!(
            concurrent >= 128,
            "only {concurrent}/256 programs spawn background work — this harness exists for the \
             concurrent surface"
        );
    }

    #[test]
    fn suspending_calls_in_the_body_are_always_wait_prefixed() {
        // An un-prefixed adjacent pair would form an auto-parallel I/O group whose member
        // finish order legitimately differs between modes (the M3b Model-A intended reorder).
        // The oracle would flag that as a divergence; it is not one. `io_fn_names` is read from
        // the PROGRAM this exact seed produced — not a hardcoded guess that happens to be
        // complete only by coincidence with `declare_helpers`' `1 + below(2)` draw.
        for seed in 0..128u64 {
            let p = generate(seed);
            for line in p.source.lines() {
                let t = line.trim();
                for name in &p.io_fn_names {
                    let call = format!("{name}(");
                    if t.contains(&call) && !t.starts_with("function ") {
                        assert!(
                            t.contains(&format!("wait {call}"))
                                || t.contains(&format!("background {call}")),
                            "seed {seed}: bare suspending call `{t}`"
                        );
                    }
                }
            }
        }
    }

    /// Per-construct corpus floors (v0.3-M8 Phase 8 fix round 3, should-fix 2). The
    /// pre-existing anti-triviality asserts (`a_seed_range_produces_many_distinct_programs`,
    /// `a_meaningful_share_of_programs_exercise_concurrency`, and the two above) measure
    /// distinctness and the substring `background` — never WHICH menu arm fired. Proven by the
    /// gate-check this round answers: deleting `emit_statement`'s arms 12 and 13 (the taught
    /// drain-loop idiom and the two-spawn Auto-Arc topology — the two arms Phases 4 and 5 exist
    /// for) left every existing assertion green. These floors bind to the SAME counters the
    /// composites themselves set (`Program::fired_*`), not to a source-text substring search —
    /// a substring search over the FULL source is unsound here regardless (an unconditionally
    /// emitted, uncalled `feedN` helper always contains `.close()` in ITS OWN body, so a
    /// whole-source substring count for "close() in body" would read close to 100% no matter
    /// what the entrypoint's own statements did).
    ///
    /// Floors are set comfortably below the values measured over this exact 256-seed corpus on
    /// the widened generator (see the fix-round dispatch record for the measured numbers) — a
    /// starved or deleted arm collapses its share toward 0%, which trips the floor long before
    /// ordinary RNG variance would.
    #[test]
    fn per_construct_floors_hold_over_a_fixed_corpus() {
        let n = 256usize;
        let mut inline_close = 0usize;
        let mut drain_loop = 0usize;
        let mut two_spawn_arc = 0usize;
        let mut map_decl = 0usize;
        let mut array_intrinsic = 0usize;
        let mut shape_field_read = 0usize;
        let mut owned_heap_channel = 0usize;
        let mut pool_reuse = 0usize;

        for seed in 0..n as u64 {
            let p = generate(seed);
            inline_close += p.fired_inline_close as usize;
            drain_loop += p.fired_drain_loop as usize;
            two_spawn_arc += p.fired_two_spawn_arc as usize;
            map_decl += p.fired_map_decl as usize;
            array_intrinsic += p.fired_array_intrinsic as usize;
            shape_field_read += p.fired_shape_field_read as usize;
            owned_heap_channel += p.fired_owned_heap_channel as usize;
            pool_reuse += p.fired_pool_reuse as usize;
        }

        let floor = |count: usize, pct: usize, label: &str| {
            assert!(
                count * 100 >= n * pct,
                "{label}: only {count}/{n} ({:.1}%) fired — floor is {pct}%; a menu arm may \
                 have been deleted, starved, or a precondition arm is silently falling \
                 through to `stmt_int_let`",
                count as f64 * 100.0 / n as f64
            );
        };

        floor(
            inline_close,
            20,
            "close() + receive-past-end-of-stream (inline channel)",
        );
        floor(drain_loop, 20, "taught end-of-stream drain loop (arm 12)");
        floor(two_spawn_arc, 20, "two-spawn Auto-Arc topology (arm 13)");
        floor(
            map_decl,
            40,
            "standalone map<string, int> declaration (arm 7)",
        );
        floor(
            array_intrinsic,
            15,
            "array intrinsic .add()/.count()/index (arm 6)",
        );
        floor(shape_field_read, 25, "shape literal + field read (arm 9)");
        // This round's flagship construct — treated like the other three flagships above (a
        // floor comfortably below the measured ~77%, not merely "present at all").
        floor(
            owned_heap_channel,
            25,
            "a channel<array<int>>/channel<map<string,int>>/channel<number> composite actually ran",
        );
        // v0.3-M8 Phase 8 fix round 4, BLOCKER 1: this round's flagship construct was dead code
        // (see `take_or_make_array`'s doc comment) — a raw-count floor here, not a percentage
        // one, is the direct guard against it silently dying again. A percentage floor doesn't
        // fit this construct: reuse needs FOUR preconditions to align in the SAME statement (an
        // Array/Map inline-channel draw; a non-empty `arrays`/`maps` pool; the composite running
        // before ANY suspension anywhere earlier in the body; a `one_in(2)` coin flip), so it is
        // genuinely rare by construction — measured 2/256 over this exact fixed corpus. `>= 1`
        // is what "not dead" means here; `floor`'s percentage semantics would need `pct == 0` to
        // admit a measured value this small, which would silently accept 0 too and defeat the
        // guard's entire purpose.
        assert!(
            pool_reuse >= 1,
            "pool-reuse (take_or_make_array/take_or_make_map picking a PRE-EXISTING pooled \
             binding for a send) fired 0/{n} times — the reuse branch has gone dead again (this \
             is exactly the v0.3-M8 Phase 8 fix round 4 BLOCKER 1 shape: `suspension_seen` set \
             too early makes `!self.suspension_seen` permanently false)"
        );
    }
}
