# Architectural Lock-In Mistakes — Build / Packaging + Cross-Platform

## Methodology

Each finding was identified from first-principles knowledge of language ecosystem history, then verified against web sources (official docs, postmortems, blog posts, bug trackers, PEPs/JEPs, HN threads). Where a URL confirms the specific claim, it is cited. Where the finding is common knowledge in the ecosystem but no single authoritative URL nails the specific claim made here, it is marked `uncited (LLM recall)`. The Yinz status column reflects the current state of Yinz design files (`design/packages.md`, `design/modules.md`, `design/compiler.md`, `design/config.md`, `design/future/packages.md`) as read at the start of this session.

**After cleanup**: removed 15 already-solved findings; 13 at-risk/unaddressed findings remain.

---

## Findings


### Finding #1: npm's Mutability — Left-Pad and the Unpublish Window

- **Category**: build-packaging
- **Language**: JavaScript / Node
- **What they did**: npm launched (2010) with a simple design rule: package authors own their packages and can unpublish at any time. No dependency on a package's existence was tracked. Any user could `npm unpublish` any package instantly.
- **When**: Policy existed from npm's beginning (2010); incident happened March 22, 2016.
- **Why it became locked-in**: The architecture stored packages by name+version but had no enforcement that "if N other packages depend on this, it cannot be removed." Adding that check retroactively required npm to permanently alter the contract between authors and the registry — which they did, but only after the incident. The design assumption of "author controls existence" is still present for packages under 24 hours old with no dependents.
- **The cost**: Azer Koçulu unpublished 273 packages on March 22, 2016 — including `left-pad` (11 lines of code, 2.5 million downloads/month). Within hours, builds for Babel, Webpack, React, and projects at Facebook, PayPal, Netflix, and Spotify broke worldwide. npm manually restored the package ~2 hours later and changed policy: packages with dependents and age >24h cannot be unpublished.
- **Receipt**: https://en.wikipedia.org/wiki/Npm_left-pad_incident · https://blog.npmjs.org/post/141577284765/kik-left-pad-and-npm · https://www.theregister.com/2016/03/23/npm_left_pad_chaos/
- **Yinz status**: **unaddressed** — `design/packages.md` and `design/future/packages.md` don't yet specify registry policies (unpublish windows, yanking vs deleting, immutability guarantees). This is a v0.2+ registry design concern (`design/registry.md` is cited as a future file). The bake-in-now opportunity is to commit to content-addressed, immutable package storage in the binary format and registry design before any packages ship.

---


### Finding #2: npm's Postinstall Scripts — Permanent Supply-Chain Attack Surface

- **Category**: build-packaging
- **Language**: JavaScript / Node
- **What they did**: npm designed lifecycle hooks (`preinstall`, `install`, `postinstall`) that run arbitrary shell scripts during `npm install`. This was designed for native module compilation (node-gyp). Any package can declare a postinstall script that runs as the installing user with full filesystem access.
- **When**: Feature existed from npm 1 (2010).
- **Why it became locked-in**: Thousands of packages legitimately use postinstall for native compilation (sqlite3, node-canvas, bcrypt, etc.). Removing postinstall would break native module installation across the ecosystem. The attack surface can be mitigated (`npm install --ignore-scripts`) but not removed without breaking the native module ecosystem.
- **The cost**: Nearly every supply-chain attack on npm uses postinstall as the execution vector. The March 2026 Axios attack (North Korea-nexus threat actor) used a malicious dependency's postinstall script to deploy a cross-platform RAT. pnpm v10 (2024) finally disabled postinstall execution by default. npm has not followed suit as of 2025.
- **Receipt**: https://www.microsoft.com/en-us/security/blog/2026/04/01/mitigating-the-axios-npm-supply-chain-compromise/ · https://pnpm.io/supply-chain-security · https://thehackernews.com/2022/09/warning-pypi-feature-executes-code-automatically-after-python-package-download.html
- **Yinz status**: **at-risk** — `design/packages.md` does not address build scripts or lifecycle hooks. The question of whether Yinz packages can declare code to run at install time is unanswered. If Yinz ever ships a build-script mechanism analogous to `build.rs` or `postinstall`, it should be sandboxed or opt-in at the consumer level, not the publisher level.

---

### Finding #3: Cargo's `build.rs` — Same Pattern, Slightly Better

- **Category**: build-packaging
- **Language**: Rust
- **What they did**: Cargo allows packages to declare a `build.rs` file — a Rust program that runs at build time before compilation. Designed for C/C++ FFI integration (compiling native libraries). Like npm's postinstall, it executes arbitrary code on the builder's machine.
- **When**: Feature available since Cargo's early versions (~2014).
- **Why it became locked-in**: Native FFI (openssl, sqlite, etc.) requires running build-system code to find or compile the native library. The pattern is load-bearing for the Rust ecosystem's ability to wrap C libraries.
- **The cost**: Running `cargo build` on an untrusted repository executes arbitrary code. The Rust security team explicitly documents "do not run any cargo commands on untrusted projects." Unlike npm's postinstall, `build.rs` is at least visible in the package source — but the supply-chain concern is the same. Sandboxing `build.rs` has been discussed but not implemented as of 2025.
- **Receipt**: https://shnatsel.medium.com/do-not-run-any-cargo-commands-on-untrusted-projects-4c31c89a78d6 · https://internals.rust-lang.org/t/sandbox-build-rs-and-proc-macros/16345
- **Yinz status**: **at-risk** — same concern as Finding #7. No build-script mechanism is currently designed, which is the right starting position. If one is ever added (for FFI compilation), the sandbox-by-default question should be resolved before it ships.

---

### Finding #4: Cargo Feature Unification — Workspace Dependency Conflicts

- **Category**: build-packaging
- **Language**: Rust
- **What they did**: When multiple crates in a Cargo workspace share a dependency, Cargo unifies all enabled features from all crates and builds the dependency once with the union of all features. This ensures a single compiled copy per dependency. However, it means a feature enabled by *any* workspace member is enabled for *all* of them, even if some members should not have that feature.
- **When**: Behavior existed from Cargo's beginning (~2014); recognized as a significant footgun by 2021.
- **Why it became locked-in**: Cargo's resolver was designed around the single-copy guarantee. Allowing per-member feature isolation would require building the same dependency multiple times, increasing build times and potentially introducing incompatibilities. A new resolver (v2, Cargo 1.50+) partially addresses this but doesn't fully fix the problem — separate build invocations or sub-workspaces are still the practical workaround.
- **The cost**: A concrete example: a workspace with a server crate (needing `zlib-ng-compat` feature of `flate2`) and a desktop crate (needing only the default implementation) would fail to build on machines without cmake, even when building only the desktop binary — because the server crate's features pollute the workspace-wide build. Binary size inflates as unwanted feature code is included. The fix (resolver = "2") was not backported and requires an opt-in declaration.
- **Receipt**: https://nickb.dev/blog/cargo-workspace-and-the-feature-unification-pitfall/ · https://doc.rust-lang.org/cargo/reference/features.html · https://rust-lang.github.io/rfcs/2957-cargo-features2.html
- **Yinz status**: **unaddressed** — `design/packages.md` specifies tree shaking but doesn't address the multi-target / multi-member workspace scenario. Yinz has no workspace design yet. When workspace/multi-crate support ships, this design question needs an explicit answer.

---






### Finding #5: Semver Violations — Trusted But Not Enforced

- **Category**: build-packaging
- **Language**: JavaScript / Node (and most ecosystems)
- **What they did**: npm (and most package ecosystems) adopted semantic versioning (semver) as the standard for expressing compatibility. Package managers use semver ranges (`^1.2.3`, `~1.2.3`) to allow automatic patch and minor upgrades. However, semver compliance is entirely voluntary — maintainers routinely ship breaking changes in minor versions and call them "behavioral improvements."
- **When**: npm adopted semver as default convention from npm 2 (~2012); semver.org spec established 2011.
- **Why it became locked-in**: Enforcing semver programmatically would require the package manager to run the test suite of every downstream consumer for every published version — impossible at scale. The ecosystem is built on a trust assumption that is frequently violated. Tools like `semantic-release` help but are optional.
- **The cost**: Dependabot and automated upgrade tools classify `^` upgrades as "low risk" based on the semver promise. When maintainers violate semver, these automated tools silently introduce breaking changes into downstream projects. Since virtually every library has transitive dependencies that violate semver at some point, "it never actually is semver" is a common practitioner complaint.
- **Receipt**: https://gist.github.com/jashkenas/cbd2b088e20279ae2c8e · https://fossa.com/blog/breaking-changes-rewriting-semantic-version/
- **Yinz status**: **unaddressed** — `design/versioning.md` specifies a no-backward-compat policy for Yinz itself, but doesn't specify what the Yinz package registry will enforce for third-party packages. The `yinz.lock` checksum model ensures bit-for-bit reproducibility within a locked build, but doesn't address version range assumptions before the lock is committed.

---

### Finding #6: Java's `java.util.Date` — Mutable, Timezone-Defaulting, Deprecated in 1997

- **Category**: cross-platform
- **Language**: Java
- **What they did**: Java 1.0 (1996) shipped `java.util.Date` — a mutable class representing a point in time, with constructors that took year/month/day values, methods that used the JVM's default timezone, and no clear separation between "an instant in time" and "a calendar date." Most methods were deprecated in Java 1.1 (1997) in favor of `Calendar`, which was also mutable and timezone-implicit.
- **When**: Java 1.0 (1996); deprecated 1997; replaced by `java.time` in Java 8 (2014).
- **Why it became locked-in**: Decades of Java code called `new Date()`, passed `Date` objects across APIs, serialized them in databases, and relied on the JVM default timezone behavior. The fix (`java.time`, JSR-310, 2014) required 17 years of Joda-Time workaround ecosystem. Legacy code using `java.util.Date` still exists in production systems and cannot be easily migrated without careful timezone analysis.
- **The cost**: 17 years from deprecation of the problematic methods (1997) to the proper replacement (Java 8, 2014). The default-timezone behavior means a Java program's date handling changes silently if deployed to a server in a different timezone — a class of bugs that has caused production outages. The `Date` class being mutable means defensive copying is required everywhere to prevent calendar aliasing bugs in concurrent code.
- **Receipt**: https://www.baeldung.com/java-date-time-history · https://medium.com/@renaud.mathieu/dealing-with-date-objects-in-android-part-2-history-of-date-in-java-a236a13333bb
- **Yinz status**: **unaddressed** — Yinz has no date/time stdlib design yet. When it ships, the lesson is: date/time types should be immutable, timezone-explicit (no default-timezone API), and separate "instant" from "calendar date" from "local time" from "zoned time."

---

### Finding #7: Java's UTF-16 Internal String Encoding — 2x Memory for ASCII

- **Category**: cross-platform
- **Language**: Java
- **What they did**: Java (1996) chose UTF-16 as the internal encoding for all `String` objects. Every character occupies 2 bytes minimum, regardless of whether it is ASCII. An ASCII string "hello" takes 10 bytes internally instead of 5.
- **When**: Java 1.0, 1996. The choice was made when Unicode was expected to be a 2-byte encoding (UCS-2). Surrogate pairs were added later, breaking the "16 bits is enough" assumption.
- **Why it became locked-in**: The `char` type in Java is a 16-bit unsigned integer. Changing `String` internals would require breaking the entire Java type system and every program that manipulates strings at the character level. A workaround (JEP 254 "Compact Strings") was shipped in Java 9 (2017): strings that contain only Latin-1 characters are stored as 1 byte per character with an encoding flag. But strings with any non-Latin-1 character still require 2 bytes per character throughout.
- **The cost**: Before Java 9 Compact Strings, all English-language string data used 2x the memory it needed. For string-heavy applications (web servers, data processing), heap pressure was significantly higher. The Compact Strings fix in Java 9 was measured to improve typical benchmark performance by 5–10% through reduced GC pressure. The fix took 21 years (1996–2017).
- **Receipt**: https://openjdk.org/jeps/254 (note: this URL returned 403; citing from search results) · https://www.javathinking.com/blog/what-is-the-java-s-internal-represention-for-string-modified-utf-8-utf-16/ · https://medium.com/@AlexanderObregon/character-encoding-in-java-strings-97dc9bc3b5f8 · `uncited (LLM recall)` for the 5-10% performance improvement figure.
- **Yinz status**: **unaddressed** — Yinz has no string implementation design yet. The lesson is: default to UTF-8 internally (compact for ASCII, exact for Unicode), making ASCII-heavy workloads use 1 byte per character without special flags.

---

### Finding #8: Python's `open()` — Platform-Dependent Default Encoding

- **Category**: cross-platform
- **Language**: Python
- **What they did**: Python 3's `open()` function reads and writes text files using the *operating system's preferred encoding* by default — `locale.getpreferredencoding(False)`. On Linux/macOS, this is typically UTF-8. On Windows, it is typically cp1252 (or another ANSI codepage).
- **When**: Python 3.0 (2008); the design was inherited from Python 2's locale-dependent behavior.
- **Why it became locked-in**: Changing the default would break any code that relied on the locale-dependent behavior — particularly code that correctly passed `encoding=None` to mean "use the system encoding." PEP 686 (Make UTF-8 mode default) was proposed in 2022, debated through Python 3.12, and retargeted for Python 3.15 (expected 2026 or 2027) — 17+ years after Python 3.0 shipped the wrong default.
- **The cost**: Python developers who develop on macOS/Linux write code that silently fails on Windows when it encounters non-ASCII data. The failure is a `UnicodeDecodeError` or garbled output — a runtime error, not a compile-time error. The pip package manager itself had a bug (issue #9573) where a temp directory with a non-Latin path character broke installation. This is one of the most common Python gotchas in cross-platform development.
- **Receipt**: https://peps.python.org/pep-0686/ · https://dev.to/methane/python-use-utf-8-mode-on-windows-212i · https://github.com/pypa/pip/issues/9573
- **Yinz status**: **unaddressed** — Yinz has no file I/O stdlib design yet. The lesson is: text file I/O should default to UTF-8 unconditionally; platform encoding should require an explicit opt-in. Java fixed this in Java 18 (JEP 400); Python is fixing it in 3.15.

---

### Finding #9: Locale-Sensitive `tolower` — The Turkish I Security Bug

- **Category**: cross-platform
- **Language**: C, C++, Java, Python, .NET, and most languages that exposed locale-sensitive string operations
- **What they did**: Many standard libraries exposed case-conversion functions (`tolower`, `toUpperCase`, `String.lower()`) that used the *current locale* to decide what uppercase and lowercase mean. In Turkish (`tr_TR`), lowercase `i` uppercases to `İ` (dotted I) and uppercase `I` lowercases to `ı` (dotless i) — not to `i`. This differs from English and breaks any security check that performs case-insensitive comparison via lowercase/uppercase normalization.
- **When**: The bug has existed since early string libraries; documented in Java and .NET since at least the early 2000s. First widely publicized around 2008.
- **Why it became locked-in**: The locale-sensitive API was the "correct" design for internationalization. Fixing it requires adding locale-explicit APIs (`String.toLowerCase(Locale.ENGLISH)`) and educating developers not to use the locale-sensitive versions for security-critical comparisons. The locale-sensitive API cannot be removed without breaking i18n use cases.
- **The cost**: Concrete confirmed instances: OpenSSL 3.0 had a Turkish locale bug where cipher name matching failed (Red Hat documented the fix). Apache Spark had a Turkish locale bug (SPARK-20156) where SQL keywords in uppercase failed to lowercase correctly, breaking query parsing on Turkish-locale systems. .NET, PHP, JavaScript frameworks, and VS Code all had documented Turkish locale issues as of 2025. The real-world severity: one documented case described a system where the bug contributed to a criminal incident.
- **Receipt**: https://haacked.com/archive/2012/07/05/turkish-i-problem-and-why-you-should-care.aspx/ · https://developers.redhat.com/articles/2022/06/15/openssl-30-dealing-turkish-locale-bug · https://issues.apache.org/jira/browse/SPARK-20156 · https://blog.codinghorror.com/whats-wrong-with-turkey/
- **Yinz status**: **unaddressed** — Yinz has no string/i18n stdlib design yet. The lesson is: string case operations that are used for comparison or security checks must specify a locale explicitly; there must be no "use the system locale" default for operations that affect program correctness.

---



### Finding #10: Windows MAX_PATH = 260 — DOS-Era Limit in Modern Code

- **Category**: cross-platform
- **Language**: All languages deploying on Windows
- **What they did**: Windows inherited from DOS (pre-1990) a 260-character limit on file paths (`MAX_PATH`). The Windows API enforced this by using fixed-size `char[260]` buffers in system calls. Any tool that stored or constructed paths longer than 260 characters would throw `PathTooLongException` (Java), `ERROR_PATH_NOT_FOUND` (Win32), or similar errors.
- **When**: The 260-character limit dates to Windows 95 (1995), inherited from DOS. Long path support (via `\\?\` prefix or Group Policy) was added in Windows 10 version 1607 (2016).
- **Why it became locked-in**: Win32 API buffers are sized to `MAX_PATH` throughout legacy code. Even after Windows 10 added a Group Policy option to remove the limit, .NET Framework applications prior to 4.6.2 required both the OS setting *and* an application manifest entry — and many tooling authors didn't know or didn't update their manifests. The `node_modules` directory is a particularly well-known victim: deeply nested packages create paths exceeding 260 characters, which has historically broken builds on Windows.
- **The cost**: npm 3's flat hoisting (Finding #6) was partially motivated by the MAX_PATH problem — deeply nested `node_modules` exceeded 260 characters on Windows, breaking builds. Windows 11 relaxed the limit further, but any tool that allocates `char[MAX_PATH]` still silently truncates. Java's `PathTooLongException` class exists specifically to surface this at runtime rather than silently.
- **Receipt**: https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation · https://www.howtogeek.com/266621/how-to-make-windows-10-accept-file-paths-over-260-characters/ · https://learn.microsoft.com/en-us/dotnet/api/system.io.pathtoolongexception
- **Yinz status**: **unaddressed** — Yinz's module system uses project-root-relative import paths. Whether the `yinz_modules/` directory layout will hit MAX_PATH on Windows depends on how deeply nested the on-disk layout is. This is a v0.2 concern for the package install mechanism, and should be tested explicitly on Windows before shipping.

---

### Finding #11: Windows Symlinks — Admin Rights Required Until 2016

- **Category**: cross-platform
- **Language**: All languages / build tools on Windows
- **What they did**: Windows supported symlinks via `mklink` but required administrator (elevated) privileges to create them. Unix/macOS symlinks require no special permissions for standard users. Tools that used symlinks for package linking (npm's peer dependencies, pnpm's storage model, Python's virtualenv on some configurations) either failed on Windows or fell back to copies.
- **When**: Windows Vista introduced `mklink` (2006) but required elevation. Windows 10 Creators Update (April 2017) added Developer Mode, which allows non-elevated symlink creation.
- **Why it became locked-in**: The elevation requirement reflected Windows security policy — symlinks can be used for privilege escalation attacks. Removing the requirement required a new OS-level opt-in (Developer Mode). Tools that detected the absence of symlink support fell back to different behavior, creating persistent cross-platform behavioral differences in how `node_modules` or similar structures work.
- **The cost**: pnpm's hard-link-based content-addressed store (its main performance advantage) required symlinks on POSIX. On Windows, pnpm used junctions (directory-only symlinks, no elevation required) as a fallback — but junctions have different behavior from symlinks (no relative paths, same-volume only). Many CI systems ran as non-admin users and couldn't use pnpm's ideal storage model on Windows.
- **Receipt**: https://blogs.windows.com/windowsdeveloper/2016/12/02/symlinks-windows-10/ · https://www.joshkel.com/2018/01/18/symlinks-in-windows/
- **Yinz status**: **at-risk** — `design/packages.md` specifies hard-links from cache to `yinz_modules/` for bun-class install speeds. Hard links (not symlinks) work without elevation on Windows. If the design requires symlinks anywhere (e.g., for peer dependency linking), this needs a Windows-specific fallback or the Windows behavior must be explicitly tested and documented.

---

### Finding #12: glibc Version Skew — Build-Machine vs Deploy-Machine

- **Category**: cross-platform
- **Language**: C, C++, Rust (dynamically linked), Go (when not statically linked)
- **What they did**: glibc (GNU C Library) uses versioned symbols — each exported function is tagged with the glibc version it was introduced in. Binaries compiled against glibc 2.35 contain references to symbols like `printf@GLIBC_2.35`. Those binaries fail with `GLIBC_2.35 not found` when deployed on systems with older glibc (e.g., RHEL 7 ships glibc 2.17). Forward compatibility is not guaranteed.
- **When**: glibc versioned symbols introduced ~glibc 2.1 (1999); the deployment problem has existed since then.
- **Why it became locked-in**: glibc is the canonical C library on Linux and cannot be bundled with an application (it includes the dynamic linker itself). Changing the versioning model would require rebuilding the entire Linux userspace. The workaround is to build on an old glibc system — hence `manylinux` in Python packaging (building wheels on ancient CentOS images) and the Python `manylinux1` / `manylinux2010` / `manylinux2014` wheel tags.
- **The cost**: Go's choice to compile to static binaries (no glibc dynamic linking by default) is a direct response to this problem — Go binaries run anywhere with the right architecture and kernel version. Rust defaults to dynamically linking libc (glibc on Linux), which means Rust binaries built on a modern Ubuntu won't run on old RHEL — unless built with `musl` libc. Python's `manylinux` wheel-building infrastructure exists entirely to work around this: release CI builds on ancient CentOS containers to target the oldest supported glibc.
- **Receipt**: https://edu.chainguard.dev/chainguard/chainguard-images/about/images-compiled-programs/glibc-vs-musl/ · https://jangafx.com/insights/linux-binary-compatibility · https://linuxvox.com/blog/various-glibc-and-linux-kernel-versions-compatibility/
- **Yinz status**: **unaddressed** — Yinz targets LLVM native code. Whether the compiler and runtime link against glibc dynamically (and which version) is a v0.1 question about the distribution model. The lesson from Go and musl is: static linking or musl libc targeting gives `run anywhere on Linux` portability; dynamic glibc linking ties the binary to a minimum distro vintage.

---




### Finding #13: Rust's Compile Times — Monomorphization at the Wrong Granularity

- **Category**: build-packaging
- **Language**: Rust
- **What they did**: Rust generates a separate compiled copy of every generic instantiation in every crate that uses it ("downstream monomorphization"). If ten crates all use `Vec<String>`, each crate compiles its own copy. The "final" binary crate ends up with enormous monomorphization work because it instantiates all the generics from all its dependencies.
- **When**: Design baked in from Rust 1.0 (2015).
- **Why it became locked-in**: Downstream monomorphization is what gives Rust its zero-cost generics — the generated machine code is type-specialized and can be fully optimized. The alternative (shared instantiation or vtable dispatch) would sacrifice performance, violating Rust's core promise. Changing this would require changes to the ABI and the crate compilation model. Incremental compilation and parallel front-end were added as mitigations (parallel-rustc reached nightly in November 2023) but don't address the fundamental quadratic scaling.
- **The cost**: Feldera documented a compile time of 25–45 minutes for their Rust project (64-core machine, Rust barely using any cores). The community has a tracking issue for compiler performance (rust-lang/rust #48547) dating from 2018. Practical workarounds (cranelift as a debug backend, sccache, splitting into more crates) reduce but don't eliminate the problem.
- **Receipt**: https://www.feldera.com/blog/cutting-down-rust-compile-times-from-30-to-2-minutes-with-one-thousand-crates · https://www.pingcap.com/blog/rust-huge-compilation-units/ · https://blog.rust-lang.org/2023/11/09/parallel-rustc/
- **Yinz status**: **at-risk** — `design/compiler.md` targets Go-like incremental build speed with Rust-like output quality, using Salsa for incremental computation. Monomorphization is a core part of Yinz's generic model. The document notes that Yinz generates separate compiled code per generic instantiation. Without a per-instantiation deduplication strategy (shared dict, or `codecgen` opt-in), Yinz will likely encounter the same compile-time scaling problem as Rust on large projects.

---

## Convergent Themes

**1. Executable metadata is permanently dangerous**: setup.py (Python), postinstall (npm), build.rs (Rust) all allowed arbitrary code at install/build time. In each case, removing the capability would break real users, so it persists as a supply-chain attack surface. The lesson: metadata must be static. If native compilation is needed, it should be sandboxed and opt-in by the consumer.

**2. Missing built-in tooling creates permanent third-party fragmentation**: When a language ships without a dependency manager (Python, Java, early Go, Apple/iOS), third parties fill the gap. Each fills it differently. The result is multiple incompatible ecosystems that can never be fully unified, because each has a user community with switching costs (Python: pip/conda split; iOS: CocoaPods/SPM; Go: GOPATH-era tools vs modules).

**3. Backward compatibility debt compounds over decades**: Java's Date (21 years to fix), Python's open() encoding (17+ years to fix), Go's GOPATH (9 years to transition), npm's lockfile (7 years without one), Java's JAR hell (21 years for JPMS). Every year of wrong default is a year of ecosystem code that relies on the wrong behavior. Fixing it requires either breaking everything or a painful opt-in migration that never fully completes.

**4. Cross-platform assumptions get locked in at the API surface**: The OS-level differences (case sensitivity, symlinks, encoding defaults, fork semantics, MAX_PATH) are not fixable. Languages that expose them directly inherit them permanently. Languages that abstract them correctly (e.g., Rust's `Path`/`PathBuf`) pay a verbosity cost but avoid the runtime-surprise pattern.

**5. Lazy locking is the #1 reproducibility failure**: Python's requirements.txt (no transitive lock), npm pre-5 (no auto-generated lockfile), GOPATH-era Go (no versioning at all) all share the same failure: something built differently on different machines without the developer noticing. A lockfile committed to version control is the minimum viable reproducibility guarantee.

**6. Domain names in import paths create hosting lock-in**: Go's `github.com/user/repo` import paths embed a hosting decision that is expensive to change. The escape valve (vanity URLs) requires infrastructure and is opt-in, so most packages don't use it. Import paths should be logical names, not hosting locations.

---

## Languages I Skipped and Why

- **Haskell / Cabal / Stack**: Both Cabal and Stack exist, with long historical friction between them, but the ecosystem is small enough that the lock-in cost is low — Haskell is not a mainstream deployment target. Worth noting for completeness but not high-signal for Yinz.
- **PHP / Composer**: Pre-Composer PHP is historically bad, but Composer fixed it adequately in 2012. The post-Composer story is relatively clean. The pre-Composer story (global include hell) is so extreme that it's a useful negative example, but the lock-in is pre-2012 and largely resolved.
- **Crystal**: Shard manager is first-party and relatively clean. Too small a community to have produced notable lock-in stories.
- **Nim**: The Nimble package manager has had fragmentation issues, but the community is small and the patterns mirror the larger ecosystems already covered.
- **Erlang/Elixir**: Mix and Hex are well-designed first-party tools; the main pain is Erlang/OTP version coupling rather than package manager design mistakes.
- **OCaml / opam**: Historical fragmentation is real but the community is small. The patterns (no lockfile until recently, multiple competing build systems) mirror the Python and Java stories already covered.
