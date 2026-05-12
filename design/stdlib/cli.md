# Standard Library — CLI & Terminal

Scripting should be as easy as Python. One-file scripts, CLI tools, data processing — minimal boilerplate.

---

## CLI Arguments

```
let args = cli.args()
let verbose = cli.flag("verbose")           // -> bool
let port = cli.option("port", "3000")       // -> string (with default)
```

---

## Terminal Output

```
terminal.print("Hello")
terminal.printColor("Error!", "red")
terminal.clear()
```

---

## Interactive Input

```
let name = terminal.ask("What's your name?")
let confirm = terminal.confirm("Continue?")             // -> bool
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
