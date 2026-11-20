---
name: "SCRATCH-stdlib-encoding"
description: "The codec module for turning bytes into safe text and back: Base64, hexadecimal, and URL/percent encoding. This is the 'convert binary to something you can put in a string / URL / header' layer."
tags:
  - "yinz-compiler"
created_at: "2026-06-13"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# `encoding` — Base64, Hex, URL Encoding

> **Status**: Direction locked (2026-06-13), API shape designed just-in-time at its version turn.
> **Spec**: written when the module ships (a `docs/reference/REF-*.md` file shows how to use a shipped feature; see [`.claude/rules/docs-checklist.md`](../../../.claude/rules/docs-checklist.md)).
> **Audience**: contributors — design rationale, not a usage guide.

The codec module for turning bytes into safe text and back: Base64, hexadecimal, and URL/percent encoding. This is the "convert binary to something you can put in a string / URL / header" layer.

---

## Why this is a module (and why it's overdue)

Two existing rules already *depend* on a module that wasn't on the list:

- [`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md) names **base64 encode/decode as a SIMD touchpoint** (its SIMD bullet). It assumes an encoding module exists.
- `request` (v0.15) needs **URL/percent-encoding** to build query strings and encode path segments safely. HTTP basic-auth headers need base64.

So the floor for HTTP, auth headers, data URIs, and any "stuff bytes into text" task was missing. This module is that floor.

---

## Scope

| Codec | Encode | Decode | Notes |
|---|---|---|---|
| **Base64** | bytes → text | text → bytes (`errors`) | Standard alphabet (`+/`) AND URL-safe alphabet (`-_`); padding configurable but defaults fixed (same defaults on every platform) |
| **Hex** | bytes → text | text → bytes (`errors`) | Lowercase default; decode rejects odd-length / non-hex |
| **URL / percent** | text → percent-encoded | percent-encoded → text (`errors`) | Component-encoding (path segment / query value), not full-URL parsing |

**Out of scope (lives elsewhere):**
- **URL *parsing*** (split into scheme/host/path/query) is a `network` concern, not a codec — it belongs in `network.md`, not here. This module only does the percent-encoding *codec*.
- **Base32 / Base58** — not in the initial scope; add only if a real use case (e.g. a crypto address format) shows up. Noted so it's a conscious omission, not forgotten.

---

## Design rules this module obeys

Per [`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md):

- **Plain-read names stay pure:** `.encode()` / `.decode()` are pure transforms — no I/O, no allocation that's observable beyond the returned value. `.decode()` *can fail* on malformed input, so it carries `errors` (the failure is surfaced, not swallowed).
- **One API per capability:** one canonical `encode`/`decode` per codec. No `encodeFast` parallel API — SIMD is automatic (below), not a separate entry point.
- **Same defaults on every platform:** the alphabet, padding, and hex case are the same on every platform. The URL-safe and standard base64 forms are explicit named codecs, never a silent platform default.
- **SIMD for byte-touching operations:** base64 and hex encode/decode touch every byte, so they use SIMD intrinsics on x86_64/ARM64 with a scalar fallback. This is the documented default for production builds, not an opt-in.

---

## Illustrative API (PROPOSED — not yet implemented)

Dot-first, self-documenting, module name lowercase per [`.claude/rules/vocabulary.md`](../../../.claude/rules/vocabulary.md):

```
// ILLUSTRATIVE future syntax — final shape decided at the module's version turn.

const text  = encoding.base64.encode(bytes)            // bytes -> `SGVsbG8=`
const back  = encoding.base64.decode(text)             // text  -> bytes; errors on bad input
const safe  = encoding.base64url.encode(bytes)         // URL-safe alphabet (-_)

const hexed = encoding.hex.encode(bytes)               // -> `48656c6c6f`
const raw   = encoding.hex.decode(hexed)               // errors on odd-length / non-hex

const q     = encoding.url.encode(`a b&c`)             // -> `a%20b%26c` (query-value safe)
const plain = encoding.url.decode(q)                   // errors on malformed % sequence
```

The byte-buffer type these operate on ties to the `{ptr, len}` string/bytes representation in [`docs/internal/scratchpad/SCRATCH-future-string-ptr-len-overhaul.md`](SCRATCH-future-string-ptr-len-overhaul.md) — the exact `bytes` type is settled there/at-version-turn, not here.

---

## Auto-promotion analysis

Per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md):
- **Stricter/faster form?** SIMD vs scalar — but that's not a *user-visible* form (no source-level choice), so it's pure codegen, always-on, no muted hint and no lint. The compiler emits the SIMD path on supporting targets automatically (this IS the SIMD requirement).
- **No user-facing auto-promotion surface** (no typeable "fast encode" the user would write), so the muted-hint + Tier-3-lint surfaces don't apply. Stated explicitly so reviewers know it was considered.

---

## Teaching errors (WHAT / WHAT-INSTEAD / WHY per Golden Rule 3)

`.decode()` failures must teach, e.g. a bad base64 length:
- **WHAT**: the input isn't valid base64 (length isn't a multiple of 4 after padding).
- **WHAT INSTEAD**: check the source produced standard base64, or use `encoding.base64url.decode` if it came from a URL-safe context.
- **WHY**: base64 packs 3 bytes into 4 characters, so a valid stream's length is always a multiple of 4 — a different length means the data was truncated or isn't base64.

---

## Cross-references

- [`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md) — this module's contract; its SIMD bullet mandates the SIMD path
- [`docs/internal/scratchpad/SCRATCH-stdlib-network.md`](SCRATCH-stdlib-network.md) — URL *parsing* (distinct from the percent-encoding codec here); `request` (v0.15) consumes `encoding.url`
- [`docs/internal/scratchpad/SCRATCH-future-string-ptr-len-overhaul.md`](SCRATCH-future-string-ptr-len-overhaul.md) — the `bytes`/`{ptr,len}` representation these codecs operate on
- [`docs/internal/scratchpad/SCRATCH-stdlib-overview.md`](SCRATCH-stdlib-overview.md) — the module inventory + version sequence (slot TBD)
