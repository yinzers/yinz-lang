# Performance Lock-In Mistakes — CPU / Big-O / Algorithms

## Methodology

Investigated Rust, Go, Java, Python, C++, and JavaScript/Node.js. Primary benchmark sources: Rust Performance Book (nnethercote.github.io/perf-book), simdjson benchmarks (lemire.me/blog), Discord's Go→Rust post-mortem (discord.com/blog), Cloudflare Pingora report (blog.cloudflare.com), Figma Rust post-mortem (figma.com/blog), Go 1.24 Swiss Table release post (go.dev/blog/swisstable), Datadog Swiss Table production report, Computer Language Benchmarks Game, TechEmpower Round 23, and a corpus of production-engineer essays and GitHub issue threads. Citation success rate: approximately 78% of findings have a URL to a benchmark suite, production post-mortem, or compiler issue thread with attached numbers. The remaining 22% are marked `uncited (LLM recall)` and note where the claim is well-established but no single authoritative URL was pinned during research.

**After cleanup**: removed 24 findings (14 already-solved + 10 now-documented in design/); 12 at-risk/unaddressed/partially-addressed findings remain.

---

## Findings

---

### Finding #1: False sharing on atomic counters — 3× throughput collapse from cache line co-location

- **Sub-area**: concurrency-perf / memory-layout
- **Language**: All languages using shared-memory concurrency (Go, Rust, Java, C++)
- **What they did**: When two independent atomic variables are placed in the same 64-byte cache line, writes to either variable invalidate the entire cache line for all cores — even though each core is writing to a logically independent variable. This is false sharing.
- **The numbers**: A production lock-free queue saw a 3.1× performance collapse caused entirely by false sharing on tail/head pointers placed in the same cache line. Replacing 64-byte padding with 128-byte isolation and a shadow variable recovered the full 3.1× throughput. In another benchmark, eliminating false sharing produced ~3× faster lock-free data structures.
- **Why it became locked-in**: False sharing is invisible in code — the source looks correct, the data is not actually shared, and the bug only manifests on multi-core hardware under load. Most languages provide no automated detection or mitigation. Rust's `#[repr(align(64))]` and C++'s `alignas(64)` exist but require manual application. Default struct layout in every language does not cache-line-separate fields.
- **Receipt**: https://alic.dev/blog/false-sharing — "Measuring the impact of false sharing" with the 3.1× production measurement; https://coffeebeforearch.github.io/2019/12/28/false-sharing-tutorial.html — benchmark tutorial.
- **Yinz status**: unaddressed. Yinz's concurrency model (v0.2 `background` tasks) will eventually involve shared state. The compiler has the information (ownership tracking) to know when two fields accessed from different threads live in the same struct, and could warn or auto-pad. Worth adding to future language design backlog.

---

### Finding #2: Go string concatenation with `+` is O(n²) — creates a new allocation per concatenation

- **Sub-area**: stdlib-algorithm / runtime-vs-compile-time
- **Language**: Go (also JavaScript's naïve `+=` in a loop)
- **What they did**: Go strings are immutable. `s += chunk` in a loop allocates a new string each time, copying all previous bytes. A loop that builds a string of length N from N chunks performs O(N²) total copy work.
- **The numbers**: Benchmark comparing 1,000-string concatenation: `+` operator: 1,994,038 ns/op with 1,000 allocations; `strings.Builder`: 21,918 ns/op with 15 allocations — 91× faster in this test. The O(N²) behavior means the gap grows with string length and iteration count.
- **Why it became locked-in**: Immutable strings are a deliberate choice for thread safety and sharing in Go. Once strings are immutable, `+` must allocate. The idiom to use `strings.Builder` is well-known but not enforced — new Go developers consistently write the slow pattern and only fix it after profiling.
- **Receipt**: https://cristiancurteanu.com/why-your-string-concatenation-is-killing-performance-the-hidden-o-n2-trap-in-go/ — benchmarks showing 91× gap; https://hermanschaaf.com/efficient-string-concatenation-in-go/ — benchmark with multiple approaches.
- **Yinz status**: unaddressed. Yinz strings are also immutable. The IDE teaching layer (Golden Rule 11) should detect naïve string-building loops and suggest a buffer-backed approach. The compiler is the teacher — this is exactly the kind of pattern it should diagnose with a WHY explanation.

---

### Finding #3: Rust bounds check elimination fails in LLVM for widening-multiply patterns

- **Sub-area**: codegen
- **Language**: Rust
- **What they did**: Rust guarantees in-bounds access safety but promises to eliminate bounds checks it can prove statically safe. However, LLVM's alias analysis is not powerful enough to prove certain index patterns safe — notably, when an index is computed via a widening multiply (e.g., `(a as usize) * b` where a is u32). The bounds check remains in the generated code even though the math guarantees in-bounds access.
- **The numbers**: The specific failure mode (issue #65931 in the Rust repo) was reported with no workaround other than using `unsafe`. Equivalent C++ code with the same pattern can achieve the optimization because C is allowed to assume no undefined behavior — LLVM eliminates the bounds check there but not in Rust's safe code. The regression between LLVM versions added bounds checks to loops that previously had them eliminated.
- **Why it became locked-in**: LLVM's bounded alias analysis and integer range propagation are fundamental to its optimizer design. Teaching LLVM to prove more index patterns safe requires changes to LLVM internals. Rust cannot workaround it without unsafe code, which defeats the purpose. The issue has been open since 2019 with no full resolution.
- **Receipt**: https://github.com/rust-lang/rust/issues/65931 — LLVM fails to eliminate bounds checks with widening multiply; https://nnethercote.github.io/perf-book/bounds-checks.html — Rust Performance Book chapter on bounds checks with workarounds.
- **Yinz status**: PARTIALLY ADDRESSED. `design/ownership.md` 'No Direct Array Indexing' section commits to release-mode bounds-check elimination for statically-provable cases (e.g., `fixed<3>` accessed at index 1). The general LLVM widening-multiply limitation is mitigated by Yinz's design intent to do its own proof pass before delegating to LLVM, but this is not yet formally specified in `design/compiler.md`.

---

### Finding #4: Go interface dispatch prevents inlining — 2–3× overhead in tight loops vs concrete types

- **Sub-area**: codegen / runtime-vs-compile-time
- **Language**: Go
- **What they did**: Go interfaces use dynamic dispatch via an `itab` pointer (virtual function table). When a function accepts an interface argument and calls a method on it, Go cannot inline the callee because the concrete type is unknown at compile time. Even if a call site only ever sees one concrete type, Go's optimizer (without PGO) cannot prove it.
- **The numbers**: Interface dispatch vs. concrete type call shows 2–3× overhead depending on the benchmark. Polar Signals benchmark shows ~958 µs (interface) vs ~320 µs (type assertion) — approximately 3× ratio. Armir Ironenko's analysis shows ~2× overhead (2.636 ns/op interface vs 1.453 ns/op direct call). PGO (profile-guided optimization) in Go 1.21+ enables devirtualization for frequently-seen concrete types at call sites, recovering roughly 5% of overall program performance per the Go team's measurements.
- **Why it became locked-in**: Go's interface design is fundamentally structural typing (any type that has the right methods satisfies an interface, without declaring it). This prevents the compiler from knowing at declaration time what concrete types will appear. Devirtualization via PGO helps but requires a profiling step and is not applied to all call sites.
- **Receipt**: https://www.polarsignals.com/blog/posts/2023/11/24/go-interface-devirtualization-and-pgo — Polar Signals blog showing ~3× overhead; https://medium.com/@armironenko/go-performance-guide-part-1-non-zero-cost-abstractions-6c62f442536b — Armir Ironenko analysis showing ~2× overhead.
- **Yinz status**: at-risk. Yinz's `follows` contracts (analogous to Go interfaces or Rust traits) use static dispatch when the concrete type is known and dynamic dispatch only for actual runtime polymorphism. The design must enforce that the compiler always chooses static dispatch when calling a concrete type through a contract — this is Rust's approach (static dispatch by default via generics + `impl Trait`; dynamic dispatch is opt-in via `dyn`). Yinz should make this explicit in the language spec.

---

### Finding #5: Java sort is TimSort (stable, O(n) aux space) — overhead vs introsort for non-partially-sorted data

- **Sub-area**: stdlib-algorithm
- **Language**: Java, Python (also uses TimSort), Swift (switched from introsort to TimSort in 5.0)
- **What they did**: Java's `Arrays.sort()` (for objects) uses TimSort, a hybrid merge/insertion sort. TimSort requires O(n) auxiliary memory for merging. For nearly-sorted or reverse-sorted data, TimSort is dramatically faster (14× in one benchmark at 16M elements). For random data with no natural runs, TimSort's merge overhead costs ~20–30% vs a well-tuned quicksort.
- **The numbers**: At 16M elements on nearly-sorted data: TimSort completes in 0.15s vs introsort's 2.21s — 14× faster. For truly random data: TimSort's O(n) auxiliary allocation and merge overhead costs vs. in-place introsort. Java's sort for primitives (e.g., `int[]`) uses a dual-pivot quicksort (introsort) instead of TimSort — Java's designers recognized stability is irrelevant for primitives and avoided the memory overhead there.
- **Why it became locked-in**: Stability (equal elements maintain original order) is required for multi-key sort correctness in real applications (sort by name, then by age — the second sort must be stable). Making the default sort stable is correct for objects. The O(n) aux space is the price. Java chose correctly; the "mistake" framing here is that languages without a stable sort guarantee have the wrong default.
- **Receipt**: https://www.kirupa.com/sorts/timsort.htm — TimSort mechanics; https://ohmyswift.com/blog/2019/09/29/swift-5-replaces-introsort-with-timsort-in-the-sort-method/ — Swift's switch from introsort to TimSort with rationale.
- **Yinz status**: unaddressed. Yinz's `.sort()` is not yet specified. Recommendation from this research: default to TimSort (stable, adaptive) for correctness; provide `.sortUnstable()` for when stability is explicitly not needed and maximum speed is required. The distinction should be surfaced in the IDE teaching layer — when the compiler can see the sorted collection doesn't need stability (primitive types, new objects), it should suggest `.sortUnstable()`.

---

### Finding #6: Sequential-consistency atomic default — measurable overhead on ARM, negligible on x86

- **Sub-area**: concurrency-perf
- **Language**: C++ (default), Go (formerly; switched to acquire-release semantics), Java (`volatile` is seq-cst)
- **What they did**: C++ atomics default to `memory_order_seq_cst` — the strongest ordering guarantee. On x86, seq-cst is essentially free for loads (x86's TSO model provides acquire semantics by default) but adds a full memory fence on stores. On ARM (which is more weakly ordered), seq-cst adds explicit barrier instructions to both loads and stores.
- **The numbers**: For a benchmark suite of 1,279 programs on x86, seq-cst adds a geomean overhead of less than 0.4% — effectively free for most workloads. On ARM, the overhead is more significant: acquire-release semantics require fewer barriers, and using `LDAPR` (acquire-with-release semantics instruction added in ARMv8.1) instead of full seq-cst saves explicit barriers. Clang/LLVM only added LDAPR code generation in version 16 (March 2023).
- **Why it became locked-in**: For x86, seq-cst defaults are essentially costless and provide the simplest mental model. For ARM, the overhead is real but tools support for the more efficient ordering arrived recently. The ecosystem builds on seq-cst as the safe default, and relaxing atomics correctly requires deep understanding of memory models.
- **Receipt**: https://www.open-std.org/jtc1/sc22/wg21/docs/papers/2007/n2177.html — original C++ seq-cst paper; https://community.arm.com/arm-community-blogs/b/tools-software-ides-blog/posts/armv8-sequential-consistency — ARM seq-cst analysis.
- **Yinz status**: unaddressed. Yinz's concurrency model (v0.2) must specify default atomic ordering for its channel and shared-state primitives. The correct choice for a language targeting modern hardware is acquire-release by default (enough for most synchronization patterns), with seq-cst as an explicit opt-in.

---

### Finding #7: Go goroutine stack growth check at every function call — overhead for hot small-function paths

- **Sub-area**: codegen / runtime-vs-compile-time
- **Language**: Go
- **What they did**: Every Go function prologue includes a stack growth check — compare the current stack pointer to the goroutine's stack limit. This check is cheap (a few instructions) but present in every function call, including leaf functions that will never grow the stack. Functions can be annotated `//go:nosplit` to skip the check, but this requires manual intervention.
- **The numbers**: The hot-split problem (original segmented-stack design, pre-Go 1.4): a function at the boundary of a stack segment pays the full growth cost on every call that straddles the boundary, leading to exponential slowdowns in pathological cases. Go 1.4 switched to contiguous copying stacks to eliminate hot splits. The stack growth check overhead itself in current Go is small per call but measurable in tight loops calling many small functions — `//go:nosplit` directives on inner loop functions show up in performance-sensitive Go libraries.
- **Why it became locked-in**: Goroutines start at 2KB and must grow dynamically. Without the check, stack overflows would corrupt memory silently. The check cannot be removed without either fixing goroutine stack sizes at creation time (like OS threads — prohibitively expensive for millions of goroutines) or having the compiler prove a function never overflows (Go's compiler does not perform this analysis broadly).
- **Receipt**: https://blog.cloudflare.com/how-stacks-are-handled-in-go/ — Cloudflare's Go stack mechanics post; https://github.com/golang/go/issues/18138 — goroutine morestack overhead issue.
- **Yinz status**: unaddressed. Yinz will have a runtime (libynz_rt.a) and needs a stack growth strategy. The correct approach is LLVM's shadow stack or a similar static-analysis-informed stack sizing, defaulting to fixed-size stacks for functions the compiler proves stack-safe, with overflow detection only where needed.

---

### Finding #8: UTF-8 string traversal — naive byte scan is 20× slower than SIMD validation

- **Sub-area**: stdlib-algorithm / codegen
- **Language**: Python, Java, Go (all languages doing UTF-8 validation on string input)
- **What they did**: Naive UTF-8 validation processes one byte at a time, branching on each byte's lead bits to classify it as single-byte, 2-byte-lead, 3-byte-lead, or continuation. A simple state machine approach runs at approximately 8 cycles/byte. SIMD-accelerated UTF-8 validation (simdutf, fastvalidate-utf-8) processes 16–32 bytes at once using vector comparisons.
- **The numbers**: SIMD UTF-8 validation: as low as 0.7 cycles/byte, validating at approximately 13 GB/s on modern hardware (per simdjson's numbers). Naive approach: approximately 8 cycles/byte. For ASCII-only input (common in practice), SIMD can achieve less than 0.1 cycles/byte — 80× faster than naive per-byte processing. Node.js's adoption of simdutf produced a 364% benchmark improvement for UTF-8 decode-heavy workloads.
- **Why it became locked-in**: Standard library string handling was implemented before SIMD intrinsics were portable or practical. Python's string implementation in CPython is C code that processes characters using scalar comparisons. Replacing it requires SIMD intrinsics for each target architecture (x86 SSE2/AVX2, ARM NEON, RISC-V) and a fallback path — significant implementation surface. Node.js adopted simdutf as a dependency rather than rewriting the validation from scratch.
- **Receipt**: https://lemire.me/blog/2018/05/16/validating-utf-8-strings-using-as-little-as-0-7-cycles-per-byte/ — Daniel Lemire's 0.7 cycles/byte benchmark; https://github.com/simdutf/simdutf — simdutf README documents the 364% Node.js improvement.
- **Yinz status**: unaddressed. Yinz strings are UTF-8 (implied by `.byteAt()`, `.graphemeAt()` API). The string operations library must use SIMD-accelerated UTF-8 validation. This is a stdlib implementation choice, not a language design choice, but it should be specified in `design/stdlib/strings.md` (not yet written).

---

### Finding #9: PGO (Profile-Guided Optimization) is rarely used in production — leaving 10–15% performance on the table

- **Sub-area**: codegen
- **Language**: Rust, Go, C++ (all languages supporting PGO)
- **What they did**: PGO collects runtime execution profiles (which branches are hot, which functions are called most frequently) and feeds them back to the compiler for better inlining decisions, code layout, and branch prediction hints. Most production builds skip PGO because it requires a profiling run, a profile collection step, and a second compile.
- **The numbers**: rustc uses PGO for the Rust compiler itself. When PGO was enabled for rustc on Windows, the result was 10–20% improvement across most benchmark invocations (average 12.5%). In general, PGO provides 10–15% improvement for real-world workloads in languages that support it.
- **Why it became locked-in**: The PGO workflow is a multi-step process (instrument → run → collect → recompile) that doesn't fit into a single `cargo build` or `go build` command. CI pipelines rarely include the profiling step. Most language ecosystems don't surface PGO as a first-class build mode with tooling support.
- **Receipt**: https://rustc-dev-guide.rust-lang.org/profile-guided-optimization.html — Rust PGO docs; https://nnethercote.github.io/2022/07/20/how-to-speed-up-the-rust-compiler-in-july-2022.html — rustc PGO results showing 12.5% average win on Windows.
- **Yinz status**: unaddressed. The Yinz build system (not yet designed) should consider making PGO a first-class mode: `ynz build --profile` to run instrumented, `ynz build --optimized` to use the collected profile. This would be a differentiator if it's easy to use.

---

### Finding #10: Cloudflare Pingora (Rust) vs nginx — 70% less CPU, 67% less memory, connection reuse 3×

- **Sub-area**: runtime-vs-compile-time / memory-layout
- **Language**: Rust vs C (nginx)
- **What they did**: Cloudflare replaced nginx (their legacy proxy written in C) with Pingora, a proxy written in Rust. The key architectural difference was connection pooling: nginx's worker process model doesn't share connections across workers; Pingora's multi-threaded model shares connection state across all threads, multiplying connection reuse.
- **The numbers**: Pingora in production: 70% less CPU, 67% less memory for the same traffic load. Number of new connections opened reduced to one-third of nginx. Median response time reduced by 10ms. Third-party CDN performance tests show a 25% performance improvement. Serving over one trillion requests per day with these metrics.
- **Why it became locked-in for nginx**: nginx's architecture is process-based (multiple worker processes, each with independent memory). This prevents cross-worker connection sharing. Changing nginx's architecture would require rewriting its fundamental concurrency model — which is what Pingora effectively did.
- **Receipt**: https://blog.cloudflare.com/how-we-built-pingora-the-proxy-that-connects-cloudflare-to-the-internet/ — Cloudflare's engineering post with production numbers.
- **Yinz status**: N/A as a language design concern, but this is a data point for Yinz's value proposition: ownership semantics enable safe multi-threaded connection sharing that would require unsafe code or complex locking in C.

---

### Finding #11: Java's `HashMap` has a load factor default of 0.75 — causing resize at 75% fill

- **Sub-area**: stdlib-algorithm
- **Language**: Java
- **What they did**: Java's `HashMap` resizes when the number of entries exceeds `capacity × 0.75`. Resizing doubles the internal array and rehashes all entries — O(n) work. If a map will hold N entries and you don't pre-size it, Java will resize approximately `log2(N/16)` times during population, copying all entries each time.
- **The numbers**: A `HashMap` seeded to hold 1 million entries but default-constructed (initial capacity 16) will resize approximately 16 times, copying a total of 2M × log2 entries. Pre-sizing to the known capacity eliminates all resizes. For maps that are built once and read many times (common pattern), this is pure waste. By comparison, Rust's `HashMap` (hashbrown) also uses approximately 0.875 load factor but with better cache characteristics due to Swiss Table's group-based layout.
- **Why it became locked-in**: The 0.75 default was chosen in Java 1.2 (1998) as a balance between space and time. It's specified in the Javadoc as part of the public API contract, making it a de facto ABI guarantee. Changing the default would change behavior for code that relies on the documented resize threshold.
- **Receipt**: https://www.baeldung.com/java-hashmap-optimize-performance — Baeldung's HashMap optimization guide; https://java-performance.info/hashmap-overview-jdk-fastutil-goldman-sachs-hppc-koloboke-trove-january-2015/ — java-performance.info analysis.
- **Yinz status**: unaddressed. Yinz's `map<K, V>` implementation must specify initial capacity heuristics and load factor. When the compiler can infer the initial size from a map literal (`map<string, int> { "a": 1, "b": 2, ... }`), it should pre-size the map to the literal's entry count. This is compile-time work that eliminates runtime resize waste.

---

### Finding #12: C++ `std::string` lacks Small String Optimization enforcement — heap allocation for strings under 23 characters

- **Sub-area**: memory-layout / stdlib-algorithm
- **Language**: C++ (implementation-dependent), Python strings, Go strings
- **What they did**: C++ `std::string` implementations on GCC (libstdc++) use SSO for strings ≤ 15 characters. LLVM's libc++ uses SSO for strings ≤ 22 characters. Strings longer than the threshold heap-allocate. A process storing millions of short strings (keys, names, labels) pays per-string heap allocation overhead if even one string exceeds the threshold.
- **The numbers**: Benchmark comparison on x64 Intel i7 @3.40GHz: Windows ATL `CStringW` (no SSO) takes 29ms for 1 million push_back operations; STL `wstring` with SSO takes 14ms — 2× faster due to eliminated heap allocations. The SSO threshold varies by implementation (15 bytes on GCC, 22 on Clang), meaning code that works SSO-hot on Clang allocates on GCC for strings in the 16–22 byte range.
- **Why it became locked-in**: SSO is an implementation detail of `std::string`, not standardized. The C++ standard doesn't mandate SSO or specify its threshold. Different compilers make different choices, creating portability-dependent performance characteristics. Code tuned for Clang's 22-byte threshold may have unexpected heap allocations when compiled with GCC.
- **Receipt**: https://sqlpey.com/c++/small-string-optimization-sso/ — SSO analysis with benchmark data; https://giodicanio.com/2023/04/26/cpp-small-string-optimization/ — SSO threshold comparison across compilers.
- **Yinz status**: unaddressed. Yinz strings are immutable. For short string values that fit in a pointer-width (e.g., ≤ 23 bytes), the compiler should use a value-type representation (inline storage) rather than a heap pointer. This is a design decision that must be made before the string type is implemented; retrofitting SSO later requires ABI changes.

---

## Performance Headroom Patterns

Several categories of performance have been systematically under-claimed across languages:

**Hash table algorithms**: Every language that shipped its stdlib before 2017 shipped a suboptimal hash table. Swiss Tables were published by Google in 2017; any map implementation shipping before that date should be assumed outdated. The wins are large (20–60% for common workloads) and the fix is well-understood.

**GC elimination via ownership**: The Discord, Cloudflare, and Figma case studies all show that a deterministic-deallocation language beats a GC'd language not just in peak throughput but in tail latency — a different performance axis that GC tuning cannot fully address. Languages with GC cannot claim sub-millisecond tail latency under sustained allocation pressure.

**SIMD for stdlib operations**: JSON parsing, UTF-8 validation, string search, and regex matching all have SIMD implementations that are 4–40× faster than their scalar counterparts. Every language that implements these operations as scalar code leaves that gap on the table. The implementations exist; the barrier is porting and maintenance effort.

**Compile-time work avoidance**: Reflection-based serialization (Go `encoding/json`), dynamic dispatch (Java interfaces, Go interfaces, JavaScript polymorphism), and type erasure (Java generics) all move work from compile time to runtime that doesn't need to be there. Monomorphization + codegen eliminates this class of cost entirely.

**Aliasing information for vectorization**: C's voluntary `restrict` discipline and Rust's mandatory ownership tracking are the only widely-adopted mechanisms for giving LLVM the aliasing information it needs to auto-vectorize. Every other language leaves the optimizer flying blind, resulting in scalar code where SIMD was possible.

---

## Big-O Audit Table

| Operation | Rust | Go | Python | Java | Yinz target |
|-----------|------|-----|--------|------|-------------|
| HashMap insert (avg) | O(1) amortized — SwissTable | O(1) amortized — SwissTable (1.24+) | O(1) amortized — dict compact layout | O(1) amortized — separate chaining | O(1) — SwissTable |
| HashMap insert (worst) | O(n) rehash | O(n) rehash | O(n) rehash | O(n) rehash | O(n) rehash |
| HashMap lookup (avg) | O(1) — SIMD metadata scan | O(1) — SIMD metadata scan (1.24+) | O(1) — compact index array | O(1) — but pointer-chase per collision | O(1) — SIMD metadata scan |
| HashMap lookup (worst, no SwissTable) | O(n) collision chain | O(n) bucket scan (pre-1.24) | O(n) worst-case collision | O(n) linked list | O(1) bounded by SIMD group |
| Sort N items (random) | O(n log n) — pattern-defeating quicksort | O(n log n) — pdqsort | O(n log n) — TimSort | O(n log n) — TimSort (objects); dual-pivot qsort (primitives) | O(n log n) — TimSort stable / .sortUnstable() |
| Sort N items (nearly sorted) | O(n log n) worst — no TimSort exploitation | O(n log n) | O(n) — TimSort detects runs | O(n) — TimSort detects runs | O(n) — TimSort |
| Sort stability | Unstable (stable sort available separately) | Unstable | Stable | Stable (objects), unstable (primitives) | Stable default; opt-in unstable |
| Array push (amortized) | O(1) — 2×/1.5× adaptive | O(1) — 2× growth | O(1) — 1.125× growth | O(1) — 1.5× ArrayList growth | O(1) — specify 1.5× |
| Array index access | O(1) — bounds check (eliminated when provable) | O(1) — bounds check | O(1) — bounds check | O(1) — bounds check | O(1) — .get() bounds check; compile-time proof eliminates in release |
| String concat in loop (n chunks) | O(n) — String::push_str reuses buffer | O(n²) with +; O(n) with Builder | O(n²) with +; O(n) with join | O(n²) with +; O(n) with StringBuilder | O(n) — buffer-backed builder; IDE warns on O(n²) pattern |
| Regex match O(input) | O(n) — NFA-based, guaranteed | O(n) — RE2-based | O(2^n) worst-case — backtracking PCRE | O(2^n) worst-case — backtracking | O(n) — NFA-based; backtracking opt-in |
| JSON parse N bytes | O(n) scalar — ~100–200 MB/s | O(n) scalar — ~300 MB/s | O(n) scalar — ~50–100 MB/s | O(n) scalar — ~200–400 MB/s | O(n) SIMD — target ≥ 2 GB/s |
| UTF-8 validate N bytes | O(n) — simdutf in std (Rust 1.x) | O(n) scalar | O(n) scalar | O(n) scalar (UTF-16 internally) | O(n) SIMD — target ≤ 1 cycle/byte |
| Integer heap-alloc per value | No — i32/i64 are value types | No — int is value type | Yes — 28 bytes per Python int | Yes — 16 bytes per Integer (boxed) | No — int is i64 value type |
| Generic collection boxing | No — monomorphized | No — uses unsafe type-pun internally | N/A (dynamic) | Yes — type erasure forces Object boxing | No — monomorphized |

---

## Languages I Skipped and Why

- **C**: Not a "language making algorithm mistakes" — C lets you do anything, including optimal implementations. The performance mistakes in C happen in application code, not in the language's stdlib design choices. The relevant C comparison points (noalias/restrict, SSO in std::string) are covered under C++ findings.
- **Ruby**: Performance characteristics are similar to Python (interpreted, boxed integers, GC) but Ruby's ecosystem is more I/O-bound (Rails) than CPU-bound, making CPU/Big-O pitfalls less load-bearing as findings. The Python findings cover the shared root causes.
- **Swift**: Swift has good performance characteristics (monomorphization, value types, ARC rather than tracing GC). Its sort algorithm story (switched from introsort to TimSort in 5.0) is captured in Finding #13.
- **Zig**: Too early-stage for production benchmark evidence. Interesting decisions (comptime, no hidden control flow) but insufficient production post-mortems to cite.
- **Kotlin**: Runs on JVM, inherits Java's findings (#2, #19). Kotlin/Native exists but has limited production benchmark evidence at Yinz-relevant scale.
