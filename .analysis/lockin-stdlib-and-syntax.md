# Architectural Lock-In Mistakes — Stdlib API + Syntax

## Methodology

For each candidate mistake, I (a) drew on training knowledge to identify the pattern, (b) performed one or more WebSearch calls to locate a receipt (GitHub issue, TC39 proposal, RFC, blog post with citations, or official documentation), and (c) fetched the primary source when needed to extract concrete numbers or dates. Items I could not verify with a real URL are marked `uncited (LLM recall)`. The Yinz status column reflects the state of `design/` files read at session start. Findings are ordered roughly by lock-in severity (most permanent first within each category).

---

## Findings

### Finding #1: JavaScript `typeof null === 'object'`
- **Category**: syntax
- **Language**: JavaScript
- **What they did**: The first JS engine stored a type tag in the low bits of every value. Objects had tag `000`; `null` was also stored as all-zero bits, so the type-check landed in the object branch. `typeof null` has returned `'object'` since Netscape 2 (1995).
- **When**: 1995 — present
- **Why it became locked-in**: A patch was shipped in V8 under the ES5 harmony flag but broke enough real web pages that the ES5 committee withdrew it. Brendan Eich himself said: "I think it is too late to fix typeof. The change proposed for typeof null will break existing code." A second attempt in 2013 was rejected for the same reason.
- **The cost**: Every JavaScript developer must learn this exception. `typeof` is the most basic introspection tool in the language, and it lies about one of its four primitive types. The Temporal proposal and nullish-coalescing additions all have to route around it.
- **Receipt**: https://esdiscuss.org/topic/typeof-null — the original esdiscuss thread; https://dev.to/m0slah/why-typeof-null-object-in-javascript-a-deep-dive-into-a-quirky-bug-n6p
- **Yinz status**: Already solved. Yinz uses `none` for absent values; the concept has a distinct identity and is not a subtype of any collection type.

---

### Finding #2: JavaScript `Date` — months 0-indexed, year arithmetic for 1900s
- **Category**: stdlib-api
- **Language**: JavaScript
- **What they did**: `new Date(2026, 4, 15)` is May 15, not April. Months are 0–11. Days are 1–31. Year values < 100 are treated as "1900 + year" (so `new Date(99, 0, 1)` is Jan 1, 1999 — fixed by passing a full 4-digit year via `setFullYear`). The object is mutable. No timezone awareness in the default constructor.
- **When**: 1995 — present (the `Date` object was copied from Java's `java.util.Date` which borrowed the zero-indexed month from Unix's `struct tm` which dates to 1973 Research Unix V4)
- **Why it became locked-in**: Browsers shipped, the web shipped, and the ECMAScript spec froze the behavior. The replacement (Temporal API) spent nine years in the TC39 proposal process and reached Stage 4 in March 2026. The old `Date` object cannot be removed.
- **The cost**: Nine years of committee time. Every date library (Moment.js, date-fns, dayjs, Luxon) exists primarily to paper over this API. Temporal reached Stage 4 in March 2026 (ECMAScript 2026) — Firefox 139 ships it by default, Chrome 144 shipped January 2026.
- **Receipt**: https://waspdev.com/articles/2025-05-24/temporal-api — Temporal status; https://socket.dev/blog/tc39-advances-temporal-to-stage-4 — Stage 4 confirmation; https://www.jefftk.com/p/history-of-zero-based-months — historical origin
- **Yinz status**: Already solved. `design/stdlib/dates.md` explicitly states "months are NOT zero-indexed. May = 5, not 4." Dates are also immutable — all mutation methods return new values.

---

### Finding #3: Go `time.Format` magic reference date
- **Category**: stdlib-api
- **Language**: Go
- **What they did**: Instead of format specifiers (`%Y`, `%m`, `%d`) or named tokens (`YYYY`, `MM`), Go uses a specific real date as a template: `Mon Jan 2 15:04:05 MST 2006`. To format a year, you write `2006`. To format a month with leading zero, you write `01`. To format a 12-hour hour, you write `3`. Every token is the value of that field in the reference datetime (Unix timestamp 1136239445).
- **When**: Go 1.0 (2012) — present
- **Why it became locked-in**: GitHub issue #38871 (filed 2020) was closed "FrozenDueToAge" with no fix. The Go team acknowledged it as "a regrettable historic error." Go 1 compatibility guarantee prevents changing any existing API; a new formatting API would need to coexist.
- **The cost**: Constant developer confusion, especially for non-American developers (month=01, day=02 follows US convention). Every code review of a Go date format requires verifying against the reference. The Hacker News thread "Fucking Go Date Format" has 300+ comments.
- **Receipt**: https://github.com/golang/go/issues/38871 — the tracked issue, closed FrozenDueToAge; https://lobste.rs/s/8hbuzu/fucking_go_date_format — community thread; https://medium.com/@lordmoma/why-go-keeps-torturing-me-on-dates-formatting-0548051aa941
- **Yinz status**: Already solved. `design/stdlib/dates.md` uses named readable methods (`date.from(year, month, day)`) and a format-string approach using readable tokens (`"MMMM D, YYYY"`), not magic numbers.

---

### Finding #4: Java `java.util.Date` / `Calendar` — deprecated but permanent
- **Category**: stdlib-api
- **Language**: Java
- **What they did**: `java.util.Date` shipped in Java 1.0 (1996). It was mutable. It had no timezone. Many constructors were deprecated as early as JDK 1.1 (1997). `java.util.Calendar` replaced it in 1997 but was also poor. `java.time` (JSR-310) finally shipped a correct API in Java 8 (2014).
- **When**: 1996 — `java.util.Date` still ships in every JDK in 2026
- **Why it became locked-in**: Oracle's Java backwards-compatibility guarantee. No Java class can be removed once shipped in a non-preview release. The OpenJDK mailing list had a thread in 2022 proposing formal deprecation-for-removal of `java.util.Date` and `java.util.Calendar`, but the thread concluded the classes must remain due to too many external libraries depending on them.
- **The cost**: 18 years of a wrong Date API (1996–2014). Every Java codebase post-2014 must actively resist using `Date` and teach every new hire to avoid it. Libraries like Joda-Time (the spiritual precursor to `java.time`) exist solely because the stdlib was broken.
- **Receipt**: https://mail.openjdk.org/pipermail/discuss/2022-May/006077.html — 2022 OpenJDK deprecation discussion; https://docs.oracle.com/javase/8/docs/api/java/util/Date.html — official javadoc with deprecation notes
- **Yinz status**: Already solved (see Finding #2 and #3).

---

### Finding #5: Java checked exceptions — API evolution lockout
- **Category**: syntax
- **Language**: Java
- **What they did**: Methods declare checked exceptions in their signature: `void read() throws IOException`. Every caller must either catch or re-declare. Adding a new checked exception to a public method is a source-breaking change: every downstream caller that didn't catch the new exception fails to compile.
- **When**: Java 1.0 (1996) — present in the spec; universally avoided in modern Java
- **Why it became locked-in**: The feature is part of the JVM bytecode spec and the language specification. Removing it would break every existing library that declares checked exceptions. Every new JVM language (Kotlin, Scala, Groovy) made all exceptions unchecked by default. Java itself abandoned checked exceptions in the Streams API (Java 8) because lambdas cannot propagate checked exceptions across a functional interface.
- **The cost**: Every major Java framework (Spring, Hibernate, Jackson) wraps checked exceptions in unchecked runtime exceptions. Java's own Oracle tutorials admit "checked exceptions are controversial." The Eclipse IDE added "add throws declaration" and "wrap in RuntimeException" quick-fixes as escape hatches because developers refused to handle checked exceptions at every layer.
- **Receipt**: https://www.javacodegeeks.com/2026/01/javas-checked-exceptions-the-20-year-experiment-that-failed.html; https://literatejava.com/exceptions/checked-exceptions-javas-biggest-mistake/ — "2000+ non-functional catch-throw blocks per project"; https://news.ycombinator.com/item?id=24440536
- **Yinz status**: Already solved. `design/errors.md` uses the `errors` keyword — not try/catch, not checked exception declaration in signatures. Error propagation is opt-in via flow-sensitive narrowing; API evolution doesn't break callers.

---

### Finding #6: Python string formatting — three permanent competing systems
- **Category**: syntax
- **Language**: Python
- **What they did**: Python shipped with `%` printf-style formatting (Python 1.0, 1994). Then added `.format()` string method (Python 2.6/3.0, 2008). Then added f-strings (Python 3.6, 2016). All three coexist; none can be removed.
- **When**: 1994, 2008, 2016 — all three active in 2026
- **Why it became locked-in**: `%` formatting is used in the logging stdlib (deliberately, for deferred evaluation) and in millions of codebases. `.format()` is in even more code. Removing either would be mass-breakage. f-strings cannot substitute in all logging contexts because they evaluate eagerly.
- **The cost**: New Python developers must learn all three to read existing code. Code style guides exist solely to pick one for new code. The `logging` module's documented best practice still uses `%` formatting for performance, making the "use f-strings everywhere" advice wrong for at least one stdlib module. Python's own documentation gives three examples of each.
- **Receipt**: https://docs.bswen.com/blog/2026-04-14-python-string-formatting/ — side-by-side comparison; https://peps.python.org/pep-0498/ — PEP 498 (f-strings); the `%` format predates any PEP
- **Yinz status**: Already solved. Yinz has exactly one string interpolation mechanism (to be designed). The lesson: design interpolation once and make it work for all contexts including lazy/deferred evaluation.

---

### Finding #9: JavaScript `var` hoisting — dead but immortal
- **Category**: syntax
- **Language**: JavaScript
- **What they did**: `var` declarations are hoisted to the top of their enclosing function (or global scope) and initialized to `undefined`. A variable declared with `var` can be used before its declaration site with no error.
- **When**: 1995 — `var` still valid in ECMAScript 2026
- **Why it became locked-in**: Removing `var` semantics would break any code relying on hoisting behavior. `let` and `const` (ES6, 2015) were added to fix this; they have block scope and a temporal dead zone. But `var` remains in the spec and is valid syntax. Every browser will honor it forever.
- **The cost**: Strict mode (`'use strict'`) was added partly to change scoping behavior. Two decades of "why is `x` undefined?" debugging. The `const`/`let` migration in large codebases took years; thousands of packages still distribute `var`-based code. ES6's TDZ for `let`/`const` introduced its own confusion for developers expecting the same hoisting behavior.
- **Receipt**: https://www.freecodecamp.org/news/javascript-let-and-const-hoisting/ — TDZ explainer; https://www.freecodecamp.org/news/var-let-and-const-whats-the-difference/ — official MDN ref history
- **Yinz status**: Already solved. Yinz has `let` and `const` with block scope; no hoisting. `design/scope.md` explicitly prohibits hoisting.

---

### Finding #10: Python mutable default arguments
- **Category**: syntax
- **Language**: Python
- **What they did**: Default argument values are evaluated once when the function is defined, not when it is called. `def append_to(x, lst=[])` shares the same list across all calls that don't pass `lst`.
- **When**: Python 1.0 (1994) — present
- **Why it became locked-in**: The behavior follows directly from Python's "functions are first-class objects" design: default values are stored as attributes on the function object. Changing the behavior would require either making functions heavier (copy defaults on every call) or introducing a new syntax. PEP 671 proposes a `=>` "late-bound default" syntax, but as of 2026 it has not been accepted. Changing existing behavior would break code that deliberately exploits mutable defaults as a cache.
- **The cost**: One of the top gotchas in every Python intro course. The Hitchhiker's Guide to Python devotes a section to it. Silent shared state across function calls; debugging requires knowing which invocation modified the default.
- **Receipt**: https://docs.python-guide.org/writing/gotchas/ — Hitchhiker's Guide; https://florimond.dev/en/posts/2018/08/python-mutable-defaults-are-the-source-of-all-evil; https://peps.python.org/pep-0671/ — the proposed fix, still unaccepted
- **Yinz status**: Not applicable in the same form (Yinz is statically typed, values are owned/moved, default values would need careful design). At risk for the principle: function default values should not be shared mutable state. Worth flagging when designing the Yinz default-argument model.

---

### Finding #11: Python list comprehension scope leak (Python 2)
- **Category**: syntax
- **Language**: Python
- **What they did**: In Python 2, the iteration variable of a list comprehension leaked into the enclosing scope. `x = 100; [x for x in range(5)]; print(x)` printed `4`, not `100`.
- **When**: Python 1.x to Python 2.7 (the leak). Fixed in Python 3 (2008) by giving comprehensions their own scope.
- **Why it became locked-in (temporarily)**: The fix was a breaking change — Python 3 changed the semantics. This is one of the reasons Python 2→3 migration took from 2008 to 2020 (Python 2 EOL). The fix was "free" only because Python 3 was willing to break backwards compatibility; in a language without a version 3, this would be permanent.
- **The cost**: The Python 2→3 migration is often cited as the most painful version transition in any major language. It took 12 years to sunset Python 2 after Python 3 shipped. The scope-leak fix was one of dozens of breaking changes; together they prevented easy migration and split the ecosystem into 2.x and 3.x for years.
- **Receipt**: https://codeql.github.com/codeql-query-help/python/py-leaking-list-comprehension/ — CodeQL description; https://riptutorial.com/python/example/7961/leaked-variables-in-list-comprehension — examples of the behavior
- **Yinz status**: Already solved. Yinz has block scoping; `design/scope.md` is explicit that variables exist only in the block where they're declared.

---

### Finding #12: PHP needle/haystack argument order inconsistency
- **Category**: stdlib-api
- **Language**: PHP
- **What they did**: String functions take haystack first: `strpos($haystack, $needle)`. Array functions take needle first: `in_array($needle, $haystack)`. This is the opposite convention. There is no mnemonic; you must memorize each function.
- **When**: PHP 3 / PHP 4 era (late 1990s) — present
- **Why it became locked-in**: PHP has never had a compatibility-break release equivalent to Python 3. The argument orders are now baked into billions of lines of PHP code. A comprehensive audit found both conventions used across the PHP standard library with no consistent rule even within subcategories.
- **The cost**: PHP Sadness (phpsadness.com) catalogued this as one of its canonical examples. Every PHP developer must either memorize the per-function convention or look it up. IDE tooling exists solely to surface argument names at call sites to compensate.
- **Receipt**: phpsadness.com entry #9 on argument order (SSL cert expired, URL dead); https://gist.github.com/salathe/1672543 — community audit of haystack/needle ordering across all PHP functions
- **Yinz status**: ALREADY SOLVED. Yinz uses dot-method-first convention universally (`value.method(args)` — receiver always first by syntax). PHP's haystack/needle problem is a procedural-function-argument-order problem that cannot occur in Yinz because there are no free stdlib functions; everything is `receiver.method(args)`.

---

### Finding #13: JavaScript `==` vs `===` — type coercion forced a second operator
- **Category**: syntax
- **Language**: JavaScript
- **What they did**: `==` performs type coercion before comparison: `"5" == 5` is `true`, `false == 0` is `true`, `null == undefined` is `true`. To get identity comparison, `===` was added to the spec.
- **When**: The coercive `==` shipped in 1995; `===` was added in ES3 (1999)
- **Why it became locked-in**: Removing `==` would break code that relies on the coercion. `===` is additive; both operators coexist forever. Every JavaScript linter (`eslint eqeqeq`) has a rule recommending `===` over `==`, which means they've been recommending workarounds for the original design for 25+ years.
- **The cost**: Every JavaScript education course devotes time to the `==`/`===` distinction. The Google JavaScript Style Guide and Airbnb Style Guide both ban `==`. Code review time spent catching `==` usage. TypeScript eliminates this class of bugs entirely by making cross-type comparison a compile error.
- **Receipt**: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/sort (MDN) — for the broader context of JS design decisions; https://codeworks.me/blog/how-is-different-from-in-javascript/ — the design explanation
- **Yinz status**: Already solved. `design/operators.md` explicitly states: "Yinz has no implicit coercion. The type system prevents comparing a `string` to a `number` at compile time. `===` solves a problem that doesn't exist in a statically-typed language."

---

### Finding #16: C++ `std::endl` flushes the buffer — silent perf trap
- **Category**: stdlib-api
- **Language**: C++
- **What they did**: `std::endl` is a stream manipulator that writes `\n` AND flushes the buffer. For a developer who thinks `endl` just means "end of line," every output operation flushes — degrading performance by orders of magnitude compared to just `'\n'`.
- **When**: C++ standard library (early 1990s) — present
- **Why it became locked-in**: `endl` is part of the C++ standard and changes would break existing code that intentionally flushes with every newline (e.g., interactive terminals, logging). `'\n'` has always been the correct alternative, but the more-readable name (`endl`) silently does more.
- **The cost**: clang-tidy ships a `performance-avoid-endl` check as a standard rule. cppreference explicitly warns: "Use of std::endl in place of '\n', encouraged by some sources, may significantly degrade output performance." The performance loss on buffered output (files, pipes) is proportional to output volume — any benchmark that writes many lines with `endl` gives an incorrect baseline.
- **Receipt**: https://clang.llvm.org/extra/clang-tidy/checks/performance/avoid-endl.html — clang-tidy rule; https://accu.org/journals/overload/27/149/sharpe_2619/ — "Don't Use std::endl" ACCU article; https://en.cppreference.com/w/cpp/io/manip/endl — cppreference warning
- **Yinz status**: Unaddressed specifically, but Yinz's `print()` always works without flushing semantics baked into the name. The lesson: output helpers must not silently perform expensive I/O operations the developer can't see from the name.

---

### Finding #17: C++ `>>` template closing bracket ambiguity
- **Category**: syntax
- **Language**: C++
- **What they did**: In C++ pre-2011, `vector<vector<int>>` failed to compile because the lexer parsed `>>` as the right-shift operator before the parser could resolve it as two closing angle brackets. Developers had to write `vector<vector<int> >` with a mandatory space.
- **When**: C++98 / C++03 (1998–2011); fixed in C++11
- **Why it became locked-in (for 13 years)**: The "maximum munch" lexer rule (parse the longest valid token) was baked into the spec. Fixing it required an exception to the maximum munch rule that applied when angle brackets were contextually open. The WG21 paper N1757 proposed the fix in 2005; it was adopted in C++11 (2011).
- **The cost**: 13 years of `vector<vector<int> >` syntax enforced by compilers. Every C++ codebase from 1998–2011 has this pattern. New developers inheriting old code encounter it and are confused. The fix required a context-sensitive exception to a fundamental lexer rule — a complexity that compounds with other template disambiguation rules.
- **Receipt**: https://www.open-std.org/jtc1/sc22/wg21/docs/papers/2005/n1757.html — the fix proposal N1757; https://cppreference.com/w/cpp/language/angle_bracket_hack.html — cppreference documentation of the hack; https://developeradventure.blogspot.com/2013/09/c11-right-angle-bracket.html — C++11 right angle bracket explanation
- **Yinz status**: Already solved. Yinz uses `<T>` for type parameters but has a clear lexer model that distinguishes type-parameter positions from comparison operators. The specific `>>` issue doesn't arise because Yinz's grammar is designed from scratch.

---

### Finding #18: C++ `[=]` lambda captures `this` by reference — silent dangling
- **Category**: syntax
- **Language**: C++
- **What they did**: In C++11–17, `[=]` means "capture all used variables by value." However, if `this` is used inside the lambda, it is captured by pointer (not by value), even under `[=]`. This violates the developer's reasonable assumption that `[=]` means everything by value. The captured `this` dangles if the lambda outlives the object.
- **When**: C++11 (2011) — the inconsistency was acknowledged; deprecated in C++20 (2020)
- **Why it became locked-in**: The behavior is in the C++11 and C++14 spec. Existing code relying on `[=]` capturing `this`-by-pointer would silently change behavior if the spec changed the semantics. C++20 deprecated `[=]` for implicit `this` capture; the old behavior still compiles with a warning.
- **The cost**: Class of dangling-pointer bugs specific to async/callback-heavy code that uses member lambdas. The fix (C++20 deprecation + explicit `[=, this]` or `[=, *this]`) requires a new syntax that most C++ books predating 2020 didn't cover.
- **Receipt**: https://www.learncpp.com/cpp-tutorial/lambda-captures/ — explicit deprecation note; https://en.cppreference.com/cpp/language/lambda — C++20 deprecation documented; https://medium.com/factset/modern-c-in-depth-lambdas-part-2-a2d54c7b51 — detailed analysis
- **Yinz status**: Already solved. Yinz closures capture by ownership rules (`.share`/`.lend`/`.give`/`.copy`). There is no implicit capture of a parent-scope reference. Dangling captured `this` is impossible.

---

### Finding #19: JavaScript `Array.prototype.sort()` — mutating by default
- **Category**: stdlib-api
- **Language**: JavaScript
- **What they did**: `Array.prototype.sort()` sorts the array in place and returns a reference to the same array. `Array.prototype.concat()`, `map()`, `filter()` return new arrays. There is no consistent mutability convention across array methods.
- **When**: 1995 — present; `toSorted()` added in ES2023 (2023) as the non-mutating counterpart
- **Why it became locked-in**: Changing `sort()` to be non-mutating would break any code that passes the sorted array to another function while also using the original variable (both are the same array, sorted). TC39 chose to add `toSorted()` / `toReversed()` / `toSpliced()` in ES2023 rather than change the original methods.
- **The cost**: 28 years of `arr.sort()` silently mutating the passed-in array. React, Redux, and all immutable-state-based frameworks must teach "copy before sort": `[...arr].sort()`. The TC39 proposal stage took several years; ES2023's non-mutating methods are not yet available in all environments (especially older mobile browsers).
- **Receipt**: https://v8.dev/features/stable-sort — V8 blog on sort stability; https://allthingssmitty.com/2025/09/08/finally-safe-array-methods-in-javascript/ — ES2023 toSorted overview; https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Array/sort — MDN mutation note
- **Yinz status**: Already solved. `design/collections.md` records that `.set()` is explicit mutation; read operations return new values. The mutation/immutation split is structural: mutating methods require a `lend` borrow; non-mutating methods require a `share` borrow. The difference is visible in the function signature.

---

### Finding #20: JavaScript `Array.prototype.sort()` — unstable sort spec (pre-ES2019)
- **Category**: stdlib-api
- **Language**: JavaScript / ECMAScript
- **What they did**: The ECMAScript spec prior to ES2019 did not require `Array.prototype.sort()` to be stable (i.e., preserve relative order of equal elements). V8 used an unstable QuickSort for arrays > 10 elements; SpiderFire used a different algorithm. The behavior was engine-dependent and array-size-dependent.
- **When**: 1995 — ES2019 (2019); fixed 24 years after the language shipped
- **Why it became locked-in**: The spec intentionally left algorithm choice to implementors for flexibility. Engine teams chose unstable algorithms for performance (QuickSort is cache-friendly). When TC39 finally mandated stability in ES2019, all major engines had to change their implementations.
- **The cost**: 24 years of silently incorrect sort results for arrays > 10 elements in Chrome. Applications relying on sort order for deterministic rendering or test comparisons produced different results in different browsers or at different array sizes. This category of bug is nearly impossible to unit test because unit tests typically use small arrays.
- **Receipt**: https://v8.dev/features/stable-sort — V8's announcement of stable sort and the spec change; https://www.dmcinfo.com/blog/25016/sorting-in-javascript-handling-google-chromes-unstable-sort/ — production impact description
- **Yinz status**: Already addressed by design philosophy. `design/collections.md` doesn't specify sort algorithms; when sort is implemented, stability should be mandated in the spec.

---

### Finding #21: Python `datetime` naive vs aware — `utcnow()` deprecated 30 years after the fact
- **Category**: stdlib-api
- **Language**: Python
- **What they did**: `datetime.datetime.now()` returns a naive datetime (no timezone info). `datetime.datetime.utcnow()` also returns a naive datetime that represents UTC, but nothing marks it as UTC. Passing a naive-UTC datetime to code that assumes naive-local datetimes silently produces wrong timestamps.
- **When**: Python's `datetime` module shipped with Python 2.3 (2003). `utcnow()` was deprecated in Python 3.12 (2023) — 20 years later.
- **Why it became locked-in**: Changing the default to timezone-aware would break code relying on naive datetime arithmetic. `utcnow()` had to be deprecated (not removed) rather than fixed because its behavior was by-design in the original API.
- **The cost**: Two decades of timezone bugs. The `pytz` library (third-party) exists because stdlib timezone support was inadequate. `dateutil` is another workaround library. PEP 495 (2015) added `fold` to handle DST ambiguity — a 12-year-later patch for a problem baked in at the API's design.
- **Receipt**: https://blog.miguelgrinberg.com/post/it-s-time-for-a-change-datetime-utcnow-is-now-deprecated — Miguel Grinberg's detailed analysis; https://peps.python.org/pep-0495/ — PEP 495 (fold attribute, 2015)
- **Yinz status**: Already solved. `design/stdlib/dates.md` explicitly documents timezone-aware design: all date objects carry timezone; conversion methods are explicit (`.inTimeZone()`, `.toUTC()`, `.toLocal()`).

---

### Finding #22: Lua 1-indexed arrays
- **Category**: stdlib-api
- **Language**: Lua
- **What they did**: Lua uses 1-based array indexing. `t[1]` is the first element. Standard library functions (`ipairs`, `table.insert`, `#t` length operator) all assume 1-based indexing.
- **When**: Lua 1.0 (1993) — present
- **Why it became locked-in**: The standard library is built on 1-based convention. `ipairs` iterates from 1 to `#t`. `table.insert(t, x)` appends to position `#t + 1`. Building 0-based arrays breaks ipairs, the length operator, and all the standard table functions. GitHub user USN484259 shipped a "Lua0" fork with 0-based indexing as a proof of concept; it required changing the entire runtime.
- **The cost**: Every Lua library and every Lua tutorial uses 1-based indexing. Code that interfaces with C (which is 0-based) at FFI boundaries must add/subtract 1. The debate resurfaces in every Lua community forum thread annually. No fix is planned; the Lua team explicitly defends 1-based indexing as a design choice for non-programmer audiences.
- **Receipt**: https://github.com/a327ex/blog/issues/68 — "Why 1-based indexing is right for Lua" community debate; http://lua-users.org/wiki/CountingFromOne — lua-users wiki; https://news.ycombinator.com/item?id=43434630 — HN thread 2025
- **Yinz status**: Already solved. Collections use 0-based indexing by convention (design/collections.md uses `players[3]` as example; standard 0-based). The date API uses 1-based months explicitly (following human convention) but that is months, not array indices.

---

### Finding #23: Lua globals-by-default — silent `local` omission bugs
- **Category**: syntax
- **Language**: Lua
- **What they did**: Variables in Lua are global by default. `x = 5` creates a global. `local x = 5` creates a local. Forgetting `local` silently pollutes the global namespace.
- **When**: Lua 1.0 (1993) — partially addressed in Lua 5.5 (December 2025) with optional enforcement
- **Why it became locked-in**: Changing the default to local would break all existing Lua code. Lua 5.5 (2025) added `<close>` and stricter options but the global default remains. The community workaround is `strict.lua`, a script that installs a metatable to trap unexpected global reads. This pattern has existed for 20+ years rather than fixing the language.
- **The cost**: A typo in a variable name creates a silent global (reads return `nil` instead of an error). In embedded Lua environments (game scripting, Nginx config), a global collision can corrupt unrelated state silently. The `strict.lua` workaround is not included in the stdlib and must be explicitly loaded.
- **Receipt**: https://fixthisbug.de/en/content/bugs/lua/global-variables — fix documentation; http://lua-users.org/wiki/ScopeTutorial — scope tutorial acknowledging the footgun; https://www.lua.org/pil/14.2.html — official "declaring global variables" documentation
- **Yinz status**: Already solved. Yinz has no implicit globals. `design/scope.md` prohibits mutable file-level variables entirely; everything is either block-scoped `let`/`const` or explicitly exported.

---

### Finding #24: Go `:=` short variable declaration — confusing redeclaration rules
- **Category**: syntax
- **Language**: Go
- **What they did**: Go has two assignment operators: `=` (assign to existing) and `:=` (short variable declaration, infers type). The `:=` operator has a quirk: it only requires one new variable on the left side. `x, err := foo()` declares both `x` and `err` as new. Then `y, err := bar()` declares only `y` as new — `err` is reused from the outer scope. This interacts unexpectedly with loop variables.
- **When**: Go 1.0 (2012) — present
- **Why it became locked-in**: `:=` is used in virtually all Go code (it is the idiomatic way to declare variables). Changing its semantics would break the Go ecosystem. The redeclaration rule is documented but not obvious to new Go developers.
- **The cost**: The `:=` redeclaration rule is listed in every Go "common gotchas" article. In particular, the interaction with `err` in multiple sequential `:=` declarations can shadow errors from outer scopes when developers expect a new `err` variable.
- **Receipt**: https://www.godesignpatterns.com/2014/04/assignment-vs-short-variable-declaration.html — `:=` vs `=` design patterns; https://medium.com/swlh/stuff-i-wish-id-known-sooner-about-variable-assignments-in-golang-f00317b57bcb — "Stuff I Wish I'd Known" on Go variables
- **Yinz status**: Already solved. Yinz uses `let` for declaration (always explicit) and `=` for reassignment. No dual-purpose operator. The type-inference case uses explicit `let x = foo()` — same syntax, inferring the type from the right-hand side.

---

### Finding #25: Go `iota` — terse constants with silent ordering dependencies
- **Category**: syntax
- **Language**: Go
- **What they did**: `iota` is a per-const-block counter. Inside a `const (...)` block, the first `iota` is 0, next is 1, etc. Reordering constants in the block changes their numeric value, and if those values were serialized to a database, the data is silently corrupted.
- **When**: Go 1.0 (2012) — present; a proposal for exhaustive `switch` type-safety (#36387) was opened and rejected
- **Why it became locked-in**: `iota` is the only enum-like feature Go has. Providing a native `enum` keyword would have made `iota` redundant — but `iota` is used in millions of lines of existing code. GitHub proposal #36387 requested exhaustive switch safety for iota-based enums; it was rejected as "too complex to add to the spec."
- **The cost**: Reordering a const block to add a new constant in the middle silently changes all subsequent values. Any serialized iota value (database column, network protocol, config file) corrupts if the ordering changes. There is no compiler warning. Every Go codebase with iota-based constants must treat them as permanently ordered — or document them as stable values and hand-assign the numbers, which defeats iota's purpose.
- **Receipt**: https://github.com/golang/go/issues/36387 — exhaustive switch proposal rejected; https://bitfieldconsulting.com/posts/iota — "The smallest thing in Go" with ordering-danger analysis; https://news.ycombinator.com/item?id=39564737 — "Go Enums Suck" HN thread
- **Yinz status**: Already solved. `design/type-system.md` and `.claude/rules/naming.md` describe `options` as the named-constant mechanism: `options Status { active, inactive, banned }`. Values are named, not positional integers. Reordering the declaration doesn't change semantics.

---

### Finding #26: Rust `macro_rules!` vs proc-macro — two permanent macro systems
- **Category**: syntax
- **Language**: Rust
- **What they did**: Rust shipped `macro_rules!` (declarative macros) in Rust 1.0 (2015). Procedural macros (proc-macros) — which operate on token streams and support `#[derive(...)]` and attribute macros — shipped in Rust 1.15 (2017) for custom derives, and stabilized fully in Rust 1.30 (2018). The two systems have different syntax, different capabilities, different mental models, and different tooling.
- **When**: `macro_rules!` Rust 1.0 (2015); proc-macros Rust 1.30 (2018). Both permanent.
- **Why it became locked-in**: RFC 1584 acknowledged "there are several changes to the declarative macro system which are desirable but not backwards compatible," which is why a new system was proposed rather than changing `macro_rules!`. The Rust 1 stability guarantee prevents removing `macro_rules!`. There is also a third system under development (declarative macros 2.0 / `macro`) that would replace `macro_rules!` but not proc-macros — resulting eventually in three systems.
- **The cost**: Two conceptually different systems that beginners must navigate. `macro_rules!` uses pattern matching on syntax trees with unusual syntax (`$( $x:expr ),*`). Proc-macros require a separate crate, use the `proc_macro` API, and interact with the token stream. There is no single place to learn "how Rust macros work."
- **Receipt**: https://rust-lang.github.io/rfcs/1584-macros.html — RFC 1584 "Macros" (2016) explicitly acknowledges the incompatible-changes problem; https://rust-lang.github.io/rfcs/1566-proc-macros.html — RFC 1566 proc-macro RFC; https://doc.rust-lang.org/book/ch20-05-macros.html — the Rust Book section distinguishing the systems
- **Yinz status**: Unaddressed. Yinz has no macro system defined. This is a deferred design space, but the lesson is: if you ship a macro system at all, commit to one model and don't layer a second one on top.

---

### Finding #27: JavaScript ASI (Automatic Semicolon Insertion) — permanent ambiguity tax
- **Category**: syntax
- **Language**: JavaScript
- **What they did**: The ECMAScript parser inserts semicolons automatically at line breaks when it can form a valid statement. The rules are complex enough that `return\n{...}` returns `undefined` (semicolon inserted after `return`), not the object literal. The famous `return` + object-literal-on-next-line bug silently returns `undefined`.
- **When**: 1995 — present; ASI is in the core ECMAScript spec and cannot be removed
- **Why it became locked-in**: All existing semicolon-free code (a large fraction of the Node.js ecosystem) depends on ASI. The ECMAScript spec defines exact ASI rules; changing them would break the web. The ongoing "semicolons vs no semicolons" style debate exists entirely because ASI makes both approaches valid — but not equivalent.
- **The cost**: The `return {}` gotcha is one of the most cited JavaScript WTFs. Every linter (ESLint `no-unexpected-multiline`) has rules for ASI edge cases. Two dominant style guides (Airbnb vs Standard.js) disagree on semicolons because both are valid. Code review time spent enforcing a per-project convention that shouldn't need to exist.
- **Receipt**: https://inimino.org/~inimino/blog/javascript_semicolons — the most cited ASI analysis; https://wikibooks.org/wiki/JavaScript/Automatic_semicolon_insertion; https://2ality.com/2011/05/semicolon-insertion.html — Dr. Axel Rauschmayer's analysis
- **Yinz status**: Already solved. Yinz requires explicit statement terminators (newlines as terminators per the current grammar design). No ASI.

---

### Finding #28: Python `GIL` — implementation detail that became de-facto specification
- **Category**: stdlib-api
- **Language**: Python (CPython)
- **What they did**: The Global Interpreter Lock (GIL) is a mutex in CPython that prevents multiple threads from executing Python bytecode simultaneously. It was an implementation detail for memory safety in the reference-counting GC. Over time, C extensions and the numpy ecosystem were written assuming single-threaded Python execution, making the GIL load-bearing across the ecosystem.
- **When**: CPython early implementation — present; PEP 703 "Making the GIL Optional" accepted in 2023; free-threaded experimental mode in Python 3.13 (2023), advancing in 3.14 (2025)
- **Why it became locked-in**: The GIL is not in the language spec but became de-facto spec because the C extension ecosystem (numpy, scipy, PIL, CPython bindings for BLAS, etc.) makes thread-safety assumptions based on the GIL's presence. Removing it requires making all C extension authors re-examine and potentially rewrite their thread-safety models.
- **The cost**: Python cannot use multiple CPU cores for CPU-bound work without multiprocessing (not threads), which requires IPC overhead and memory duplication. This forces Python to use workers/processes for parallelism rather than threads. PEP 703's own authors admit the no-GIL build has slower single-threaded performance than the GIL build. As of Python 3.13, the free-threaded build is experimental and the default still uses the GIL.
- **Receipt**: https://peps.python.org/pep-0703/ — PEP 703 with the acceptance and implementation challenges; https://discuss.python.org/t/a-steering-council-notice-about-pep-703-making-the-global-interpreter-lock-optional/30474 — steering council notice
- **Yinz status**: Unaddressed (no runtime designed yet). But relevant: Yinz's concurrency model (`design/concurrency.md`) is compile-time ownership — thread-safety is enforced by ownership, not a runtime lock. No GIL is possible or needed.

---

### Finding #29: Node.js callbacks-first design — ecosystem rewrite took a decade
- **Category**: stdlib-api
- **Language**: Node.js / JavaScript
- **What they did**: Node.js shipped (2009) with an error-first callback convention as the primary async pattern: `fs.readFile(path, (err, data) => {...})`. The Node.js standard library was built entirely on this convention. Promises existed in libraries but were not built into the Node.js stdlib.
- **When**: Node.js 0.1 (2009); native Promises added to JS spec ES6 (2015); Node.js `util.promisify` added Node 8 (2017); `async/await` in Node 8 (2017); `fs.promises` API added Node 10 (2018) — 9 years after the original callback-based `fs`.
- **Why it became locked-in**: The entire npm ecosystem (at the time, hundreds of thousands of packages) used callbacks. Changing the stdlib to return promises would have broken all existing code. Instead, parallel promise-based APIs were added alongside the callback APIs. Both exist in Node.js today.
- **The cost**: "Callback hell" became a canonical term in software engineering. Entire books were written about managing nested callbacks. A billion lines of Node.js code needed migration when promises became idiomatic. Two parallel Node.js stdlib APIs (callback-based `fs` and promise-based `fs.promises`) now coexist permanently.
- **Receipt**: https://callbackhell.com/ — the canonical callback hell manifesto; https://blog.risingstack.com/node-js-async-best-practices-avoiding-callback-hell-node-js-at-scale/ — migration best practices; https://dzone.com/articles/from-callbacks-to-async-await-a-migration-guide — migration guide
- **Yinz status**: Already solved. Yinz's concurrency design has no callbacks and no async/await function coloring. `design/future/concurrency.md` implements whole-program may-block analysis so `wait` (when needed) is inferred, not propagated through function signatures.

---

### Finding #31: Ruby optional parentheses — readability and parsing ambiguity
- **Category**: syntax
- **Language**: Ruby
- **What they did**: Ruby allows parentheses to be omitted on method calls. `foo bar, baz` calls `foo` with two arguments. `foo bar baz` is a syntax error. `foo bar(baz)` calls `bar(baz)` and passes the result to `foo`. Edge cases are numerous and parser-dependent.
- **When**: Ruby 1.0 (1995) — present
- **Why it became locked-in**: Optional parens are a core Ruby aesthetic — Matz explicitly designed this to make Ruby read "like English." Removing the ability to omit parens would break millions of lines of Ruby and DSL-style code (Rails routes, RSpec matchers, etc.).
- **The cost**: A SyntaxError is produced when the parser can't resolve which method an argument belongs to. The rule "use parentheses to avoid ambiguity" was established early, meaning some teams have coding standards banning the paren-free style. The community produced a canonical blog post "Revoking the (Parentheses) Privilege" about making parens mandatory in a codebase.
- **Receipt**: https://gmalette.dev/posts/revoking-the-parentheses-privilege/ — "Revoking the Parentheses Privilege"; https://bugs.ruby-lang.org/issues/18396 — Ruby bug caused by parens-free syntax with hash-value omission; https://whydavewhy.com/2013/09/05/parentheses-in-ruby/ — the parentheses debate
- **Yinz status**: Already solved. Yinz requires parentheses for all function calls. There is no paren-optional syntax.

---

### Finding #32: TypeScript `as any` escape hatch — type-safety erasure that spreads
- **Category**: syntax
- **Language**: TypeScript
- **What they did**: TypeScript provides `as any` as a cast that disables all type checking for a value. Any subsequent operations on the value are unchecked. A value typed `any` propagates its unchecked nature when passed to functions — the called function receives `any` and loses type safety for that parameter.
- **When**: TypeScript 1.0 (2012) — present
- **Why it became locked-in**: `as any` is intentional — TypeScript was designed to be progressively adopted in JavaScript codebases where some code is untyped. Removing it would prevent incremental migration. `unknown` (added TypeScript 3.0, 2018) is the safer alternative, but `any` remains because it's simpler for "I know what I'm doing" situations.
- **The cost**: In large codebases, `any` propagates. One team reported reducing `any` usage from 200+ instances to under 10, with a 30% decrease in bug reports as a result. The `@typescript-eslint/no-explicit-any` and `@typescript-eslint/no-unsafe-assignment` ESLint rules exist specifically because `any` is both easy to reach for and dangerous. The rules have to be manually enabled — the default TypeScript compiler accepts `any` everywhere.
- **Receipt**: https://www.explainthis.io/en/swe/typescript/10-any-type — escape hatch analysis; https://medium.com/@sohail_saifi/the-fatal-typescript-patterns-that-make-senior-developers-question-your-experience-8d7f10a3be42 — "200 `any` uses → 30% fewer bugs" case study; https://typescript-eslint.io/rules/no-explicit-any/ — the lint rule
- **Yinz status**: Already solved. Yinz has no type escape hatch. The type system is the contract; `unknown` as a concept does not exist because type narrowing is the only path. See `.claude/rules/coding-style.md` for the explicit no-`any` policy.

---

### Finding #33: Go `string` indexing returns bytes, not characters
- **Category**: stdlib-api
- **Language**: Go
- **What they did**: `s[i]` returns the byte at index `i`, not the Unicode character at index `i`. `len(s)` returns byte count, not character (rune) count. On ASCII-only strings, bytes and characters coincide. On any string with non-ASCII characters, `s[i]` returns a partial UTF-8 byte, not a character.
- **When**: Go 1.0 (2012) — present
- **Why it became locked-in**: The Go team's position is documented in the official blog post "Strings, bytes, runes and characters in Go": "Go source code is always UTF-8. A string holds arbitrary bytes." Changing `s[i]` to return a rune would change the semantics for performance-sensitive byte-processing code that deliberately processes bytes. Go forces explicit conversion to `[]rune` for character-based indexing.
- **The cost**: A commonly cited Go gotcha. `for i, c := range s` iterates by rune (correct for Unicode); `s[i]` indexes by byte (wrong for Unicode). Every Go developer must learn the distinction. Bugs in non-ASCII string handling are common in Go codebases written by developers who didn't read the strings blog post. `utf8.RuneCountInString(s)` is needed for character count; `len(s)` gives byte count.
- **Receipt**: https://go.dev/blog/strings — the official Go blog post; https://dev.to/hugohenrick/understanding-strings-in-go-bytes-runes-and-the-truth-behind-len-3818 — developer analysis; https://janert.me/blog/2024/go-is-weird-strings/ — "Go is Weird: Strings" critique
- **Yinz status**: Already solved. `design/collections.md` explicitly documents that `s[n]` is by Unicode code point (`.get(n)` returns `maybe string`), with explicit escape valves for byte access (`.byteAt(n)`) and grapheme access (`.graphemeAt(n)`). The default is the human-intuitive behavior.

---

### Finding #34: Erlang punctuation hell — `,` `;` `.` semantics locked in
- **Category**: syntax
- **Language**: Erlang
- **What they did**: Erlang uses three different punctuation marks as statement terminators with different semantics: `.` (period) ends a top-level declaration, `;` (semicolon) separates clauses within a function or case expression, `,` (comma) separates sequential expressions within a clause. Confusing them produces a syntax error; the error message does not always point to the right location.
- **When**: Erlang OTP 1 (1987) — present; Elixir (2012) eliminated this by using `do/end` blocks
- **Why it became locked-in**: Erlang's punctuation is fundamental to its grammar. Every piece of Erlang code uses all three. Changing the rules would require rewriting the Erlang parser and all existing Erlang code. Elixir was designed as a clean-break language on the BEAM VM precisely to escape this (among other ergonomic issues).
- **The cost**: A 2007 Erlang-questions mailing list thread titled "[erlang-questions] Comma, semicolon. Aaaaa" captures the frustration. The Erlang wiki has a canonical "common mistakes" page that lists semicolon/period confusion as the #1 syntax error new developers make. The very existence of Elixir as a separate language on the same VM is partially attributable to Erlang's syntax being too difficult to clean up.
- **Receipt**: https://erlang-questions.erlang.narkive.com/0E1fJntW/comma-semicolon-aaaaa — the mailing list thread; https://sadraskol.com/posts/the-alien-erlang-syntax-choices/ — analysis of Erlang syntax choices; http://erlang.org/pipermail/erlang-questions/2007-September/029250.html — original mailing list
- **Yinz status**: Already solved. Yinz uses block structure (`{}`) and newlines for statement separation. No punctuation-overloaded terminators.

---

### Finding #35: C++ `auto` keyword repurposed — two incompatible meanings
- **Category**: syntax
- **Language**: C++
- **What they did**: In C98/C++03, `auto` was a storage-class specifier meaning "automatic (stack) storage" — the default for all local variables, so it was completely redundant and almost never used. C++11 repurposed `auto` to mean "infer the type from the initializer." Any C/C++ code that used the old meaning (rare but possible) broke in C++11.
- **When**: `auto` as storage class: C89, C++98; `auto` as type inference: C++11 (2011); C23 also adopted type-inference `auto` from C++
- **Why it became locked-in**: C++11 made a deliberate breaking change to `auto` semantics, betting that the old usage was rare enough that the breakage would be acceptable. The new meaning is now universal. But the breakage was real — there are C98/C++03 codebases with `auto int x = 5;` that required remediation.
- **The cost**: The old meaning caused a small but real migration tax. The new meaning introduces subtler issues: `auto` with proxy types (e.g., `std::vector<bool>::reference`) gives a dangling reference. `auto` with expression templates gives a temporary bound to a non-const reference. clang-tidy has rules specifically for `auto` misuse in these contexts.
- **Receipt**: https://learn.microsoft.com/en-us/cpp/cpp/auto-cpp?view=msvc-170 — MSVC documentation of the dual meaning; https://tech-champion.com/programming/cpp/dangers-of-c-auto-keyword-pitfalls-and-best-practices/ — dangers analysis; https://software.codidact.com/posts/291981 — community analysis
- **Yinz status**: Unaddressed directly, but Yinz doesn't reuse existing keywords. Type inference uses `let x = foo()` — `let` is unambiguously "declare a variable" with inferred type. No repurposed keyword.

---

## Convergent themes

**1. Retrofitting timezone correctness costs 10–20 years.** JavaScript `Date`, Java `java.util.Date`, Python `datetime.utcnow()` all shipped naive/incorrect datetime APIs in the 1990s–2000s and required decade-long migrations to correct. The common failure mode: timezone awareness was optional at construction time, making it easy to forget.

**2. Parallel APIs are permanent.** Every language that added a "better" API alongside an old one (Java `java.io`/`java.nio`, Python `os.path`/`pathlib`, Go `sort.Sort`/`slices.Sort`, Node.js callbacks/promises, Python `%`/`.format()`/f-strings) still ships both. The old API never goes away. Corollary: get it right the first time, because "we'll replace it later" means "we'll ship two things forever."

**3. Implicit type coercion/confusion forces a second operator or API.** JavaScript's `==` needed `===`. Go's string indexing needed `[]rune` cast and `utf8.RuneCountInString`. Python's mutable defaults needed `None`-as-sentinel pattern. The "clever" implicit behavior forces a defensive programming pattern that must be taught to every new developer.

**4. Opaque or symbolic primitives become load-bearing.** Go's date format magic numbers, `iota`'s positional encoding, Erlang's `;`/`,`/`.` semantics, PHP's haystack/needle ordering — all of these require memorization with no recoverable mnemonic. Once enough code relies on the symbol, the symbol is permanent.

**5. Runtime I/O hidden in pure-looking operations is catastrophically expensive.** Java's `URL.equals()` doing DNS lookup is the extreme version; C++'s `endl` doing a buffer flush is the subtle version. Any stdlib operation whose name implies a pure read but performs I/O under the hood will be called in hot paths.

**6. First-mover API decisions in async/concurrency lock the ecosystem.** Node.js callbacks, Rust async, Java checked exceptions — the first concurrency/error model an ecosystem standardizes on becomes the ecosystem's type system, and every library that adopts it must be migrated when the model changes.

**7. Escape hatches to the type system propagate.** `as any` in TypeScript, `unsafe {}` in Rust, `# type: ignore` in Python — any escape hatch that's easy to reach for will infect codebases. The infection is irreversible at scale.

---

## Languages I skipped and why

- **Scala**: The language is a known complexity showcase, but Scala's design issues are primarily type-system complexity and compile-time performance, not stdlib or syntax lock-ins in the "other languages chose wrong" sense. The Scala community knows the tradeoffs and accepts them.
- **Haskell**: Whitespace/layout rules and Prelude design are real issues, but Haskell's user base is sophisticated enough that these are design tradeoffs, not the class of "wrong decision baked in forever" this document targets.
- **Swift**: Swift has forced ongoing evolution (Swift 3 ABI-broke everything, Swift 5 stabilized ABI). The evolution is active enough that lock-ins are being addressed. No single frozen mistake comparable to the others here.
- **Kotlin**: Designed explicitly to avoid Java's mistakes. Its mistakes are largely "Kotlin-specific novelties that didn't pan out" (e.g., SAM conversions' interaction with overload resolution), not structural lock-ins.
- **Nim, Crystal, Zig**: Too young to have the kind of "permanent mistake because of backwards compat" pattern this document targets. Zig's function argument ordering (`std.mem.eql(u8, s1, s2)`) is a candidate, but Zig has not reached 1.0 stability.
- **F# / OCaml**: `;` vs `;;` in OCaml is a real pain point but it affects primarily REPL users. The core language design is sound.
- **Perl**: The entire language is a historical curiosity; sigil-context overloading is well-known but Perl's adoption is declining rather than locked-in. The lesson (don't overload sigils on context) is captured more cleanly in other findings.
