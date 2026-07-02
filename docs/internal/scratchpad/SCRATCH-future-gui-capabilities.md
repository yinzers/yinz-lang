---
name: "SCRATCH-future-gui-capabilities"
description: "The capability layer is where 80% of the real GUI engineering lives, and it never fully 'ends' — every OS version changes device APIs. This is the hard, ongoing part, and it's exactly the kind of problem Yinz is good..."
tags:
  - "yinz-compiler"
created_at: "2026-06-13"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Device Capabilities & Permissions

> **Status**: Direction locked, API shape open (decided at implementation time).
> **Audience**: contributors.

The capability layer is where 80% of the real GUI engineering lives, and it never fully "ends" — every OS version changes device APIs. This is the hard, ongoing part, and it's exactly the kind of problem Yinz is good at: a clean unified surface over messy per-platform reality (Golden Rule 4).

---

## The three pieces of every capability

Each device feature (camera, mic, speaker, touch, GPS, notifications, biometrics, clipboard, filesystem, haptics…) has the same three-layer shape:

1. **One unified Yinz API** — dot-first, self-documenting, the same on every platform. The jr dev writes this once and it works everywhere:
   ```
   // ILLUSTRATIVE — proposed future syntax, not yet implemented
   const photo = camera.capture() errors      // errors surfaces "denied" / "no device"
   const here  = location.current() errors
   haptics.buzz(duration.ms(50))
   ```
2. **Per-platform native implementations** behind that surface — iOS bridges to AVFoundation/CoreLocation, Android to the Android SDK via JNI, desktop to OS APIs, web to the browser's `MediaDevices`/`Geolocation`. The compiler/stdlib picks the right one per target; the user never sees the switch.
3. **A permission model** — iOS usage strings (Info.plist), Android manifest + runtime grants, browser permission prompts.

---

## Permissions are compile-checked, denials are typed

This is the Yinz angle that other stacks lack:

- **Undeclared capability = compile error.** A capability your app uses but didn't declare in its manifest fails at *compile* time (Golden Rule 5, compile-time safety), with a teaching diagnostic that names the capability and the per-platform declaration it generates. No "works on my machine, crashes on the user's phone because Info.plist was missing a string."
- **A runtime denial is a typed `errors` value, not a silent null.** `camera.capture() errors` — if the user denied camera access, the caller *must* handle the denial (the `errors` keyword forces it). No null-deref, no swallowed failure.

The capability declaration is the single source of truth: the compiler reads it to (a) emit the right per-platform permission metadata (Info.plist entries, Android manifest permissions), and (b) gate the API surface so undeclared use can't compile.

---

## Stdlib-rule compliance (mandatory)

The capability API must follow [`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md):

- **Pure-named methods are pure** — `camera.isAvailable()` must not block on hardware init; anything doing real device I/O names it and carries `errors`.
- **No parallel APIs** — one canonical way per capability, no `v1`/`v2`.
- **No platform-default config** — defaults are identical on every platform; per-platform behavior is explicit, never a silent `getPreferredX()`.
- **Bounded by default** — capability event streams (touch events, location updates, mic frames) are bounded channels, never unbounded mailboxes (Rule 4).
- **Receiver-first** — the dot-call convention already enforces this.

---

## The honest cost

This layer is open-ended maintenance: each new OS release can add, deprecate, or gate device APIs, and each capability needs four implementations (iOS/Android/desktop/web) kept in sync behind one surface. The reference ecosystems (Capacitor plugins, Tauri plugins) treat this as a perpetual plugin surface, not a one-time build. Yinz should plan for the same: a core set of capabilities in stdlib, and the rest as packages once the package system exists (`../packages.md`).

---

## Cross-references

- [`docs/internal/scratchpad/SCRATCH-future-gui-architecture.md`](SCRATCH-future-gui-architecture.md) — the bridge these capabilities ride on
- [`docs/internal/scratchpad/SCRATCH-future-gui-build-targets.md`](SCRATCH-future-gui-build-targets.md) — where per-platform permission metadata is emitted during packaging
- [`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md) — the contract this API obeys
- [`.claude/rules/inference.md`](../../../.claude/rules/inference.md) / [`docs/reference/REF-golden-rules.md`](../../reference/REF-golden-rules.md) Rule 11 — teaching diagnostics for undeclared-capability errors
