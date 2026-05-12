# Sensitive Values — Design Decisions

User spec: `spec/sensitive.md`

---

## `sensitive` as a Type Modifier, Not a Wrapper Type

`sensitive string` is a modifier on an existing type, like `maybe`. Not a wrapper type like `Secret<String>`.

**Why**: `maybe string` is the established pattern in the language for type modifiers. `sensitive string` follows the same convention — readable, composable, consistent. A `Secret<string>` wrapper would require special unwrapping syntax instead of the intuitive `.reveal()`.

---

## `env.get()` Returns `sensitive string` by Default

Environment variables are the primary source of secrets. Making `env.get()` return `sensitive string` automatically means developers don't have to remember to annotate secrets — the safe path is the default path.

**Why this matters**: The most common security mistake is not intentionally logging secrets — it's accidentally including them in debug output, error messages, or log statements. Making accidental exposure impossible by default is better than making it possible but documented as bad practice.

---

## Auto-Redaction Everywhere

Print, log, string interpolation, type default representation — sensitive values are redacted in all output paths without any developer action.

**Why all output paths**: Developers log things for debugging. If only some output paths redact, developers will accidentally find a path that doesn't. Comprehensive redaction closes all the gaps.

---

## `.reveal()` is Explicit and IDE-Warned

Getting the raw value requires calling `.reveal()`, which is clearly named and produces an IDE warning when used in output contexts.

**Why not just return the string and let developers be careful**: That's exactly how every other language works, and it fails constantly. Accidental API key logging has caused real security incidents across major companies. Requiring explicit action + warning on suspicious usage catches the mistake at the point of error.

---

## Propagation Through String Operations

String operations on sensitive values return sensitive values. Non-string extractions (`.length`, `.exists()`) return regular types.

**Why**: A developer might think `key.toUpper()` produces a regular string. It doesn't — the uppercased key is just as secret as the original. Automatic propagation prevents leaks through transformation chains.

**Why `.length` is not sensitive**: The length of a password or API key is not a secret. Treating it as sensitive would break common patterns like checking if an env var was set at all.

---

## `--reveal-sensitive` Flag Stripped from Release Builds

The development debugging flag is unavailable in release binaries.

**Why**: A flag that reveals all secrets cannot exist in production. Stripping it at compile time makes it impossible to accidentally enable it — not just undocumented or discouraged. The compiler enforces the security boundary.

---

## Error Messages Auto-Redacted

If an errors-system error message would contain a sensitive value (like a database URL), it's redacted automatically.

**Why**: Error messages end up in log files, monitoring dashboards, Slack alerts, and bug reports. All of these are potential leak surfaces. Auto-redaction in error messages closes the gap that developers often miss.
