use std::path::Path;

use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

/// Read a source file from disk, verify it is valid UTF-8, and return the text.
///
/// On invalid UTF-8, pushes a diagnostic and returns `None`.
pub fn load_source(path: &Path, diags: &mut DiagnosticBucket) -> Option<String> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            diags.push(Diagnostic::error(
                SourceSpan::new(path.display().to_string(), 0, 0),
                format!("Could not read `{}`: {e}", path.display()),
                "Check that the file exists and you have permission to read it.",
                "The compiler needs to read your source file before it can compile it.",
            ));
            return None;
        }
    };

    match String::from_utf8(bytes.clone()) {
        Ok(s) => Some(s),
        Err(e) => {
            // Point to the first invalid byte.
            let offset = e.utf8_error().valid_up_to();
            diags.push(Diagnostic::error(
                SourceSpan::new(path.display().to_string(), offset, offset + 1),
                format!("`{}` contains bytes that are not valid UTF-8.", path.display()),
                "Save the file with UTF-8 encoding (most editors use this by default).",
                "Yinz source files must be UTF-8. \
                 Other encodings (Latin-1, Windows-1252, etc.) may look similar but will cause this error.",
            ));
            None
        }
    }
}
