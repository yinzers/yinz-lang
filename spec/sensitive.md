# Sensitive Values

Values from environment variables and other secret sources are automatically protected from accidental logging. You have to consciously choose to reveal them.

---

## How it works

`env.get()` returns `sensitive string` by default. No extra annotation needed:

```
let apiKey = env.get("STRIPE_KEY").or("")      // sensitive string
let dbPass = env.get("DB_PASSWORD").or("")     // sensitive string
let appName = "My App"                          // regular string
```

---

## Auto-redaction in all output

Sensitive values print as `[REDACTED]` everywhere — `print()`, log statements, string interpolation, and default type representations:

```
let apiKey = env.get("STRIPE_KEY").or("")

print(apiKey)                                   // [REDACTED]
log(`Connecting with key: ${apiKey}`)           // Connecting with key: [REDACTED]

type Config {
  appName: string
  apiKey: sensitive string
  port: number
}

let config: Config = { appName: "MyApp", apiKey: env.get("API_KEY").or(""), port: 3000 }
print(config)
// Config { appName: "MyApp", apiKey: [REDACTED], port: 3000 }
```

---

## `.reveal()` — explicit opt-in to access the raw value

```
// Pass to services that actually need the secret
let conn = database.connect(dbPass.reveal())
let res = http.post(url, { headers: { authorization: apiKey.reveal() } })
```

The IDE warns when you reveal in output:

```
print(apiKey.reveal())
// IDE WARNING: Revealing sensitive value in print output.
// Ensure this is not in production logging.
```

---

## Marking any value as sensitive

Not just env vars — mark any value sensitive:

```
let ssn: sensitive string = sensitive("123-45-6789")
let card: sensitive string = sensitive(rawCardNumber)

print(ssn)    // [REDACTED]
```

---

## Sensitivity propagates through string operations

String operations on sensitive values stay sensitive. Non-string extractions don't:

```
let key = env.get("API_KEY").or("")    // sensitive string
let upper = key.toUpper()              // still sensitive string
let trimmed = key.trim()               // still sensitive string
let combined = `Bearer ${key}`         // still sensitive string

let length = key.length                // number — NOT sensitive (length isn't secret)
```

---

## Sensitive values in error messages

Error messages that might contain sensitive data are auto-redacted:

```
// Error output when database connection fails:
// ERROR: Could not connect to database
//   URL: [REDACTED]
//   (use --reveal-sensitive flag for local debugging)
```

---

## Development flag

```
ynz run main.ynz --reveal-sensitive    // shows all sensitive values in output
```

This flag is stripped from release builds. `ynz build --release` makes it unavailable, preventing accidental exposure in production.
