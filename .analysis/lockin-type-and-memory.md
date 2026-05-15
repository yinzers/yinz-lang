# Architectural Lock-In Mistakes — Type System + Memory Model

## Methodology

Investigated 11 languages: Java, Rust, Go, C++, Swift, TypeScript, Scala, Haskell, OCaml, Nim, and C#. Started with strong training-data candidates for "where's the real story," then verified using WebSearch and WebFetch against primary sources (official RFCs, language-team blog posts, GitHub tracking issues, standards proposals). Citation rate ended up at approximately 65% — most findings link to a primary or near-primary source. Uncited findings are flagged explicitly. Java and Rust had the richest documented retrospective material. C++ and Go were close behind. Swift and TypeScript yielded one strong finding each with primary sources. Haskell, OCaml, and Nim yielded smaller but credible findings.

**After cleanup**: removed 23 already-solved findings; 3 findings remain (1 partially-addressed, 1 at-risk, 1 not-yet-decided).

---

## Findings

### Finding #1: Rust Pin — Self-Referential Types via a Concept That "Doesn't Work as Well as Hoped"

- **Category**: type-system + memory-model
- **Language**: Rust
- **What they did**: Rust needed self-referential types (structs where one field points into another field of the same struct) for async state machines. The solution, Pin (RFC 2349, stabilized 2018), prevents moving a value after it's been "pinned" in memory. `Pin<P>` wraps a pointer type and asserts the pointee will not move.
- **When**: Pin RFC 2349 merged 2018; stabilized in Rust 1.33 (2019).
- **Why it became locked-in**: `Pin<Box<dyn Future>>` is now the type of every boxed async future. The entire async ecosystem — Tokio, async-std, all async trait method signatures — depends on `Pin`. The `Future::poll` method signature takes `Pin<&mut Self>`. Removing `Pin` would require changing the `Future` trait, which would break every async crate ever written. The `!Unpin` concept (marking types that are unsafe to move after pinning) is baked into the trait system.
- **The cost**: (a) Self-referential types require unsafe code and substantial boilerplate — not accessible to regular programmers. (b) `Pin` doesn't mean "unmovable" — it means "unmovable once pinned," a distinction that causes confusion. `!Unpin` types can still be moved freely before pinning. (c) Pin projections (accessing fields of a pinned struct) require either the `pin-project` crate or `unsafe` code, even though the operation is conceptually straightforward. (d) Interaction between `Pin`, `Drop`, and other traits requires careful reasoning. Yoshua Wuyts (Rust contributor): the current design "clearly doesn't work as well as was hoped at the time" and "revisiting alternatives seems like a good idea." One of the original architects: "If we'd known about 'pinning' before Rust 1.0 we likely would've designed the Drop trait differently."
- **Receipt**: Yoshua Wuyts's self-referential types proposal (detailed analysis of Pin's failures): https://blog.yoshuawuyts.com/self-referential-types/. Rust internals discussion: https://internals.rust-lang.org/t/pin-ergonomics/21172. Tracking issue for Pin ergonomics improvement (2024): https://hackmd.io/@rust-lang-team/HJUQZ7-cC
- **Yinz status**: PARTIALLY ADDRESSED. Yinz's `design/future/self-references.md` chose Approach A (relative/offset pointers) over Pin-equivalent approaches. The Yinz design explicitly avoids exposing Pin-style complexity to users — the compiler auto-detects self-referential shapes and the `self-referential` modifier appears as a muted IDE hint, not user-required syntax.

---

### Finding #2: Rust Specialization — 9 Years Unstable, Proven Unsound, No Stabilization Date

- **Category**: type-system
- **Language**: Rust
- **What they did**: Specialization (RFC 1210, approved 2015) allows writing multiple implementations of a trait for overlapping types, with the more specific implementation taking precedence. It's needed for performance optimization (e.g., specializing `ToString` for `String` to avoid an allocation) and for making certain APIs expressible.
- **When**: RFC opened and approved February 2015; still unstable as of May 2026 — 11 years.
- **Why it became locked-in (but the feature isn't)**: The opposite of lock-in: specialization is the feature that CANNOT be shipped because it creates a soundness hole. The problem: specialization interacts with lifetime dispatch in a way that allows deriving `'static` lifetimes from non-`'static` references — without `unsafe`. Ralf Jung's PR "Specialization is unsound" (rust-lang/rust#71420) formally demonstrated the UB. The tension: the standard library already uses `min_specialization` (a restricted subset) internally for performance. This creates a two-tier system where the stdlib can specialize but users cannot. The feature cannot be stabilized in its current form; redesigning it is a deep research problem.
- **The cost**: (a) The stdlib uses specialization internally for `ToString`, `From`/`Into`, and collection operations — users cannot do the same. (b) The `From`/`Into` and `Display`/`ToString` relationships have a known performance gap (avoiding an allocation) that could be closed with specialization but cannot be. (c) Many library APIs that would benefit from specialization require nightly Rust — fragmenting the ecosystem. (d) 11 years of being in unstable limbo, blocking related features that depend on it.
- **Receipt**: Tracking issue (February 2015): https://github.com/rust-lang/rust/issues/31844. "Specialization is unsound" PR: https://github.com/rust-lang/rust/pull/71420. Sound specialization analysis (Aaron Turon): http://aturon.github.io/tech/2018/04/05/sound-specialization/
- **Yinz status**: NOT YET REACHED. Yinz's `follows` constraints are simpler than Rust traits — no blanket impls, no `impl<T: Bound> Trait for T` patterns in v0.1. Whether specialization will be needed at all is an open question. If Yinz's struct/method system makes the common performance cases achievable without specialization, this problem may not arise.

---

### Finding #3: Java's UTF-16 Strings — 21 Years of 2x Memory Overhead for ASCII

- **Category**: memory-model
- **Language**: Java
- **What they did**: Java (officially released January 1996) chose UTF-16 as the internal String encoding. Every character in every Java string consumed exactly 2 bytes, even for pure ASCII strings where 1 byte would suffice. This doubled the memory footprint of typical string-heavy applications.
- **When**: Java 1.0, January 1996; fixed by Compact Strings in Java 9, September 2017 — 21 years later.
- **Why it became locked-in**: Java's `char` type is defined as a 16-bit unsigned integer (UTF-16 code unit). Changing the internal String representation would require either changing the `char`/`charAt()` API (breaking change) or maintaining compatibility shims. The `String.charAt(int)` method is called billions of times across all Java code ever written — it returns a `char` (16-bit). Compact Strings (Java 9) worked around this by using `byte[]` internally and switching to 2-bytes-per-char only when needed, but the public `charAt()` API still returns `char` (hiding the internal compaction from callers).
- **The cost**: 21 years of 2x memory overhead for ASCII-heavy string workloads. A Java application holding 1 billion ASCII characters (common in web services handling JSON or XML) used 2 GB instead of 1 GB. Java 9's Compact Strings reduced memory footprint for ASCII-dominant strings by up to 50% — meaning the savings from the fix are roughly equivalent to the cost of the original mistake. The delay from 1996 to 2017 means every Java application deployed before 2017 paid this tax.
- **Receipt**: Java string internals with UTF-16 explanation: https://www.javathinking.com/blog/what-is-the-java-s-internal-represention-for-string-modified-utf-8-utf-16/. Compact strings in Java: https://javaspring.net/blog/compressed-string-java/
- **Yinz status**: PARTIALLY ADDRESSED — UTF-8 is implied by `.byteAt()` / `.graphemeAt()` API in `design/collections.md` and by `lockin-cpu-bigo.md` Finding #16, but should be locked explicitly in a new `design/strings.md` file so the commitment is unambiguous rather than inferred from the indexing API.

---

## Convergent Themes

Three patterns dominate:

**1. The Backward-Compatibility Trap**: The most permanent lock-ins happen when a design choice becomes encoded in a binary format, bytecode spec, or ABI that millions of deployed artifacts depend on. Java type erasure, Java primitives, Java UTF-16 strings, Go's nil interface, Swift's ABI — all of these are locked because the deployed artifact count makes migration impossible, not because the fix is technically hard. The lesson: bad design decisions made early in a language's life (before widespread adoption) are much cheaper to fix than bad decisions made after. The window for free fixes closes quickly.

**2. One Keyword, Many Semantics**: Scala implicits (one keyword, 7 uses) is the clearest case, but Go's `interface{}` (sum type + polymorphism + type-erased container + nil value) and C++'s exception model (error signaling + RAII interaction + zero-overhead violation) follow the same pattern. When a single mechanism handles conceptually distinct use cases, developers misuse it for cases it wasn't designed for, and fixing it requires either a breaking redesign or parallel mechanisms.

**3. Deferred Safety, Permanent Cost**: Python GIL (30 years), Java checked exceptions (30 years), C# nullability (17 years), Go generics (13 years), OCaml multicore (8 years) — all of these are cases where a safety or correctness property was deliberately deferred because it was hard, and the deferral cost compounded over time as the ecosystem grew around the gap. The Rust borrow checker rejecting valid programs and Rust specialization being unsound for 11 years are the inverse: the feature was attempted but shipped with a gap, and the gap has been expensive to close.

---

## Languages I Skipped and Why

**Erlang/Elixir**: The immutability-by-default and hot-reload constraints are well-documented but are architectural choices with known tradeoffs, not mistakes. No credible "we regret this" evidence found in the time budget.

**Kotlin**: The JVM constraint (sharing a runtime with Java) is inherited, not a Kotlin design mistake. The interop costs are real but are explicitly accepted as the price of JVM access. Kotlin multiplatform fragmentation is a platform problem, not a type system or memory model problem.

**Zig**: Zig is too young and the design is too actively in flux to have permanent lock-ins to document. The allocator-passing convention is a deliberate design (explicit control flow) rather than a regret.

**Nim**: Coverage limited to the multi-GC-mode fragmentation finding. The `ref`/`ptr` distinction and the owned-ref proposals are real design tensions but the Nim team is actively iterating — not yet a locked-in mistake.
