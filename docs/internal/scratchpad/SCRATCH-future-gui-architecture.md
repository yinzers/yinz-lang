---
name: "SCRATCH-future-gui-architecture"
description: "Design notes for a future Yinz GUI architecture: a native Yinz shell hosting the OS's own system webview (Tauri/Capacitor-style), with a typed IPC bridge to a user-chosen web frontend."
tags:
  - "yinz-compiler"
created_at: "2026-06-13"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# GUI Architecture — Webview-Hosted Native Shell

> **Status**: Direction locked (2026-06-13), detailed API deferred.
> **Audience**: contributors. This is the WHY behind the GUI model, not a usage guide.

---

## The model

Yinz builds graphical apps the way Tauri (Rust) and Capacitor (web→mobile) do:

```
┌─────────────────────────────────────────────┐
│  Native shell  (Yinz → LLVM machine code)     │  ← your app logic, no GC, Rust-class
│  ┌─────────────────────────────────────────┐  │
│  │  System webview                           │  │  ← OS-provided, NOT bundled
│  │  (WebView2 / WKWebView / WebKitGTK /      │  │
│  │   Android System WebView)                 │  │
│  │  renders your HTML / CSS / JS frontend     │  │  ← vanilla / React / Vue / Svelte
│  └─────────────────────────────────────────┘  │
│         ▲  typed IPC bridge  ▼                  │  ← frontend calls Yinz; Yinz calls back
└─────────────────────────────────────────────┘
```

- **Frontend**: a normal web frontend. The user picks the framework (or none). Yinz does not bless or bundle one — see "Why no baked-in framework" below.
- **Shell + logic**: written in Yinz, compiled to native machine code per platform. All the heavy work (filesystem, computation, data, networking, device access) lives here.
- **Webview**: the *operating system's own* webview component renders the frontend. Yinz does NOT ship a browser. This is the single biggest difference from Electron.
- **Bridge**: a typed command channel. The frontend calls named Yinz commands; Yinz returns typed results (and `errors` values when a call can fail). The device-capability layer (`capabilities.md`) rides this bridge.

One frontend codebase; Yinz recompiles the shell per target (`build-targets.md`).

---

## Why webview (the decision)

Two reasons, both decisive:

1. **It covers MOST real app use cases.** The overwhelming majority of apps people build — business tools, dashboards, CRUD, dev tools, admin panels, content apps — are bottlenecked by *logic, data, and I/O*, not by UI render speed. Yinz makes that bottleneck native-fast (see Performance). The webview UI ceiling only bites a minority of apps (games, heavy real-time graphics, pro creative tools).

2. **It is dramatically more feasible to implement.** The alternative (pixel-perfect native rendering) requires Yinz to build and maintain a CSS layout engine + GPU renderer — one of the most expensive things in all of software (browser engines are hundreds of person-years). The webview model reuses the OS's already-shipped, already-optimized engine. Yinz's effort goes into the shell, the bridge, and the capability layer — achievable as stdlib work — instead of into rebuilding a browser.

This is a real engineering tradeoff with a named cost, not a "good enough for now": we trade *pixel-identical rendering across platforms* (which we do not get) for *the entire web ecosystem + a feasible implementation + small fast binaries* (which we do get). The cost is acknowledged and bounded; the escape hatch for the minority case is a third-party package (below).

---

## Performance — is a Yinz desktop app as fast as Rust? Why isn't it Electron-slow?

Split the question, because "the app" is two layers with two different speed stories.

**The logic layer: yes, Rust-class.** Everything you write in Yinz — file I/O, computation, data processing, networking, the bridge handlers — compiles to LLVM machine code with no garbage collector and ownership-based memory management. That is the same performance class as Rust (and as Tauri's Rust backend). There is no JavaScript runtime doing your app's real work.

**The UI layer: webview-class, which is fast — and nothing like Electron.** What's drawn on screen is rendered by the OS webview (a heavily-optimized native C++ engine — WebKit/Chromium). That's *browser-rendering* speed, not *native-widget/pixel* speed. For normal UIs (forms, lists, dashboards, animations) it's smooth. The ceiling only shows up under extreme UI load: 120fps game-grade animation, enormous DOMs, heavy real-time canvas.

**Why this is decisively faster than Electron** — Electron is slow for three specific reasons, and the webview model fixes all three:

| Electron's cost | Webview model |
|---|---|
| Bundles a **whole Chromium + Node** in every app (~85–150MB, big RAM per app) | Uses the OS's **existing shared webview** (~3–10MB binary, a fraction of the RAM) |
| App **logic runs in JavaScript/Node** (GC'd, JIT'd) | App logic runs in **native Yinz** (no GC, AOT, Rust-class) |
| Per-app browser process bloat, slow cold start | Tiny native shell, fast start; webview often already warm in the OS |

Electron's reputation isn't the *rendering engine* (that engine is fast) — it's the bundling + JS-logic + per-app-browser overhead. Yinz removes all three. So a Yinz desktop app starts fast, uses modest RAM, and runs its logic at native speed.

**The honest boundary**: a pure-Rust GUI that renders with a *native* toolkit (egui/Slint) or a pixel renderer will out-render a webview UI on the screen-drawing axis specifically. If an app's bottleneck genuinely is UI rendering (a game, a 3D/video tool), the webview is the wrong tool and the answer is a native-rendering package (below) — not this model. For everything else, "as fast as Rust" is true where it matters (the logic) and "fast enough and far faster than Electron" is true for the UI.

---

## Why no baked-in frontend framework

Yinz provides the shell + bridge; the user brings the frontend framework (or none). Yinz does **not** bundle or bless React/Vue/Svelte. Rationale:

- **Frameworks churn brutally** — jQuery → Angular → React → Vue → Svelte → Solid in ~15 years. Baking one into a systems-language stdlib is `no-duct-tape.md` build-twice debt: you rebuild it the moment the framework falls out of fashion.
- **The webview hosts any of them for free** — they're just HTML/CSS/JS to the webview. Supporting "all of them" costs nothing; blessing one costs a permanent maintenance commitment.

This is why the webview model *can* keep the whole web ecosystem — see the next section for why the alternative model can't.

---

## Documented alternatives (and exactly why they are NOT the headline)

These are written down so we never relitigate them or wander into a contradiction.

### Alternative B — Pixel-perfect native renderer ("Yinz's own Flutter")

HTML/CSS as a compile-time *authoring syntax* (not a live runtime) → lowered to a native widget/render tree → AOT-compiled → Yinz draws every pixel via GPU. No JS engine at runtime, so no iOS JIT restriction; pixel-identical on every platform.

**Why it's deferred, not chosen:**
- **It cannot host the web ecosystem.** React/Vue are *runtime* frameworks — their build step emits plain JavaScript that *requires a live JS engine to run continuously* (every click re-runs JS that mutates the DOM). A pixel renderer with no DOM and no JS runtime structurally cannot run them. You'd get HTML/CSS-as-syntax + Yinz only — no React, no Vue, no npm. That directly contradicts the headline promise ("bring your existing web frontend").
- **It requires building a CSS layout engine** — flexbox, grid, the cascade, all the edge cases. This is the hundreds-of-person-years cost browsers pay. Flutter sidestepped it by inventing its *own* (non-CSS) layout model; committing to real CSS means committing to a browser-grade engine.
- **On the web target it collapses to canvas-rendering** (Flutter Web's approach: draw to `<canvas>` via a WASM renderer) — with its real downsides: multi-MB initial download, broken SEO, broken text selection, accessibility pain.

**The locked reasoning — why you can't "just snapshot a compiled React app into native":** A React/Vue app is not a compiled artifact; it is a JavaScript program that runs forever, recomputing the UI on every event. (1) Its build output is plain JS that still needs a JS engine. (2) The browser never "finishes compiling" — JIT'd machine code is ephemeral, engine-internal, non-portable, and iOS-banned. (3) You *can* snapshot the rendered HTML (that's SSR/pre-rendering), but the snapshot is a dead picture — interactivity requires re-running the JS ("hydration"). Therefore keeping React/Vue *requires* a live JS engine; the webview supplies it for free, and a no-runtime pixel model cannot. Adding a JS engine back into the pixel model just reinvents React Native (JS runtime + bridge) and demotes Yinz from "the app's language" to a footnote.

If pixel-perfect rendering ever becomes a hard requirement, this is the path — and it builds on [`docs/internal/implementation/IMP-gpu.md`](../implementation/IMP-gpu.md), not on this webview model. It is plausibly its own product-sized initiative, not a milestone of this one.

### Alternative C — Transpile HTML → Flutter, let Flutter compile — REJECTED

**Rejected outright:**
- It shackles Yinz to Google's Dart/Flutter SDK as a permanent build dependency — for a language whose identity is owning its LLVM-native pipeline.
- The HTML/CSS → Flutter-widget mapping is lossy; flexbox/grid/cascade don't map onto Flutter's constraint model, so Flutter's un-intuitiveness leaks through the cracks anyway.
- Yinz (no-GC, ownership) compiling down through Dart (GC'd) is an impedance mismatch and two compile layers.

The good version of this instinct is Alternative B (build our own, don't depend on Flutter) — not "use Flutter."

---

## Escape hatch — third-party native-rendering packages

For the minority of apps that genuinely need pixel-perfect or native-widget rendering (a graphically demanding iOS app, a game, a creative tool), the answer is **a third-party package** providing an alternative renderer — once the package system exists.

This is deliberately deferred: the package system is not yet built ([`docs/internal/scratchpad/SCRATCH-future-packages.md`](SCRATCH-future-packages.md)). When it ships, nothing in this webview model prevents a community or first-party package from offering a native/pixel renderer as an opt-in alternative. We document the door; we don't build it now. The user-facing "you can bring your own renderer" note belongs in `spec/` only once GUI actually ships (per [`.claude/rules/spec-writing.md`](../../../.claude/rules/spec-writing.md), spec covers shipped features only) — until then it lives here and in the packages doc.

---

## Cross-references

- [`docs/internal/scratchpad/SCRATCH-future-gui-capabilities.md`](SCRATCH-future-gui-capabilities.md) — the device-capability + permission layer on top of the bridge
- [`docs/internal/scratchpad/SCRATCH-future-gui-build-targets.md`](SCRATCH-future-gui-build-targets.md) — per-platform compile + packaging + the WASM web target
- [`docs/internal/scratchpad/SCRATCH-future-packages.md`](SCRATCH-future-packages.md) — the package system that enables alternative-renderer packages
- [`docs/internal/implementation/IMP-gpu.md`](../implementation/IMP-gpu.md) — the GPU foundation a future pixel renderer would build on
- `.claude/rules/no-duct-tape.md` — the named-tradeoff discipline this decision follows
