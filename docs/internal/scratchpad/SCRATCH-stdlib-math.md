---
name: "SCRATCH-stdlib-math"
description: "Comprehensive math module. Scientist-grade coverage. Tree shaking means unused functions cost nothing — add everything."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Standard Library — Math

Comprehensive math module. Scientist-grade coverage. Tree shaking means unused functions cost nothing — add everything.

---

## Number Types

```
// Number — decimal, always correct (default)
let price = 0.1 + 0.2              // 0.3 — exact
let tax = 19.99 * 0.07             // 1.3993 — exact

// Float — fast binary (opt-in)
let velocity: Float = 9.81 * 2.5   // fast, may have tiny rounding

// Int — whole numbers only
let count: Int = 42
```

IDE warns when `Float` is used for financial values. IDE suggests `Int` for pure integer math.

---

## Basic Math

```
math.round(3.7)                     // 4
math.floor(3.7)                     // 3
math.ceil(3.2)                      // 4
math.clamp(value, min, max)         // keep value within range
math.abs(-5)                        // 5
math.sign(-5)                       // -1
math.pow(2, 8)                      // 256
math.sqrt(16)                       // 4
math.cbrt(27)                       // 3
math.min(3, 7, 2)                   // 2
math.max(3, 7, 2)                   // 7
math.sum([1, 2, 3, 4])              // 10
math.average([1, 2, 3, 4])          // 2.5
math.median([1, 2, 3, 4, 5])        // 3
math.mod(10, 3)                     // 1
math.gcd(12, 8)                     // 4
math.lcm(4, 6)                      // 12
math.factorial(5)                   // 120
math.fibonacci(10)                  // 55
```

---

## Trigonometry

```
math.sin(angle)
math.cos(angle)
math.tan(angle)
math.asin(value)
math.acos(value)
math.atan(value)
math.atan2(y, x)
math.sinh(value)
math.cosh(value)
math.tanh(value)
math.toRadians(degrees)             // 180 → π
math.toDegrees(radians)             // π → 180
```

---

## Logarithms & Exponentials

```
math.log(value)                     // natural log (ln)
math.log2(value)
math.log10(value)
math.logBase(value, base)
math.exp(value)                     // e^x
math.exp2(value)                    // 2^x
```

---

## Constants

```
math.pi                             // 3.14159265358979...
math.e                              // 2.71828182845904...
math.tau                            // 2π
math.phi                            // golden ratio 1.61803...
math.sqrt2
math.ln2
math.ln10
math.infinity
math.negativeInfinity
```

---

## Statistics

```
stats.mean(data)
stats.median(data)
stats.mode(data)
stats.variance(data)
stats.standardDeviation(data)
stats.percentile(data, 95)
stats.correlation(dataA, dataB)
stats.covariance(dataA, dataB)
stats.zScore(value, mean, stdDev)
stats.normalize(data)               // normalize to 0-1 range
stats.histogram(data, bins: 10)
```

---

## Linear Algebra

```
matrix.from([[1, 2], [3, 4]])
matrix.transpose()
matrix.inverse()
matrix.determinant()
matrix.multiply(other)
matrix.add(other)
matrix.scale(factor)
matrix.eigenvalues()
matrix.eigenvectors()
matrix.rank()
matrix.trace()
matrix.identity(size)

vector.from([1, 2, 3])
vector.dot(other)
vector.cross(other)
vector.magnitude()
vector.normalize()
vector.angle(other)
vector.project(onto)
```

---

## Interpolation & Curves

```
math.lerp(a, b, t)
math.smoothstep(a, b, t)
math.bezier(points, t)
math.spline(points, t)
math.map(value, inMin, inMax, outMin, outMax)
```

---

## Random

```
random.number()                     // 0 to 1
random.between(min, max)
random.int(min, max)
random.boolean()
random.pick(array)
random.shuffle(array)
random.seed(value)
random.gaussian(mean, stdDev)
random.uuid()
```

---

## Physics Constants

```
physics.speedOfLight                // 299792458 m/s
physics.gravity                     // 9.80665 m/s²
physics.planck                      // 6.62607015e-34 J⋅s
physics.boltzmann                   // 1.380649e-23 J/K
physics.avogadro                    // 6.02214076e23 mol⁻¹
physics.electronMass                // 9.1093837015e-31 kg
physics.protonMass                  // 1.67262192369e-27 kg
physics.elementaryCharge            // 1.602176634e-19 C
physics.vacuumPermittivity          // 8.8541878128e-12 F/m
physics.vacuumPermeability          // 1.25663706212e-6 N/A²
```

---

## Unit Conversion

```
let boiling = convert.celsius(100)
let boilingFahrenheit = boiling.toFahrenheit()

let distance = convert.kilometers(5)
let distanceMiles = distance.toMiles()

let weight = convert.kilograms(1)
let weightPounds = weight.toPounds()

let height = convert.meters(1)
let heightFeet = height.toFeet()

let volume = convert.liters(1)
let volumeGallons = volume.toGallons()

let energy = convert.joules(1)
let energyCalories = energy.toCalories()

let pressure = convert.pascals(101325)
let pressureAtmospheres = pressure.toAtmospheres()

let angle = convert.radians(math.pi)
let angleDegrees = angle.toDegrees()
```

---

## Expansion Candidates

- Complex number arithmetic (a + bi)
- Quaternion math (3D rotations)
- Numerical integration and differentiation
- FFT (Fast Fourier Transform)
- ODE/PDE solvers
- Big integer / arbitrary precision arithmetic
- Polynomial operations
- Number theory (primality, factoring)
- Signal processing basics
- Coordinate system conversions (cartesian, polar, spherical)
- Numerical optimization (gradient descent, Newton's method)
