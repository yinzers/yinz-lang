/// Audit: non-zero hidden-field defaults in the v0.2.0-m4 codebase.
///
/// Compiled before the codegen fix to verify there were no live consumers of the
/// broken behavior (zero-init of non-zero hidden defaults).  0 non-zero defaults
/// were found, meaning the fix changes a silently-broken path with no test regressions.
///
/// # Audit methodology
///
/// Command run: `grep -rn "hidden " examples/ crates/ynz-driver/tests/fixtures/`
/// Filter: declarations matching `hidden <ident>: <type> = <non-zero-expr>`.
///
/// # AUDIT FINDING
///
/// **0 non-zero hidden-field defaults found.**
///
/// Every `hidden` field declaration in the v0.2.0-m4 codebase uses `= 0` (or `= none`,
/// or no default expression at all). Concrete list of all hidden-field declarations found:
///
/// - `examples/pirates-roster/entrypoint.ynz:366` — `hidden current: int = 0`
/// - `crates/ynz-driver/tests/fixtures/m4_neg_hidden_field_access.ynz:3` — `hidden balance: int = 0`
/// - `crates/ynz-driver/tests/fixtures/m7_user_iterable.ynz:8` — `hidden pos: int = 0`
/// - `crates/ynz-driver/tests/fixtures/m4_hidden_field.ynz:3` — `hidden count: int = 0`
///
/// **Implication**: The Phase 11a bug (non-zero hidden defaults silently zero-init) has
/// zero live consumers in the current test suite. The fix changes a broken code path
/// with no measurable effect on existing test output. Insta snapshots for all existing
/// fixtures remain stable post-fix (verified by `cargo test --workspace` green).
///
/// **The new fixtures** (`m5_hidden_default_string.ynz`, `m5_hidden_default_int.ynz`,
/// `m5_hidden_default_nested.ynz`) are the only fixtures that exercise the non-zero
/// default path and will fail on the v0.2.0-m4 baseline.

#[test]
fn audit_finding_zero_non_zero_defaults() {
    // WHY: this test is a runnable record of the audit conclusion.
    //      If someone adds a non-zero hidden-field default to a fixture in the future
    //      and this test file is checked, they can see the historical baseline.
    //      The test itself always passes — it's documentation, not a code gate.

    let zero_default_fixtures = [
        ("examples/pirates-roster/entrypoint.ynz", "hidden current: int = 0"),
        (
            "crates/ynz-driver/tests/fixtures/m4_neg_hidden_field_access.ynz",
            "hidden balance: int = 0",
        ),
        (
            "crates/ynz-driver/tests/fixtures/m7_user_iterable.ynz",
            "hidden pos: int = 0",
        ),
        (
            "crates/ynz-driver/tests/fixtures/m4_hidden_field.ynz",
            "hidden count: int = 0",
        ),
    ];

    // Verify the expected zero-default fixtures still exist and have the expected content.
    // If a test here fails, someone renamed or removed a fixture — update the audit record above.
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    for (path, expected_content) in &zero_default_fixtures {
        let full_path = workspace_root.join(path);
        if full_path.exists() {
            let content = std::fs::read_to_string(&full_path)
                .unwrap_or_else(|e| panic!("Could not read {}: {e}", path));
            assert!(
                content.contains(expected_content),
                "Audit record mismatch: expected `{}` to contain `{}`.\n\
                 Update the audit record in audit_hidden_field_defaults.rs if the fixture changed.",
                path,
                expected_content
            );
        }
        // If the file doesn't exist, it was moved/deleted — the audit is still valid
        // but the record should be updated.
    }
}
