use std::collections::HashMap;
use ynz_ast::nodes::{Item, Module};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

/// A registered `options` type with its declared variant names in declaration order.
#[derive(Clone, Debug)]
pub struct OptionsEntry {
    /// Variant names in declaration order. Tags are assigned as 0, 1, 2, … in this order.
    pub variants: Vec<String>,
    /// Source span of the options declaration, for diagnostics.
    pub decl_span: SourceSpan,
}

/// Registry of all options types in scope.
///
/// Populated from `Item::OptionsDecl` items plus the two built-in options types
/// (`SortOrder`, `Comparison`) registered at startup.
#[derive(Clone, Debug, Default)]
pub struct OptionsTable {
    /// Map from options type name to its entry.
    pub options: HashMap<String, OptionsEntry>,
}

impl OptionsTable {
    /// Create a new table pre-populated with the two built-in options types.
    pub fn with_builtins() -> Self {
        let mut t = Self::default();
        // Built-in options always in scope — no import needed.
        t.options.insert(
            "SortOrder".into(),
            OptionsEntry {
                variants: vec!["asc".into(), "desc".into()],
                decl_span: SourceSpan::new("<stdlib>", 0, 0),
            },
        );
        t.options.insert(
            "Comparison".into(),
            OptionsEntry {
                variants: vec!["equal".into(), "greater".into(), "less".into()],
                decl_span: SourceSpan::new("<stdlib>", 0, 0),
            },
        );
        t
    }

    /// Returns the tag (0-indexed) for a variant of the given options type.
    pub fn tag_for(&self, type_name: &str, variant: &str) -> Option<i8> {
        let entry = self.options.get(type_name)?;
        entry
            .variants
            .iter()
            .position(|v| v == variant)
            .map(|i| i as i8)
    }

    /// Returns the options type name that has this variant in scope — used for shorthand resolution.
    ///
    /// Returns `Ok(name)` when exactly one visible options type has the variant.
    /// Returns `Err(candidates)` when multiple types have it (ambiguous shorthand).
    /// Returns `Ok(None)` implicitly when called with a specific expected type — not used here;
    /// callers use `has_variant` for that case.
    pub fn resolve_shorthand<'a>(
        &'a self,
        variant: &str,
        expected_type: &'a str,
    ) -> ShorthandResult<'a> {
        // If the expected type is known, check it directly.
        if let Some(entry) = self.options.get(expected_type) {
            if entry.variants.contains(&variant.to_string()) {
                return ShorthandResult::Found(expected_type);
            }
        }
        // Otherwise scan all visible types.
        let mut candidates: Vec<&'a str> = self
            .options
            .iter()
            .filter(|(_, e)| e.variants.contains(&variant.to_string()))
            .map(|(name, _)| name.as_str())
            .collect();
        candidates.sort(); // deterministic order for diagnostics
        match candidates.len() {
            0 => ShorthandResult::NotFound,
            1 => ShorthandResult::Found(candidates[0]),
            _ => ShorthandResult::Ambiguous(candidates),
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.options.contains_key(name)
    }

    pub fn get(&self, name: &str) -> Option<&OptionsEntry> {
        self.options.get(name)
    }
}

#[derive(Debug)]
pub enum ShorthandResult<'a> {
    Found(&'a str),
    Ambiguous(Vec<&'a str>),
    NotFound,
}

/// Collect all `options` declarations from a module into an OptionsTable.
///
/// Validates:
/// - Duplicate options type names
/// - Empty / single variant bodies
/// - ≥256 variant count
/// - Duplicate variant names within one type
pub fn collect_options(module: &Module, diags: &mut DiagnosticBucket) -> OptionsTable {
    let mut table = OptionsTable::with_builtins();

    for item in &module.items {
        let Item::OptionsDecl(opt) = item else {
            continue;
        };

        // Empty body: parse was tolerant, typeck rejects.
        if opt.variants.is_empty() {
            diags.push(Diagnostic::error(
                opt.span.clone(),
                format!("`options {}` has no variants.", opt.name),
                "Add at least 2 variants: `options Status { active, inactive }`",
                "An options type with no variants can never hold a value — it's useless. \
                 Add at least 2 named variants.",
            ));
            continue;
        }

        // Single variant.
        if opt.variants.len() == 1 {
            diags.push(Diagnostic::error(
                opt.span.clone(),
                format!(
                    "`options {}` has only 1 variant — options types need at least 2.",
                    opt.name
                ),
                "Add a second variant, or use a const if you only need one named value:\n\
                 `const ACTIVE: int = 0`",
                "A single-variant options type has only one possible value and carries no \
                 information. It is almost always the wrong tool.",
            ));
            continue;
        }

        // ≥256 variants.
        if opt.variants.len() >= 256 {
            diags.push(Diagnostic::error(
                opt.span.clone(),
                format!(
                    "`options {}` has {} variants — the limit is 255.",
                    opt.name,
                    opt.variants.len()
                ),
                "Split into multiple options types, or model as `int` if you need more values.",
                "256+ variants almost always indicates a design smell. Options types work best \
                 for small, finite sets of named states.",
            ));
            continue;
        }

        // Duplicate variant names.
        let mut seen: HashMap<String, ()> = HashMap::new();
        let mut has_dup = false;
        for (v_name, v_span) in &opt.variants {
            if seen.contains_key(v_name) {
                diags.push(Diagnostic::error(
                    v_span.clone(),
                    format!("Duplicate variant `{v_name}` in `options {}`.", opt.name),
                    "Remove or rename one of the duplicate variants.",
                    "Each variant in an options type must have a unique name.",
                ));
                has_dup = true;
            } else {
                seen.insert(v_name.clone(), ());
            }
        }
        if has_dup {
            continue;
        }

        // Name clash with existing options type.
        if table.options.contains_key(&opt.name) {
            diags.push(Diagnostic::error(
                opt.name_span.clone(),
                format!("Options type `{}` is already declared.", opt.name),
                "Use a different name for this options type.",
                "Each options type must have a unique name in the module.",
            ));
            continue;
        }

        table.options.insert(
            opt.name.clone(),
            OptionsEntry {
                variants: opt.variants.iter().map(|(v, _)| v.clone()).collect(),
                decl_span: opt.span.clone(),
            },
        );
    }

    table
}
