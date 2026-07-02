# Feature Registry Rule

**Load when**: any milestone plan or code change that (a) adds a new language keyword, banned-jargon word, primitive method, type-attached constant, reserved/deferred feature, diagnostic template, or muted-hint domain to the compiler; OR (b) touches [`registry/features.toml`](../../registry/features.toml) or `crates/ynz-registry/`; OR (c) adds new user-facing feature inventories to `crates/ynz-{diagnostics,typeck,parser}/src/`.

**Does NOT apply to**: test-only code, compiler-internal bookkeeping with no user-facing surface, pure refactors that move existing code without adding features.

---

## The Rule

> **Every new user-facing feature item goes in [`registry/features.toml`](../../registry/features.toml) first. Code follows the registry — the registry does not follow the code.**

See [`docs/internal/implementation/IMP-feature-registry.md`](../../docs/internal/implementation/IMP-feature-registry.md) for the full schema reference and carve-out policy.

---

## Required Entry Types Checklist

When adding a new language feature, check each row:

| What you're adding | Required registry entry kind | Notes |
|---|---|---|
| New keyword the lexer recognizes as a token | `[[keyword]]` | |
| New keyword from another language that Yinz rejects | `[[banned_declaration_keyword]]` | Error text in `what_instead` + `why` |
| New jargon word banned from error messages | `[[banned_jargon]]` | Replacement word required |
| New primitive method (int/float/number/bool/string) | `[[primitive_intrinsic]]` | Include all overloads |
| New free-standing function (range, etc.) | `[[primitive_intrinsic]]` kind = "free_fn" | Include all overloads |
| New type-attached constant (e.g., `int.max`) | `[[type_attached_constant]]` | BOTH typeck value_type AND codegen value_literal required |
| New reserved-but-deferred language feature | `[[deferred_language_feature]]` | WHY, SUBSTITUTE, SHIPS_IN, DESIGN_DOC all required |
| New reserved-but-deferred tooling feature | `[[deferred_tooling_feature]]` | Same |
| New canonical diagnostic template | `[[diagnostic_template]]` | Only for reusable/canonical messages; per-site dynamic messages stay in code |
| New IDE muted-hint inference domain | `[[muted_hint_domain]]` | `placement_category` must match [`.claude/rules/inference.md`](inference.md) |

---

## Carve-Out — When NOT to Add a Registry Entry

A Rust constant in the crate is fine WITHOUT a registry entry when ALL of:
- The item is `#[cfg(test)]` — test-only. Registry is for production surface only.
- OR: the item is compiler-internal only — no error messages reference it, no IDE uses it.

Any carve-out that MIGHT be mistaken for a scattered registry violation MUST have a `// CARVE-OUT: <reason>` comment on the definition line.

If unsure whether an item needs a registry entry, the answer is yes. Excess registry entries are easy to remove; drift is hard to detect.

---

## Bouncer Pattern

The [`.claude/graveyard.md`](../graveyard.md) "scattered-registry-without-SSOT" entry runs a grep check on every diff. It catches new `pub const`/`pub static` string-array definitions in `crates/ynz-{diagnostics,typeck,parser}/src/` that lack a `// CARVE-OUT:` annotation. If the Bouncer fires, either:
1. Add the registry entry (preferred), OR
2. Add a `// CARVE-OUT: <reason>` comment explaining why this specific constant is exempt.

---

## Cross-References

- [`docs/internal/implementation/IMP-feature-registry.md`](../../docs/internal/implementation/IMP-feature-registry.md) — full schema + carve-out policy + self-hosting migration
- [`registry/features.toml`](../../registry/features.toml) — the TOML data file (source of truth)
- `crates/ynz-registry/` — the crate that parses TOML + generates Rust
- [`.claude/graveyard.md`](../graveyard.md) — "scattered-registry-without-SSOT" Bouncer entry
- [`.claude/rules/plan-invariants.md`](plan-invariants.md) — `### Feature Registry Entries` subsection (required for v0.2-M2+ plans)
