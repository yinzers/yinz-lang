---
name: "REF-sensitive"
description: "Values from environment variables and other secret sources are automatically protected from accidental logging. You have to consciously choose to reveal them."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Sensitive Values

Values from environment variables and other secret sources are automatically protected from accidental logging. You have to consciously choose to reveal them.

---

## How it works

`env.get()` returns `sensitive String` by default. No extra annotation needed:

```
let foundApiKey = env.get(`STRIPE_KEY`)         // a maybe: the key, or nothing
let apiKey = foundApiKey.or(``)                 // sensitive String
let foundDatabasePassword = env.get(`DB_PASSWORD`)     // a maybe: the password, or nothing
let databasePassword = foundDatabasePassword.or(``)  // sensitive String
let appName = `My App`                          // regular String
```

---

## Auto-redaction in all output

A sensitive value prints as `[REDACTED]` everywhere: `print()`, log lines, text with `${...}` in it, and when you print a whole shape that holds one:

```
let foundApiKey = env.get(`STRIPE_KEY`)
let apiKey = foundApiKey.or(``)

print(apiKey)                                   // [REDACTED]
log(`Connecting with key: ${apiKey}`)           // Connecting with key: [REDACTED]

shape Config {
  appName: String
  apiKey: sensitive String
  port: Number
}

let foundConfigKey = env.get(`API_KEY`)
let configKey = foundConfigKey.or(``)
let config: Config = { appName: `MyApp`, apiKey: configKey, port: 3000 }
print(config)
// Config { appName: `MyApp`, apiKey: [REDACTED], port: 3000 }
```

---

## `.reveal()` — ask for the real value

```
// Pass the secret only to code that actually needs it
let connection = database.connect(databasePassword.reveal())
let response = request.post(url, { headers: { authorization: apiKey.reveal() } })
```

The IDE warns you when you print a revealed value:

```
print(apiKey.reveal())
// IDE WARNING: Revealing sensitive value in print output.
// Ensure this is not in production logging.
```

---

## Marking any value as sensitive

Not just env vars — mark any value sensitive:

```
let socialSecurityNumber: sensitive String = sensitive(`123-45-6789`)
let cardNumber: sensitive String = sensitive(rawCardNumber)

print(socialSecurityNumber)    // [REDACTED]
```

---

## Text made from a sensitive value stays sensitive

Changing or joining a sensitive String gives another sensitive String. A plain fact about it, such as its length, is not sensitive:

```
let foundKey = env.get(`API_KEY`)      // a maybe: the key, or nothing
let key = foundKey.or(``)              // sensitive String
let upper = key.toUpperCase()          // still sensitive String
let trimmed = key.trim()               // still sensitive String
let combined = `Bearer ${key}`         // still sensitive String

let length = key.count()               // Int — NOT sensitive (length isn't secret)
```

---

## Sensitive values in error messages

Error messages hide sensitive data automatically:

```
// Error output when database connection fails:
// ERROR: Could not connect to database
//   URL: [REDACTED]
```

---

## Development flag

> **Not available yet.** The `--reveal-sensitive` flag is planned but does not exist today.

```
ynz run entrypoint.ynz --reveal-sensitive    // shows all sensitive values in output
```

The flag will not work in release builds. `ynz build --release` leaves it out, so secrets can't leak in production by accident.
