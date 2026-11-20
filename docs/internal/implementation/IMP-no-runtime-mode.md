---
name: "IMP-no-runtime-mode"
description: "Design for the --kernel/--bare compiler flag enabling bare-metal targets (kernels, bootloaders, firmware) by disabling runtime-dependent features and requiring user-supplied allocator/scheduler/panic-handler primitives."
tags:
  - "yinz-compiler"
created_at: "2026-05-14"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# No-Runtime Mode — `--kernel` Flag for Bare-Metal Targets

**Status**: Design locked. **Not built**: the compiler has no `--kernel` flag, and the `runtime.set*` functions below do not exist yet.

User spec target: [`docs/reference/REF-tooling.md`](../../reference/REF-tooling.md) (gets a `--kernel` flag section when the flag ships).

---

## The Decision

Yinz compiles to environments without a standard runtime: kernels, bootloaders, firmware, microcontrollers, safety-critical embedded systems. The mechanism is a `--kernel` (or `--bare`) compiler flag that:

1. **Disables runtime-dependent features** at compile time (heap allocation, default scheduler, default panic handler, OS-dependent stdlib).
2. **Requires the user to plug in their own primitives** for anything the runtime normally provides (allocator, scheduler if used, panic handler, output device).
3. **Emits teaching compile errors** when a blocked feature is used, pointing to the kernel-mode alternative.

The default is the runtime (most users want it); no-runtime is the opt-in.

**Why**: those environments have no OS-provided `malloc`, threads, file I/O or `printf`. Without compile-time enforcement, `Array<T>` would silently fail to link there.

---

## What `--kernel` mode disables

Any language feature or stdlib API that requires the runtime is a compile error in `--kernel` mode unless a custom primitive is plugged in:

| Feature | Default behavior | `--kernel` behavior |
|---------|------------------|---------------------|
| `Array<T>` (growable) | Heap allocation via libc malloc | Compile error unless `.in(myAllocator)` provided |
| `Map<K, V>` | Heap allocation | Compile error unless allocator provided |
| `String` (growable) | Heap allocation | Compile error unless allocator provided |
| `Fixed<T>` | Stack allocation | Always works (no heap needed) |
| `print()` / `println()` | libc stdio | Compile error unless `runtime.setStdout(myWriter)` |
| `background` (spawn task) | Runtime scheduler | Compile error unless `runtime.setScheduler(myScheduler)` |
| `wait` (I/O suspension) | Runtime suspension | Compile error unless scheduler provided |
| Default panic handler | Prints stack + exits | Compile error unless `runtime.setPanicHandler(myHandler)` |
| Auto-`Arc` (cross-thread shared state) | Standard atomic refcount | Compile error unless allocator + thread primitives |
| `arena {}` blocks | Compiler-managed scratchpad | Always works (uses user-provided allocator if no default) |

`Fixed<T>`, primitive math, control flow, function calls, `shape` declarations and value semantics work in `--kernel` mode because they do not depend on the runtime.

---

## Plug-in runtime architecture

The user provides custom primitives through runtime-injection functions (stdlib functions in `--kernel` mode):

```ynz
// Custom kernel allocator
runtime.setAllocator(myKernelAllocator)

// Custom output (e.g., serial port, UART)
runtime.setStdout(mySerialWriter)

// Custom panic handler
runtime.setPanicHandler((info: PanicInfo) -> Nothing {
  // log panic to firmware NVRAM, halt CPU, etc.
})

// Once configured, normal Yinz code works
let buffer: Array<Int> = []              // uses myKernelAllocator
print(`kernel booting`)                  // uses mySerialWriter
```

The injection functions are heap-free: they store function pointers in known locations that compiler-emitted code references.

---

## Compile-error format for blocked features

Every kernel-mode error follows Golden Rule 3's three parts (what, what instead, why):

```
let buffer: Array<Int> = []
// COMPILE ERROR: Array<T> requires a heap allocator — not available in --kernel mode.
//
//   Use Fixed<T, N> for stack-allocated storage with a compile-time-known size,
//   or provide a custom allocator: Array<Int>.in(myKernelAllocator).
//
//   Kernels don't have an OS-provided allocator. Fixed<T, N> uses stack memory
//   (size locked at compile time). For dynamic sizing, plug in your kernel's
//   allocator via Array<Int>.in(...) — the language ships allocator-injection
//   for exactly this scenario. See docs/internal/implementation/IMP-no-runtime-mode.md.
```

---

## Runtime statement for every new feature

Every new feature states what it needs at runtime and whether it works in `--kernel` mode, so the `--kernel` work never analyzes a feature after the fact.

- `shape` declarations: runtime-independent; always work in `--kernel`.
- `Array<T>`: requires an allocator; `--kernel` requires an explicit `.in(allocator)` on every construction.
- `errors` keyword: runtime-independent (type-system flow only); always works.
- `background` task spawning: requires a scheduler; `--kernel` requires custom scheduler injection.

---

## Target audiences

- **Embedded firmware**: IoT devices, automotive ECUs, sensor controllers.
- **Kernel modules**: drivers and utilities running in kernel space.
- **Bootloaders**: early-boot code that runs before any OS.
- **Safety-critical systems**: avionics, medical devices, industrial control.
- **Aerospace**: deterministic memory layout and no allocation in the critical path.

Formal verification (for example "this function never allocates after initialization") is a v3+ research project outside `--kernel`. `--kernel` is its prerequisite, because verification needs controlled allocation and runtime dependencies. Tracked in the "Parking Lot" of [`docs/internal/scratchpad/SCRATCH-future-designs-index.md`](../scratchpad/SCRATCH-future-designs-index.md).

---

## Open questions

- Exact flag name: `--kernel`, `--bare` or `--no-runtime`.
- Runtime-injection API shape: function pointers, contracts, or global static initialization.
- Cross-compilation targets: which target triples `--kernel` supports (ARM Cortex-M, RISC-V, custom MMIO platforms).
- Binary size: the no-runtime binary size baseline.
- Test infrastructure: how CI tests `--kernel` (QEMU, real hardware).

---

## Cross-references

- [`docs/internal/implementation/IMP-no-function-coloring.md`](IMP-no-function-coloring.md) (background/wait require the scheduler, which is disabled in kernel mode unless custom-provided)
- [`docs/internal/scratchpad/SCRATCH-future-arena.md`](../scratchpad/SCRATCH-future-arena.md) (arenas work in kernel mode with user-provided base allocator)
- [`docs/internal/scratchpad/SCRATCH-future-packages.md`](../scratchpad/SCRATCH-future-packages.md) (binary metadata includes per-item kernel-mode compatibility flags)
