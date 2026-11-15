//! v0.3-M9 C3b: every place a string is stored owns its bytes.
//!
//! The scope-exit release pass (C6/C7) frees each heap-backed local, strings included, so before
//! it lands a stored string must be either a fresh string the store takes by move, or an owned
//! copy of a borrowed one (a literal, a field or element read, a parameter, an `.or()` result).
//! Nothing frees a string yet, so a missing copy cannot show as a double free today. What shows
//! today is the number of heap strings the program allocates: one per fresh string, one more per
//! borrowed string stored, and none for a string stored at its last use or fresh. Each fixture's
//! header counts them. The ASan runs are the guard for the release pass: they stay clean now and
//! are what turns red if a release is ever wired to a store that aliased.
//!
//! All are High tier (memory safety: a stored string freed twice or freed while another holder
//! reads it; Golden Rule 8 cost: a copy where Rust moves). One fixture per store kind, three
//! tests each: the count (also asserts stdout), and AddressSanitizer at both optimization levels.

mod support;

use support::{assert_asan_clean_and_prints, build, run_counted};

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

/// `store_case!(case, fixture, expected_stdout, heap_strings, "bug it catches (High)")`.
macro_rules! store_case {
    ($case:ident, $fixture:literal, $expected:literal, $heap_strings:literal, $bug:literal) => {
        #[doc = $bug]
        mod $case {
            use crate::{assert_asan_clean_and_prints, assert_prints_and_allocates};

            #[test]
            fn heap_strings_allocated() {
                assert_prints_and_allocates($fixture, $expected, $heap_strings);
            }

            #[test]
            fn asan_optimized() {
                assert_asan_clean_and_prints($fixture, false, $expected);
            }

            #[test]
            fn asan_no_optimize() {
                assert_asan_clean_and_prints($fixture, true, $expected);
            }
        }
    };
}

store_case!(
    let_and_assign,
    "v0_3_m9_c3b_let_assign.ynz",
    "item-4\nitem-4\nnext-4\nplain\nplain\n",
    6,
    "`let b = a` or `b = a` stores `a`'s pointer while `a` is still read (two owners of one heap string), or copies a last-use / fresh string (a copy where Rust moves) (High)"
);

store_case!(
    shape_field,
    "v0_3_m9_c3b_fields.ynz",
    "label-7\nfixed\nfixed\n",
    5,
    "a shape field built from a still-read local, a literal or another field holds the same pointer as that source (High)"
);

store_case!(
    array_element,
    "v0_3_m9_c3b_array.ynz",
    "built-5\nbuilt-5\nlit\nmore\nlast-5\n",
    6,
    "an array literal, `add` or index assignment holds the pointer of a still-read local or a literal (High)"
);

store_case!(
    map_key_and_value,
    "v0_3_m9_c3b_map.ynz",
    "value-2\nvalue-2\nv\nvalue-2\n",
    6,
    "a map stores a string key by its raw pointer and a value by its pointer, aliasing a still-read local or a literal (High)"
);

store_case!(
    channel_and_background,
    "v0_3_m9_c3b_channel_background.ynz",
    "copy-3\ncopy-3\nlast-3\nlit\ntask-3\ntask-3\nover-3\n",
    7,
    "a string sent into a channel or `.copy()`'d into a task shares bytes with the sender, who still reads them (High)"
);

store_case!(
    return_and_call_result,
    "v0_3_m9_c3b_return_call.ynz",
    "a\nnone\nbuilt-2\nbuilt-2\n12\n",
    7,
    "a function returns its parameter's or an element's pointer, so the caller's fresh-looking call result aliases a string another holder owns (High)"
);

store_case!(
    runtime_dependent_or_default,
    "v0_3_m9_c3b_or_default.ynz",
    "a\nb\nnone\nnone\n",
    6,
    "`names[index].or(`none`)` is an element or a static literal depending on `index`, so the bound copy must be made for the expression, not decided from the value (High)"
);

store_case!(
    equal_text_compares_equal_after_a_copy,
    "v0_3_m9_c3b_content_equality.ynz",
    "true\ntrue\ntrue\nfalse\ntrue\n5\n5\n",
    6,
    "`array<string>.contains` and `array<shape>.contains` compared string cells by pointer, so the copy a store now makes is never found by the text it was copied from (High: wrong answer)"
);

store_case!(
    fixed_string_copy_has_strings_of_its_own,
    "v0_3_m9_c3b_fixed_copy.ynz",
    "p\nchanged\nq\n",
    5,
    "`fixed<string>.copy()` is refused once a string is not plain bits, or builds but shares its strings with the original (High: a program that built stops building; two owners of one string)"
);

/// Every `alloca` of a function's entry block comes before the loop's `llvm.stacksave`.
///
/// Bug caught (High: memory safety): an entry-block slot created after a loop was lowered sat
/// after that loop's `stacksave`, so the back-edge `stackrestore` released a live local's slot
/// and a callee's frame overwrote it. It showed as a crash only once a callee's frame grew
/// (`ynz_string_clone`), under AddressSanitizer, which makes entry allocas dynamic.
#[test]
fn entry_allocas_come_before_a_loops_stacksave() {
    let built = build("v0_3_m9_union_coerce_let.ynz", true);
    let ir = std::fs::read_to_string(&built.ir).expect("read emitted IR");
    let main_start = ir.find("define i32 @main()").expect("main is emitted");
    let entry: Vec<&str> = ir[main_start..]
        .lines()
        .skip(1)
        .take_while(|line| {
            !(line.ends_with(':') || line.contains(": ")) || line.starts_with("entry")
        })
        .collect();
    let stacksave = entry
        .iter()
        .position(|line| line.contains("@llvm.stacksave"))
        .expect("the fixture's `while` takes a stacksave in the entry block");
    let late_allocas: Vec<&&str> = entry[stacksave..]
        .iter()
        .filter(|line| line.contains("= alloca"))
        .collect();
    assert!(
        late_allocas.is_empty(),
        "entry-block allocas after the loop's stacksave would be released by its stackrestore: {late_allocas:?}"
    );
}
