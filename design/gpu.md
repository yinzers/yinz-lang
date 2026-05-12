# GPU Dispatch — Vision Document

**Status: MVP2+. Not in initial release. Documented here to preserve the design direction.**

---

## The Problem

GPU programming today requires:
- Separate languages (CUDA, OpenCL, GLSL, Metal)
- Manual memory management between CPU and GPU address spaces
- Explicit data transfer (CPU → GPU → CPU)
- Painful synchronization between CPU and GPU work
- Completely different programming models

Mixing GPU and CPU code in one application is a multi-language, multi-toolchain nightmare.

---

## The Yinz Vision — Same Syntax, Compiler Manages Dispatch

```
// Data just exists — compiler decides where it lives
let prices: array[float] = loadMarketData()
let signals: array[float] = loadSignals()

// CPU operation — small data, stays on CPU
let avg = math.average(prices.take(100))

// GPU operation — massive data, compiler dispatches to GPU
let correlations = gpu matrix.correlate(prices, signals)
// compiler handles: allocate GPU memory → copy data → execute → copy back

// Mix freely — compiler manages the data transfer
let filtered = correlations.where(c => c > 0.8)    // back on CPU
let ranked = filtered.sort(c => c, desc)             // CPU

// Developer writes linear code. Compiler shuffles data between CPU and GPU.
```

`gpu` is a call-site keyword prefix — same philosophy as `wait` and `background`. The function doesn't know it's being called on the GPU. The caller decides.

---

## Why Yinz Can Do This

The ownership system already proves what the compiler needs to know to manage GPU dispatch safely:

- **Data size**: The compiler knows the types and can estimate data size → decides CPU vs GPU
- **Ownership**: The compiler tracks who has the data → knows when it's safe to copy to GPU
- **Dependency graph**: Already used for CPU auto-parallelization → extends to scheduling GPU and CPU work
- **Data transfer**: Lazy — only copy when needed, cache on GPU if reused in subsequent `gpu` calls

The same proof that prevents data races in CPU concurrency can prevent GPU/CPU data races.

---

## Target Use Cases

**Trading systems:**
```
function analyzeMarket(share data: MarketData) -> array[Signal] errors {
  let correlations = gpu matrix.correlate(data.prices, data.volumes)
  let features = gpu tensor.transform(correlations, data.indicators)
  let signals = features.where(f => f.strength > threshold)
  let backtested = gpu simulate(signals, data.history)
  return backtested.where(b => b.profitable)
}
```

**Game rendering:**
```
function renderFrame(share scene: Scene) -> Frame {
  let transforms = gpu matrix.multiply(scene.objects, viewMatrix)
  let lit = gpu shader.phong(transforms, scene.lights)
  let antialiased = gpu image.ssaa(lit, samples: 4)
  return antialiased
}
```

---

## What Needs to Be Built for MVP2+

- LLVM GPU backend integration (NVVM for NVIDIA, ROCm for AMD)
- Shader/compute kernel generation from Yinz function bodies
- Automatic memory management between CPU and GPU
- GPU-aware dependency graph scheduling
- Fallback to CPU when no GPU is available
- `gpu` keyword parser and AST node
- Type system extensions for GPU-compatible types (likely a subset of types)

---

## Syntax Decision

`gpu` as a call-site prefix keyword — consistent with `wait` (force sequential) and `background` (separate lifetime). The function itself doesn't change; the caller annotates how to run it.

Alternative considered: `@gpu` annotation on the function definition. Rejected — same reason `async` on function definitions was rejected. The caller decides, not the function.
