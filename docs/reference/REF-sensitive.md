---
name: "REF-sensitive"
description: "A value marked sensitive prints as [REDACTED] everywhere. You have to consciously choose to reveal it."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Sensitive Values

A value marked `sensitive` is protected from accidental logging. It prints as `[REDACTED]`, and you have to consciously choose to reveal it.

---

## Marking a value as sensitive

Wrap the value in `sensitive(...)` and give the variable a `sensitive String` type:

```
let apiKey: sensitive String = sensitive(`sk-test-12345`)
let appName = `My App`                          // regular String

print(apiKey)                                   // [REDACTED]
print(appName)                                  // My App
```

Only a `String` can be marked `sensitive`.

---

## Auto-redaction in all output

A sensitive value prints as `[REDACTED]` everywhere: `print()`, text with `${...}` in it, and when you print a whole shape that holds one:

```
shape Config {
  appName: String
  apiKey: sensitive String
  port: Int
}

function entrypoint() -> Nothing {
  let apiKey: sensitive String = sensitive(`sk-test-12345`)
  print(`Connecting with key: ${apiKey}`)         // [REDACTED] (the whole text is sensitive, because it holds the key)

  let config: Config = { appName: `MyApp`, apiKey: apiKey, port: 3000 }
  print(config)
  // Config { appName: MyApp, apiKey: [REDACTED], port: 3000 }
}
```

---

## `.reveal()` — ask for the real value

```
let apiKey: sensitive String = sensitive(`sk-test-12345`)
let rawKey = apiKey.reveal()                    // String
print(rawKey)                                   // sk-test-12345
```

Pass the revealed value only to code that actually needs it.

---

## Text made from a sensitive value stays sensitive

Changing or joining a sensitive String gives another sensitive String. A plain fact about it, such as its length, is not sensitive:

```
let key: sensitive String = sensitive(`abc123`)
let upper = key.toUpperCase()          // still sensitive String
let trimmed = key.trim()               // still sensitive String
let combined = `Bearer ${key}`         // still sensitive String

let length = key.count()               // Int — NOT sensitive (length isn't secret)

print(upper)                           // [REDACTED]
print(length)                          // 6
```

---

## Showing sensitive values while you develop

`ynz run` takes a flag that prints sensitive values in plain text:

```text
ynz run entrypoint.ynz --reveal-sensitive
```

Without the flag, sensitive values stay `[REDACTED]`. With it, text made from a sensitive value prints in plain text too.
