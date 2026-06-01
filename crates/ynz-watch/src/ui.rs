use std::io::{self, IsTerminal, Write};

/// Clear the terminal using ANSI escape sequences.
///
/// No-op when:
/// - `no_clear` is true (user passed `--no-clear`)
/// - stdout is not a TTY (CI logs, pipes, redirections)
///
/// Uses `\x1b[2J\x1b[H` (erase display + move cursor to top-left).
/// Falls back gracefully on terminals that don't support ANSI — the sequence
/// is ignored rather than printing garbage.
pub fn clear(no_clear: bool) {
    if no_clear || !io::stdout().is_terminal() {
        return;
    }
    print!("\x1b[2J\x1b[H");
    let _ = io::stdout().flush();
}

/// Print the "rebuilding…" status line.
pub fn print_building(path: &str) {
    println!("▶ Building {path}…");
    let _ = io::stdout().flush();
}

/// Print the "clean build" status line with elapsed time.
pub fn print_success(elapsed_ms: u128) {
    println!("✓ Built in {elapsed_ms}ms");
    println!("✓ Watching…");
}

/// Print the "compile error" status line.
pub fn print_errors(count: usize, elapsed_ms: u128) {
    if count == 1 {
        println!("✗ 1 error in {elapsed_ms}ms");
    } else {
        println!("✗ {count} errors in {elapsed_ms}ms");
    }
    println!("✓ Watching…");
}

/// Print the "watching" idle prompt without clearing.
pub fn print_watching() {
    println!("✓ Watching…");
    let _ = io::stdout().flush();
}

/// Print a file-removed warning.
pub fn print_file_removed(path: &str) {
    println!("ynz watch: {path} vanished; watch continues, will re-pick-up on re-creation");
    let _ = io::stdout().flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    // WHY: ui primitives must never panic — a panic in print_watching() aborts the
    //      daemon mid-cycle and leaves a zombie child. Tests run in a non-TTY context
    //      which is the most likely place panics would occur (clear() skips ANSI).

    #[test]
    fn clear_no_clear_is_noop() {
        // no_clear=true → always a no-op regardless of TTY status.
        clear(true);
    }

    #[test]
    fn status_lines_do_not_panic() {
        // Each call must not panic in a non-TTY test context.
        print_building("foo.ynz");
        print_success(123);
        print_errors(3, 456);
        print_watching();
        print_file_removed("foo.ynz");
    }
}
