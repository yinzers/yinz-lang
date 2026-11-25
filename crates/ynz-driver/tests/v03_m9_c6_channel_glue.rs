//! v0.3-M9 C6: a channel's element drop glue is the one release helper, not a second per-type
//! dispatch (`.claude/rules/authoritative-derivation.md`).
//!
//! Before this, `channel_drop_glue` freed an `array` element with the flat `ynz_array_drop`, so
//! the strings an `array<string>` payload owns leaked at channel teardown. No program can drop a
//! channel before C7 releases locals, so the glue cannot be run to completion yet; the IR is the
//! observable. High tier: data integrity (a leak per buffered element, or a second dispatch that
//! drifts from the helper).

mod support;

use support::build;

/// The text of the function `@name` in `ir`, from its `define` line to the closing brace.
fn function_text<'ir>(ir: &'ir str, name: &str) -> &'ir str {
    let start = ir
        .find(&format!("define internal void @\"{name}\""))
        .or_else(|| ir.find(&format!("define void @\"{name}\"")))
        .or_else(|| ir.find(&format!("define internal void @{name}(")))
        .or_else(|| ir.find(&format!("define void @{name}(")))
        .unwrap_or_else(|| panic!("`{name}` is not defined in the emitted IR"));
    let end = ir[start..]
        .find("\n}\n")
        .unwrap_or_else(|| panic!("`{name}` has no closing brace"));
    &ir[start..start + end]
}

/// Bug: the channel glue frees an array element flat, leaking the strings it owns (High).
// bug: channel teardown frees an Array<String> element flat, leaking the strings it owns | tier: High
#[test]
fn channel_element_glue_calls_the_release_function_of_the_element_type() {
    let built = build("v0_3_m9_c6_channel_glue.ynz", true);
    let ir = std::fs::read_to_string(&built.ir).expect("read emitted IR");
    let glue = function_text(&ir, "ynz_chan_drop_glue_array(string)");
    assert!(
        glue.contains("ynz_release_array(string)"),
        "the glue must call the element type's release function:\n{glue}"
    );
    assert!(
        !glue.contains("@ynz_array_drop"),
        "the glue must not free the array flat:\n{glue}"
    );
}
