---
name: "IMP-no-runtime-mode"
description: "Design for the --kernel/--bare compiler flag enabling bare-metal targets (kernels, bootloaders, firmware) by disabling runtime-dependent features and requiring user-supplied allocator/scheduler/panic-handler primitives."
tags:
  - "yinz-compiler"
created_at: "2026-05-14"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# No-Runtime Mode — `--kernel` Flag for Bare-Metal Targets

**Status**: Locked, v0.3 implementation. Design ready when implementation is scheduled.

User spec target: [`docs/reference/REF-tooling.md`](../../reference/REF-tooling.md) (gets a `--kernel` flag section when implemented).

---

## The Decision

Yinz supports compiling to environments WITHOUT a standard runtime — kernels, bootloaders, firmware, microcontrollers, NASA-grade embedded systems. The mechanism: a `--kernel` (or `--bare`) compiler flag that:

1. **Disables runtime-dependent features** at compile time (heap allocation, default scheduler, default panic handler, OS-dependent stdlib)
2. **Requires the user to plug in their own primitives** for anything that would normally come from the runtime (custom allocator, custom scheduler if used, custom panic handler, custom output device)
3. **Emits teaching compile errors** when blocked features are used, pointing to the kernel-mode alternative

This is Yinz's equivalent of Rust's `no_std`. The difference: Yinz's default IS the runtime (most users want it); `no_std` is opt-in via flag. Rust's default is the OTHER way around.

---

## Why Yinz needs this

Patrick's stated goal: Yinz should eventually be usable for chipset code, OS kernels, and NASA-grade embedded systems. None of those environments have an OS providing `malloc`, threads, file I/O, or even `printf`. They have:
- A specific memory layout (sometimes mapped registers, no heap)
- A custom allocator (kmalloc in kernels, fixed buffers in firmware)
- A custom output device (UART, framebuffer, system log)
- A custom panic handler (kernel oops, firmware halt-and-flash-LED)

Without `--kernel` mode, Yinz's `array<T>` would secretly require malloc and silently fail to link in these environments. Compile-time enforcement is the only safe path.

---

## What `--kernel` mode disables

Any language feature or stdlib API that requires the runtime becomes a COMPILE ERROR in `--kernel` mode unless a custom primitive is plugged in:

| Feature | Default behavior | `--kernel` behavior |
|---------|------------------|---------------------|
| `array<T>` (growable) | Heap allocation via libc malloc | COMPILE ERROR unless `.in(myAllocator)` provided |
| `map<K, V>` | Heap allocation | COMPILE ERROR unless allocator provided |
| `string` (growable) | Heap allocation | COMPILE ERROR unless allocator provided |
| `fixed<T>` | Stack allocation | Always works (no heap needed) |
| `print()` / `println()` | libc stdio | COMPILE ERROR unless `runtime.setStdout(myWriter)` |
| `background` (spawn task) | Runtime scheduler | COMPILE ERROR unless `runtime.setScheduler(myScheduler)` |
| `wait` (I/O suspension) | Runtime suspension | COMPILE ERROR unless scheduler provided |
| Default panic handler | Prints stack + exits | COMPILE ERROR unless `runtime.setPanicHandler(myHandler)` |
| Auto-`Arc` (cross-thread shared state) | Standard atomic refcount | COMPILE ERROR unless allocator + thread primitives |
| `arena {}` blocks | Compiler-managed scratchpad | Always works (uses user-provided allocator if no default) |

`fixed<T>`, primitive math, control flow, function calls, `shape` declarations, and value semantics all work in `--kernel` mode — they don't depend on the runtime.

---

## Plug-in runtime architecture

The user provides custom primitives via runtime-injection APIs (these are stdlib functions in `--kernel` mode):

```ynz
// Custom kernel allocator
runtime.setAllocator(MyKernelAllocator)

// Custom output (e.g., serial port, UART)
runtime.setStdout(MySerialWriter)

// Custom panic handler
runtime.setPanicHandler((info: PanicInfo) -> nothing {
  // log panic to firmware NVRAM, halt CPU, etc.
})

// Once configured, normal Yinz code works
let buffer: array<int> = []              // uses MyKernelAllocator
println("kernel booting")              // uses MySerialWriter
```

The runtime-injection APIs are themselves stdlib items — they're heap-free (they just store function pointers in known locations the compiler-emitted code references).

---

## Compile-error format for blocked features

Following Golden Rule 11's three-part format:

```
let buffer = array<int>()
// COMPILE ERROR: array<T> requires a heap allocator — not available in --kernel mode.
//
//   Use fixed<T, N> for stack-allocated storage with a compile-time-known size,
//   or provide a custom allocator: array<int>.in(myKernelAllocator).
//
//   Kernels don't have an OS-provided allocator. fixed<T, N> uses stack memory
//   (size locked at compile time). For dynamic sizing, plug in your kernel's
//   allocator via array<int>.in(...) — the language ships allocator-injection
//   for exactly this scenario. See docs/internal/implementation/IMP-no-runtime-mode.md.
```

Every kernel-mode error MUST follow this format. The error is also a teaching moment for kernel devs — they probably haven't seen Yinz's allocator-injection pattern before.

---

## Forward-compatibility for v0.1-v0.2 features

Every NEW feature from M3 onward MUST declare its runtime dependencies and kernel-mode behavior in its milestone plan's `### Runtime Dependencies` and `### Kernel-Mode Behavior` sub-sections (per [`.claude/rules/plan-invariants.md`](../../../.claude/rules/plan-invariants.md)).

Examples:

- `shape` declarations (M4): runtime-independent. Always works in `--kernel`.
- `array<T>` (M5): requires allocator. `--kernel` requires explicit `.in(allocator)` parameter on every construction.
- `errors` keyword (M7): runtime-independent (just type-system flow). Always works.
- `background` task spawning (M8 parsing-only, v0.2 implementation): requires scheduler. `--kernel` requires custom scheduler injection.

If a feature lands WITHOUT declaring its runtime dependencies, the v0.3 work to add `--kernel` mode hits a wall — features have to be retroactively analyzed. The plan-invariants rule prevents this.

Graveyard Entry 4 (Phase 5 of the design-lockdown plan) catches plans that touch `crates/` without declaring runtime dependencies. Mechanical enforcement.

---

## Target audiences

This isn't just for hobbyists wanting to write a toy OS. Real Yinz `--kernel` users:

- **Embedded firmware** — IoT devices, automotive ECUs, sensor controllers
- **Kernel modules** — Linux kernel drivers, Yinz-rewritten Unix utilities running in kernel space
- **Bootloaders** — early-boot code that runs before any OS
- **Safety-critical systems** — avionics, medical devices, industrial control
- **Aerospace** — NASA-grade software where deterministic memory layout and no-allocation-in-critical-path matter

Yinz's combination of memory safety + ownership + zero-cost abstractions + custom allocator support hits a niche Rust already serves but with a friendlier learning curve. Long-term: position Yinz as "Rust for embedded systems" with better ergonomics.

---

## Beyond `--kernel`: formal verification

Some of these target audiences (NASA, avionics) need MORE than `--kernel` — they need formal verification of properties like "this function never allocates after initialization" or "this function terminates within N cycles."

Formal verification is a v3+ research project, not in scope for `--kernel` mode itself. But `--kernel` mode is a prerequisite: without controlling memory allocation and runtime dependencies, formal verification is much harder. Adding `--kernel` now makes the formal-verification step tractable later.

See [`docs/internal/scratchpad/SCRATCH-future-designs-index.md`](../scratchpad/SCRATCH-future-designs-index.md) "Parking Lot" section for formal verification.

---

## v0.3 Implementation notes

The v0.3 milestone plan must address:

- Exact flag name: `--kernel` vs `--bare` vs `--no-runtime`. Probably `--kernel` since that's what users searching for "Yinz embedded" will type.
- Runtime-injection API design: function pointers? Trait-like contracts? Global static-init?
- Cross-compilation targets: what triples does `--kernel` mode support? ARM Cortex-M? RISC-V? Custom MMIO platforms?
- Binary size: kernel binaries need to be SMALL. What's the no-runtime binary size baseline?
- Test infrastructure: how do we test `--kernel` mode in CI? QEMU? Real hardware?

---

## Cross-references

- [`.claude/rules/plan-invariants.md`](../../../.claude/rules/plan-invariants.md) (M4+ plans declare runtime dependencies + kernel-mode behavior — mechanical enforcement for forward-compat)
- [`docs/internal/implementation/IMP-no-function-coloring.md`](IMP-no-function-coloring.md) (background/wait require the scheduler, which is disabled in kernel mode unless custom-provided)
- [`docs/internal/scratchpad/SCRATCH-future-arena.md`](../scratchpad/SCRATCH-future-arena.md) (arenas work in kernel mode with user-provided base allocator)
- [`docs/internal/scratchpad/SCRATCH-future-packages.md`](../scratchpad/SCRATCH-future-packages.md) (binary metadata includes per-item kernel-mode compatibility flags)
- [`.claude/planning/done/2026-05-12-v0-1-compiler/roadmap.md`](../../../.claude/planning/done/2026-05-12-v0-1-compiler/roadmap.md) "Forward-Compatibility Constraints" (Phase 5 locks the requirement)
