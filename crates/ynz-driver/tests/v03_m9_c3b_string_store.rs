//! C3b: every place a string is stored owns its bytes. A stored string must be either a fresh
//! string the store takes by move, or an owned copy of a borrowed one (a literal, a field or
//! element read, a parameter, an `.or()` result). What shows is the number of heap strings the
//! program allocates: one per fresh string, one more per borrowed string stored, and none for a
//! string stored at its last use or fresh (a missing copy is an alias; an extra copy is a copy
//! where Rust moves). Each fixture's header counts them. The AddressSanitizer runs, at both
//! optimization levels, are the guard for the release pass: they turn red if a release is wired
//! to a store that aliased. One case per store kind.

mod support;

use support::{assert_asan_clean_and_prints, at_both_optimization_levels, build, run_counted};

/// Build the fixture, run it with the string counter on, and assert stdout and the number of heap
/// strings allocated.
fn assert_prints_and_allocates(fixture: &str, expected_stdout: &str, expected_strings: u64) {
    let built = build(fixture, false);
    let (output, counters) = run_counted(&built);
    assert_eq!(output.exit_code, 0, "stderr:\n{}", output.stderr);
    assert_eq!(output.stdout, expected_stdout, "{fixture}: wrong output");
    assert_eq!(
        counters.string_alloc, expected_strings,
        "{fixture}: expected {expected_strings} heap strings, saw {} (counters: {counters:?})",
        counters.string_alloc
    );
}

/// One named test per store kind of the `string store` run unit; each case is a test the runner
/// names.
macro_rules! string_store_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal, $heap_strings:literal;)+) => { $(
        // bug: a stored string aliases another holder's bytes (freed twice or while still read) or is copied where Rust moves, so the heap string count is off or ASan reports | tier: High
        #[test]
        fn $case_name() {
            assert_prints_and_allocates($fixture, $expected_stdout, $heap_strings);
            at_both_optimization_levels(|no_optimize| {
                assert_asan_clean_and_prints($fixture, no_optimize, $expected_stdout);
            });
        }
    )+ };
}

string_store_high_case! {
    // case: `let b = a` or `b = a` stores `a`'s pointer while `a` is still read (two owners of one heap string), or copies a last-use / fresh string (a copy where Rust moves)
    let_and_assign, "v0_3_m9_c3b_let_assign.ynz", "item-4\nitem-4\nnext-4\nplain\nplain\n", 6;
    // case: a shape field built from a still-read local, a literal or another field holds the same pointer as that source
    shape_field, "v0_3_m9_c3b_fields.ynz", "label-7\nfixed\nfixed\n", 5;
    // case: an array literal, `add` or index assignment holds the pointer of a still-read local or a literal
    array_element, "v0_3_m9_c3b_array.ynz", "built-5\nbuilt-5\nlit\nmore\nlast-5\n", 6;
    // case: a map stores a string key by its raw pointer and a value by its pointer, aliasing a still-read local (key or value) or a literal
    map_key_and_value, "v0_3_m9_c3b_map.ynz", "value-2\nshared-2\nvalue-2\nv\nvalue-2\ns\n", 13;
    // case: a string sent into a channel or `.copy()`'d into a task shares bytes with the sender, who still reads them
    channel_and_background, "v0_3_m9_c3b_channel_background.ynz", "copy-3\ncopy-3\nlast-3\nlit\ntask-3\ntask-3\nover-3\n", 7;
    // case: a function returns its parameter's or an element's pointer, so the caller's fresh-looking call result aliases a string another holder owns
    return_and_call_result, "v0_3_m9_c3b_return_call.ynz", "a\nnone\nbuilt-2\nbuilt-2\n12\n", 7;
    // case: `names[index].or(`none`)` is an element or a static literal depending on `index`, so the bound copy must be made for the expression, not decided from the value
    runtime_dependent_or_default, "v0_3_m9_c3b_or_default.ynz", "a\nb\nnone\nnone\n", 6;
    // case: `Array<String>.contains` and `Array<shape>.contains` compared string cells by pointer, so the copy a store now makes is never found by the text it was copied from
    equal_text_compares_equal_after_a_copy, "v0_3_m9_c3b_content_equality.ynz", "true\ntrue\ntrue\nfalse\ntrue\n5\n5\n", 6;
    // case: `Fixed<String>.copy()` is refused once a string is not plain bits, or builds but shares its strings with the original
    fixed_string_copy_has_strings_of_its_own, "v0_3_m9_c3b_fixed_copy.ynz", "p\nchanged\nq\n", 5;
    // case: `.copy()` of a shape or a map, and a `Maybe<String>` stored in a field or returned, hold the source's string pointer, so releasing both frees it twice
    copied_containers_own_their_strings, "v0_3_m9_c3b_container_copy.ynz", "t-1\nv-1\nx-1\nx-1\n", 11;
    // case: `Array<Tag>.copy()` memcpys each element, so the copy's `name` fields hold the original's string pointers and releasing both frees them twice
    array_of_shapes_copy_has_strings_of_its_own, "v0_3_m9_c3b_array_shape_copy.ynz", "t-1\nu-1\nt-1\nu-1\n", 4;
    // case: returning a `Maybe<Tag>` memcpys the payload shape, so its `name` is the array element's pointer and a release of both frees it twice; a binding of the element read (`let found = tags.get(0)`) only reads it and is a view, which owns no string
    shape_in_a_maybe_owns_its_strings, "v0_3_m9_c3b_maybe_shape.ynz", "t-1\nt-1\n", 2;
    // case: a string wrapped into a union slot (`let`, field, argument) was never copied, because `lower_expr` asked for the expression's type after the slot type replaced it, so a literal or a still-read string was shared with the envelope
    string_entering_a_union_slot, "v0_3_m9_c3b_union_slot.ynz", "2\n2\n2\n2\n2\nb-1\n", 6;
    // case: `Array<String>.copy()` clones the buffer and shares each string pointer with the original
    array_of_strings_copy_has_strings_of_its_own, "v0_3_m9_c3b_array_string_copy.ynz", "a-1\nb-1\nc-1\nb-1\n", 5;
    // case: the defaults initializer stores a hidden string default without the store rule (copy a borrowed string, move a fresh one), so an instance holds a string no one owns
    hidden_string_default_is_copied_per_instance, "v0_3_m9_c3b_hidden_default.ynz", "default\ndefault\ndefault\nnote\nn-1\n", 9;
    // case: `names[0].or(...)` is classified as a call of a function that is also named `or`, so `let got` takes the element's own pointer and the element and `got` free one string twice
    builtin_method_named_like_a_function_stays_borrowed, "v0_3_m9_c3b_builtin_name_clash.ynz", "a\n", 2;
    // case: `.copy()` of a `Map<String, Int>` or `Map<Int, String>` clones the wrong side: an `int` read as a string pointer (crash), or the string side shared with the source
    map_copy_copies_only_the_string_side, "v0_3_m9_c3b_map_copy_kinds.ynz", "7\nv-1\n7\nv-1\n", 6;
    // case: `list[i] = value` on a `Fixed<String>` stores a still-read local or an element read by pointer (two owners), or copies a fresh or last-use string
    fixed_index_assign, "v0_3_m9_c3b_fixed_index_assign.ynz", "fresh-3\nnamed\nfresh-3\n", 6;
    // case: a binding reassigned after its string moved out frees or aliases the moved string
    assign_after_move, "v0_3_m9_c3b_assign_after_move.ynz", "first-5\n", 2;
}
