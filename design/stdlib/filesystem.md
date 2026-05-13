# Standard Library — File System

---

## File Operations

All file operations use the `errors` system.

```
let content = file.read("data.txt")                      // -> string errors
let lines = file.readLines("data.txt")                   // -> array<string> errors (loads all lines)
let lazyLines = file.lines("data.txt")                   // -> Iterable<string> errors (lazy — one line at a time)
let bytes = file.readBytes("image.png")                  // -> array<byte> errors

file.write("output.txt", content)                        // -> nothing errors
file.appendLine("log.txt", "new entry")                  // -> nothing errors

if (file.exists("config.ynz")) { ... }
let size = file.size("data.txt")                         // -> int errors
let modified = file.lastModified("data.txt")             // -> Date errors
```

---

## Directory Operations

```
let files = directory.list("/path/to/dir")               // -> array<string> errors
directory.create("/path/to/new")                         // -> nothing errors
directory.delete("/path/to/old")                         // -> nothing errors
```

---

## Path Utilities

```
let full = path.join("src", "utils", "helpers.ynz")
let ext = path.extension("photo.jpg")                    // "jpg"
let name = path.filename("/app/src/main.ynz")            // "main.ynz"
let dir = path.directory("/app/src/main.ynz")            // "/app/src"
```

---

## Expansion Candidates

- File watching (notify on changes)
- Temp file/directory creation with auto-cleanup
- File locking
- Streaming reads/writes for large files
- Glob pattern matching
- File permissions management
- Symlink support
- Archive support (zip, tar, gzip)
- File copy/move helpers
