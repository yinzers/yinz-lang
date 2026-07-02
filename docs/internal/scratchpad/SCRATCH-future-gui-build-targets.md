---
name: "SCRATCH-future-gui-build-targets"
description: "One frontend codebase; Yinz recompiles the native shell per target and emits the platform's real package format. WASM is folded in here because the web 'build' *is* the WASM target — it's not a separate concern, it's..."
tags:
  - "yinz-compiler"
created_at: "2026-06-13"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Build Targets & Packaging (incl. the WebAssembly web target)

> **Status**: Direction locked, packaging details open.
> **Audience**: contributors.

One frontend codebase; Yinz recompiles the native shell per target and emits the platform's real package format. WASM is folded in here because the web "build" *is* the WASM target — it's not a separate concern, it's one more output of the same pipeline.

---

## Targets and how each is produced

| Target | Shell compiles to | Webview used | Package emitted |
|---|---|---|---|
| **Desktop — Windows** | `x86_64-pc-windows` native | WebView2 (Edge/Chromium, OS-provided) | `.exe` / installer |
| **Desktop — macOS** | `x86_64` / `aarch64-apple-darwin` native | WKWebView | `.app` / `.dmg` |
| **Desktop — Linux** | `x86_64` / `aarch64` native | WebKitGTK | AppImage / `.deb` / binary |
| **iOS** | `aarch64-apple-ios` native (AOT — required by Apple) | WKWebView (mandatory) | `.ipa` |
| **Android** | `aarch64-linux-android` native (+ JNI bridge) | Android System WebView | `.apk` / `.aab` |
| **Web** | **`wasm32` (WebAssembly)** | the user's actual browser | static web bundle |

The frontend assets (HTML/CSS/JS) are identical across targets; only the shell is recompiled. LLVM already provides every triple above, so each target is "another `--target`" plus the per-platform glue (webview embedding, packaging, signing).

---

## The WebAssembly target (the web build)

For the web target there is no native shell — the browser *is* the host. The Yinz logic that would be the "shell" elsewhere compiles to **WebAssembly** and runs in the browser alongside the frontend, exposed through the same bridge API the native targets use. So a command the frontend calls resolves to:
- a native Yinz function on desktop/mobile, or
- a WASM Yinz function on web —

…with the **same call site in the frontend.** Write the bridge once; it works on every target.

**Why WASM is folded in here, not its own top-level concern:** from the app author's perspective the web build is just another target of the same GUI pipeline. (WASM has uses beyond GUI — plugins, serverless — but those are out of scope for this folder; if they grow, they earn their own doc.)

**The hard part of the WASM target — ownership across the JS boundary.** Yinz's no-GC ownership model has to interoperate with the browser's GC'd JS objects (DOM nodes, fetch responses, etc.). Rust solves this with `wasm-bindgen` + borrowed `JsValue` handles; Yinz needs its own answer — most likely a "borrowed JS handle" type that the ownership system understands and that the `errors` model uses for boundary failures. This is the one genuinely novel design problem in the web target and is flagged Open.

---

## Capability metadata is emitted at packaging time

The capability declarations (`capabilities.md`) drive per-platform permission metadata, generated during packaging:
- iOS → Info.plist usage strings
- Android → manifest `<uses-permission>` + runtime-grant scaffolding
- Web → the browser permission prompts are runtime, but the build records which capabilities the app may request

An undeclared capability is a *compile* error (before packaging ever runs), so the emitted metadata is always complete and consistent with the code.

---

## What's locked vs open

- **Locked**: the set of targets; one-frontend-many-shells; webview-per-platform; WASM as the web shell; the bridge API being identical across targets.
- **Open** (decided at implementation): packaging/signing toolchain choices, the exact ownership-across-JS-boundary type, and whether mobile ships in the first GUI milestone or as a follow-on (mobile adds real Swift/Kotlin interop + app-store packaging weight).

---

## Cross-references

- [`docs/internal/scratchpad/SCRATCH-future-gui-architecture.md`](SCRATCH-future-gui-architecture.md) — the shell + bridge model these targets compile
- [`docs/internal/scratchpad/SCRATCH-future-gui-capabilities.md`](SCRATCH-future-gui-capabilities.md) — the capability declarations that become per-platform permission metadata
- [`docs/internal/implementation/IMP-no-runtime-mode.md`](../implementation/IMP-no-runtime-mode.md) — `--kernel` is a *different* "no OS" target; the WASM/web target here assumes a browser host, not bare metal
