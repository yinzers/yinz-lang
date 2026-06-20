# GUI & Cross-Platform Apps — Index

> **Status**: Direction locked, implementation deferred (far-future stdlib, post-v0.5).
> **Decision (2026-06-13)**: Yinz ships GUI as a **webview-hosted native shell** (the Tauri/Capacitor model) — one HTML/CSS/JS frontend, compiled into native binaries for web, desktop, iOS, and Android. Yinz owns the native shell, the IPC bridge, the device-capability layer, and the per-platform compile — **not** the frontend framework.
> **Why this and not pixel-perfect-native**: it covers MOST app use cases and is dramatically more feasible to implement (see `architecture.md` → "Why webview"). A pixel-perfect native renderer (the Flutter model) is documented as a someday-maybe, not the headline.

This folder groups the design for how Yinz builds graphical and cross-platform applications. It's a multi-file topic, so it gets a subfolder under `design/future/` rather than one giant file.

---

## The one-line model

You write a normal HTML/CSS/JS frontend (vanilla, React, Vue, Svelte — your choice). Yinz compiles a **native shell** around it per target platform, embeds that platform's **system webview** to render your frontend, and gives your frontend a typed **bridge** to call Yinz code for anything native (filesystem, camera, notifications…). The output is a real native app — `.exe` / `.app` / `.apk` / `.ipa` / web bundle — not a bundled browser (no Electron-style bloat).

---

## Files in this folder

| File | Covers |
|---|---|
| [`architecture.md`](architecture.md) | The webview-shell model in depth; the decision rationale ("why webview"); the documented alternatives (pixel-perfect renderer, "use Flutter") and exactly why they're not the headline; the locked constraints that stop us relitigating this |
| [`capabilities.md`](capabilities.md) | The device-capability layer — unified Yinz API for camera / mic / speaker / touch / GPS / notifications / biometrics, with per-platform implementations and a compile-checked permission model |
| [`build-targets.md`](build-targets.md) | Per-platform compilation and packaging (desktop / iOS / Android / web), including the WebAssembly target that powers the web build |

---

## Status header meaning

Per `design/future/index.md` conventions, each file states one of:
- **Direction locked** — the architectural choice is decided (webview model); the detailed API is not.
- **Open** — a specific sub-decision still needs Patrick's sign-off at implementation time.

These docs capture *what Yinz will build and why*. They are NOT a milestone plan — no phases, no schedule. When GUI is actually planned (post-v0.5, after the package system and stdlib basics exist), a real execution plan references these.

---

## Cross-references

- `design/future/packages.md` — the package system that will let third parties ship alternative renderers (e.g. a pixel-perfect native UI package) once it exists
- `design/gpu.md` — if Yinz ever builds its own pixel renderer, it builds on the GPU work, not on this webview model
- `design/future/wasm-target.md` is folded into `build-targets.md` here (the web build IS the WASM target)
- `.claude/rules/stdlib-design.md` — the capability API must follow the stdlib rules (pure-named methods pure, no parallel APIs, bounded-by-default, no platform-default config)
