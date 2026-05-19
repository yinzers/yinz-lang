# Standard Library — CLI & Terminal

Scripting should be as easy as Python. One-file scripts, CLI tools, data processing — minimal boilerplate.

---

## CLI Arguments

```
let args = cli.args()
let verbose = cli.flag("verbose")           // -> boolean
let port = cli.option("port", "3000")       // -> string (with default)
```

---

## Terminal Output

```
terminal.print("Hello")
terminal.printColor("Error!", "red")
terminal.clear()
```

**Flush semantics**: `terminal.print()` writes to stdout without flushing the buffer. Output is flushed automatically when the buffer fills or the program exits normally. Explicit flushing is `terminal.flush()` — a separate, named call. There is no `terminal.println()` or equivalent that silently flushes.

**Why**: C++'s `std::endl` flushes the output buffer on every call because it combines "newline" and "flush" in one operation. Developers who use it thinking it means "end of line" unknowingly flush on every print, degrading performance by orders of magnitude on buffered output (clang-tidy ships `performance-avoid-endl` specifically for this). Yinz names the operations separately so the cost is always visible at the call site.

---

## Interactive Input

```
let name = terminal.ask("What's your name?")
let confirm = terminal.confirm("Continue?")             // -> boolean
let choice = terminal.choose("Pick one:", ["A", "B", "C"])
```

---

## Progress Tracking

```
let bar = terminal.progress(totalItems)
for (item in items) {
  processItem(item)
  bar.advance()
}
bar.complete("Done!")
```

---

## Expansion Candidates

- Subcommand parsing (like `git commit`, `git push`)
- Auto-generated help text from function signatures
- Tab completion generation
- Interactive table display
- Spinner animations for long tasks
- Color themes
- Terminal size detection
- Key press detection (raw input mode)
- Piping support (stdin/stdout as streams)
