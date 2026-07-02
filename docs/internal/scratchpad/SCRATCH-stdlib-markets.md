---
name: "SCRATCH-stdlib-markets"
description: "Technical analysis indicators. Part of the standard library — tree shaking ensures unused indicators cost nothing in the compiled binary."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Standard Library — Markets & Indicators

Technical analysis indicators. Part of the standard library — tree shaking ensures unused indicators cost nothing in the compiled binary.

Indicators are methods on `fixed<number>` / `array<number>` for close-price indicators, and on `fixed<Bar>` / `array<Bar>` for indicators that need full OHLCV data.

---

## The Bar Type — OHLCV Data

Indicators that need more than just closes (ATR, VWAP, Bollinger Bands) operate on `Bar`:

```
shape Bar {
  open: number
  high: number
  low: number
  close: number
  volume: number
  timestamp: Date
}
```

Close-only indicators can work directly on a `fixed<number>`, or you can extract closes from bars:

```
let bars: fixed<Bar> = loadBars()
let closes = bars.map(b => b.close)

let sma20 = closes.sma(period: 20)
let atr14 = bars.atr(period: 14)       // needs high/low/close — works on fixed<Bar>
```

---

## `skipWarmup` Parameter

Every indicator has a warmup period — the number of bars needed before the first valid value can be computed (e.g., a 20-period SMA needs 20 bars before it can produce a result).

All indicators accept an optional `skipWarmup` boolean:

```
// Default — full-length output, none for warmup bars
let sma = closes.sma(period: 20)
// -> fixed<maybe number>, same length as closes
// first 19 values are none, rest are valid numbers

// skipWarmup: true — only valid values, shorter output
let sma = closes.sma(period: 20, skipWarmup: true)
// -> fixed<number>, length is closes.count() - 19
// every value is guaranteed valid
```

Use the default when you need bar-for-bar alignment with price data (charting, indexing by bar number). Use `skipWarmup: true` when you want clean data for calculations | ML.

---

## SMA — Simple Moving Average

Average of the last N closes.

```
let sma20 = closes.sma(period: 20)
let sma200 = closes.sma(period: 200)
let sma20 = closes.sma(period: 20, skipWarmup: true)
```

Returns `fixed<maybe number>` by default, `fixed<number>` with `skipWarmup: true`.

---

## EMA — Exponential Moving Average

Weighted average giving more weight to recent prices. Multiplier: `2 / (period + 1)`.

```
let ema12 = closes.ema(period: 12)
let ema26 = closes.ema(period: 26)
let ema200 = closes.ema(period: 200)
let ema20 = closes.ema(period: 20, skipWarmup: true)
```

Returns `fixed<maybe number>` by default, `fixed<number>` with `skipWarmup: true`.

---

## MACD — Moving Average Convergence Divergence

Three outputs: MACD line, signal line, histogram.

```
shape MacdResult {
  macd: fixed<maybe number>
  signal: fixed<maybe number>
  histogram: fixed<maybe number>
}

shape MacdResultTrimmed {
  macd: fixed<number>
  signal: fixed<number>
  histogram: fixed<number>
}

let result = closes.macd(fast: 12, slow: 26, signal: 9)
let result = closes.macd(fast: 12, slow: 26, signal: 9, skipWarmup: true)

let lastHistogram = result.histogram.last()
if (lastHistogram.exists()) {
  let bullish = lastHistogram.value > 0
}
```

Standard defaults are `fast: 12, slow: 26, signal: 9` — all configurable.

---

## RSI — Relative Strength Index

Momentum oscillator. Returns 0-100. Above 70 is traditionally overbought, below 30 oversold.

```
let rsi14 = closes.rsi(period: 14)
let rsi14 = closes.rsi(period: 14, skipWarmup: true)

let current = rsi14.last().or(50)
if (current > 70) {
  print("Overbought")
}
```

Returns `fixed<maybe number>` by default, `fixed<number>` with `skipWarmup: true`.

---

## Bollinger Bands

Upper band, middle (SMA), and lower band. Bands are N standard deviations from the SMA.

```
shape BollingerResult {
  upper: fixed<maybe number>
  middle: fixed<maybe number>
  lower: fixed<maybe number>
}

shape BollingerResultTrimmed {
  upper: fixed<number>
  middle: fixed<number>
  lower: fixed<number>
}

let bb = closes.bollinger(period: 20, stdDev: 2.0)
let bb = closes.bollinger(period: 20, stdDev: 2.0, skipWarmup: true)

let lastClose = closes.last().or(0)
let upperBand = bb.upper.last().or(0)
if (lastClose > upperBand) {
  print("Price above upper band")
}
```

---

## ATR — Average True Range

Volatility measure. Needs high, low, close — operates on `fixed<Bar>`.

```
let atr14 = bars.atr(period: 14)
let atr14 = bars.atr(period: 14, skipWarmup: true)
```

Returns `fixed<maybe number>` by default, `fixed<number>` with `skipWarmup: true`.

---

## VWAP — Volume Weighted Average Price

Price weighted by volume. Needs close and volume — operates on `fixed<Bar>`.
Commonly used as an intraday benchmark. No warmup period — valid from bar 1.

```
let vwap = bars.vwap()

let lastVwap = vwap.last().or(0)
let lastClose = closes.last().or(0)
if (lastClose > lastVwap) {
  print("Price above VWAP")
}
```

Returns `fixed<number>` (no `maybe` — no warmup period).

---

## Relative Volume (RVOL)

Current bar's volume compared to average volume over a lookback period.
`1.0` = average, `2.0` = twice average volume.

```
let rvol = volumes.relVol(lookback: 20)
let rvol = volumes.relVol(lookback: 20, skipWarmup: true)

let current = rvol.last().or(0)
if (current > 2.0) {
  print("High relative volume")
}
```

Returns `fixed<maybe number>` by default, `fixed<number>` with `skipWarmup: true`.

---

## ROC — Rate of Change

Percentage change over N bars: `(current - N bars ago) / N bars ago * 100`.

```
let roc10 = closes.roc(period: 10)
let roc10 = closes.roc(period: 10, skipWarmup: true)
```

Returns `fixed<maybe number>` by default, `fixed<number>` with `skipWarmup: true`.

---

## Rolling High / Rolling Low

Highest high and lowest low over the last N bars. Building block for breakout systems and Donchian channels.

```
let high20 = closes.rollingHigh(period: 20)
let low20 = closes.rollingLow(period: 20)

// On Bar data — uses actual high/low fields
let high20 = bars.rollingHigh(period: 20)    // uses bar.high
let low20 = bars.rollingLow(period: 20)      // uses bar.low

// Both support skipWarmup
let high20 = closes.rollingHigh(period: 20, skipWarmup: true)
```

Returns `fixed<maybe number>` by default, `fixed<number>` with `skipWarmup: true`.

---

## Rolling Standard Deviation

Rolling standard deviation over N bars. Useful standalone and as the volatility component of Bollinger Bands.

```
let stdDev20 = closes.stdDev(period: 20)
let stdDev20 = closes.stdDev(period: 20, skipWarmup: true)
```

Returns `fixed<maybe number>` by default, `fixed<number>` with `skipWarmup: true`.

---

## Expansion Candidates

- Stochastic %K and %D
- Williams %R
- CCI (Commodity Channel Index)
- ADX (Average Directional Index — trend strength)
- OBV (On Balance Volume)
- MFI (Money Flow Index — RSI with volume weighting)
- PSAR (Parabolic SAR)
- Donchian Channels (already composable from rollingHigh + rollingLow)
- Pivot Points (daily/weekly/monthly support and resistance)
- HMA (Hull Moving Average — smoother, more responsive)
- DEMA / TEMA (Double/Triple EMA)

---

## Performance Note

All indicators are O(n) with tiny constant factors — additions, multiplications, and comparisons in tight loops. Compiled to native code with SIMD vectorization where possible. Computing any indicator over 1 million bars takes milliseconds. Faster than Python/pandas for equivalent workloads.
