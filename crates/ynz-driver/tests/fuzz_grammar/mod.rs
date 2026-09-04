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
    /// Number of statements emitted into `entrypoint`'s body — a non-triviality signal the
    /// harness reports, so "a meaningful hit rate" cannot be met by emitting `print(1)` a
    /// thousand times.
    pub body_stmts: usize,
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

/// A feeder function: sends `values` into a `lend channel<int>` then closes it.
struct FeedFn {
    name: String,
    values: Vec<i64>,
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
    uses_background: bool,
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
            uses_background: false,
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
        let n = self.rng.below(3); // 0..=2
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
            let count = 1 + self.rng.below(3);
            let values = (0..count).map(|_| self.rng.range(1, 12)).collect();
            self.feed_fns.push(FeedFn {
                name: format!("feed{i}"),
                values,
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

        let target = 9 + self.rng.below(10); // 9..=18 statements after the seed
        for _ in 0..target {
            self.emit_statement();
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
        self.ints.push(v);
    }

    fn stmt_array_decl(&mut self) {
        let a = self.fresh("rows");
        let n = 1 + self.rng.below(4);
        let elems: Vec<String> = (0..n).map(|_| self.int_lit()).collect();
        self.push(format!("let {a}: array<int> = [{}]", elems.join(", ")));
        self.arrays.push((a, n));
    }

    fn stmt_array_op(&mut self) {
        if self.arrays.is_empty() {
            return self.stmt_array_decl();
        }
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
        let m = self.fresh("book");
        let n = 1 + self.rng.below(MAP_KEYS.len());
        let mut entries = Vec::new();
        for k in MAP_KEYS.iter().take(n) {
            let v = self.int_lit();
            entries.push(format!("`{k}`: {v}"));
        }
        self.push(format!(
            "let {m}: map<string, int> = {{ {} }}",
            entries.join(", ")
        ));
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
    /// therefore byte-order-strict under the oracle.
    fn stmt_channel_inline(&mut self) {
        let c = self.fresh("wire");
        let cap = 1 + self.rng.below(8);
        self.push(format!("let {c}: channel<int> = channel<int>({cap})"));
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
        // Close, then one receive past end-of-stream: the v0.3-M8 Phase 4 contract says the
        // drained-and-closed channel answers `none`, never a hang and never an error.
        if self.rng.one_in(2) {
            self.push(format!("{c}.close()"));
            let m = self.fresh("got");
            self.push(format!("let {m} = {c}.receive()"));
            self.push(format!("if ({m}.exists()) {{"));
            self.push("  print(`unexpected value after end of stream`)".to_string());
            self.push("}".to_string());
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
        self.push(format!("let {v} = {r}.or(0)"));
        self.ints.push(v);
        self.uses_background = true;
    }

    /// The taught end-of-stream idiom: a `background` producer sends then closes; the consumer
    /// drains with `.exists()`/`.value` until the stream ends. The printed total is the sum of
    /// the feeder's literal values — deterministic regardless of interleaving.
    fn stmt_background_drain_loop(&mut self) {
        if self.feed_fns.is_empty() {
            return self.stmt_int_let();
        }
        let i = self.rng.below(self.feed_fns.len());
        let f = self.feed_fns[i].name.clone();
        let c = self.fresh("wire");
        let total = self.fresh("total");
        let open = self.fresh("open");
        let nx = self.fresh("nx");
        let cap = 1 + self.rng.below(4);
        self.push(format!("let {c}: channel<int> = channel<int>({cap})"));
        self.push(format!("background {f}({c})"));
        self.push(format!("let {total} = 0"));
        self.push(format!("let {open} = true"));
        self.push(format!("while ({open}) {{"));
        self.push(format!("  let {nx} = {c}.receive()"));
        self.push(format!("  if ({nx}.exists()) {{"));
        self.push(format!("    {total} = {total} + {nx}.value"));
        self.push("  }".to_string());
        self.push(format!("  {open} = {nx}.exists()"));
        self.push("}".to_string());
        self.push(format!("print({total}.toString())"));
        self.ints.push(total);
        self.uses_background = true;
    }

    /// Two `background` spawns handing the SAME read-only shape binding to the same reader,
    /// with nothing writing it in between — the Auto-Arc sharing topology this milestone's
    /// Phase 5 emits for. Each task sends one int; main drains exactly two.
    fn stmt_background_two_spawn_shape(&mut self) {
        if self.reader_fns.is_empty() {
            return self.stmt_int_let();
        }
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
        for f in &self.feed_fns {
            out.push_str(&format!(
                "function {}(lend wire: channel<int>) -> nothing {{\n",
                f.name
            ));
            for v in &f.values {
                out.push_str(&format!("  wire.send({v})\n"));
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

        let body_stmts = self.body.len();
        // Prune helpers the body never called? No — an uncalled helper is itself coverage
        // (it still lexes, parses, typechecks, and lowers), and pruning would make the
        // program a function of the body draw rather than of the seed.
        let uses_background = self.uses_background;
        let seed = self.seed;
        self.body.clear();
        Program {
            source: out,
            seed,
            uses_background,
            body_stmts,
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
            "// seed {seed} — {} body lines, background = {}\n{}",
            p.body_stmts, p.uses_background, p.source
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
            assert!(
                p.body_stmts >= 10,
                "seed {seed} emitted only {} body lines",
                p.body_stmts
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
    fn no_program_sends_an_owned_heap_payload_into_a_channel() {
        // v0.3-M8's transfer rules (`ConsumedBySend`, `TransferNeedsCopy`, `ParamNeedsGive`)
        // apply to owned-heap payloads. This generator keeps every channel `channel<int>` on
        // purpose: an `array`/`map` payload would need `.copy()`/`give` plumbing the grammar
        // does not model, and a correctly-REJECTED program reads as a harness failure.
        for seed in 0..128u64 {
            let src = generate(seed).source;
            assert!(
                !src.contains("channel<array") && !src.contains("channel<map"),
                "seed {seed} emitted an owned-heap channel payload"
            );
        }
    }

    #[test]
    fn suspending_calls_in_the_body_are_always_wait_prefixed() {
        // An un-prefixed adjacent pair would form an auto-parallel I/O group whose member
        // finish order legitimately differs between modes (the M3b Model-A intended reorder).
        // The oracle would flag that as a divergence; it is not one.
        for seed in 0..128u64 {
            let p = generate(seed);
            for line in p.source.lines() {
                let t = line.trim();
                for (name, _) in [("fetch0", 0), ("fetch1", 0)] {
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
}
