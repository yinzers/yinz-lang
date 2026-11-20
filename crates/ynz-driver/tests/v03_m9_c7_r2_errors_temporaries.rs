//! v0.3-M9 C7 review round 2: the result of an `errors` call that nothing binds
//! (`mk(i).failed()`, `mk(i).or(fallback)`, an expression statement) owns its ok payload or its
//! error object, and the end of its statement releases it.
//!
//! Counters assert what the emitter RELEASES; AddressSanitizer asserts it releases nothing a
//! second holder still owns. Both optimization levels run at once per test.

mod support;

use support::{
    assert_asan_clean, assert_heap_balanced, assert_strings_balanced, at_both_optimization_levels,
    run_balanced,
};

fn assert_balanced_and_clean(label: &str, program: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(label, program, expected, no_optimize);
        assert_heap_balanced(label, &counters, no_optimize);
        assert_strings_balanced(label, &counters, no_optimize);
        assert_asan_clean(label, program, expected, no_optimize);
    });
}

const STRING_ERRORS_TEMPORARIES: &str = "
function make(n: Int) -> String errors {
  let a: String = `ab`
  return a.toUpperCase()
}
function entrypoint() -> Nothing {
  let i: Int = 0
  let t: Int = 0
  while (i < 20) {
    if (make(i).failed() == false) {
      t = t + 1
    }
    make(i)
    let kept: String = make(i).or(`z`)
    t = t + kept.count() + make(i).or(`z`).count()
    i = i + 1
  }
  print(t)
}
";

/// The `errors` call result of a string, read once and dropped, leaked its payload (High).
#[test]
fn an_unbound_string_errors_result_is_released_at_the_end_of_its_statement() {
    at_both_optimization_levels(|no_optimize| {
        let label = "c7r2_ec_string_tmp";
        let counters = run_balanced(label, STRING_ERRORS_TEMPORARIES, "100\n", no_optimize);
        assert_strings_balanced(label, &counters, no_optimize);
        assert_asan_clean(label, STRING_ERRORS_TEMPORARIES, "100\n", no_optimize);
    });
}

const SHAPE_ARRAY_ERRORS_TEMPORARIES: &str = "
shape Tag {
  name: String
}
function makeTag(n: Int) -> Tag errors {
  let tag: Tag = { name: `ab` }
  return tag
}
function makeList(n: Int) -> Array<String> errors {
  let list: Array<String> = [`a`, `b`]
  return list
}
function entrypoint() -> Nothing {
  let fallbackTag: Tag = { name: `z` }
  let fallbackList: Array<String> = []
  let i: Int = 0
  let t: Int = 0
  while (i < 10) {
    t = t + makeTag(i).or(fallbackTag).name.count()
    t = t + makeList(i).or(fallbackList).count()
    makeTag(i)
    makeList(i)
    let tag: Tag = makeTag(i).or(fallbackTag)
    let list: Array<String> = makeList(i).or(fallbackList)
    t = t + tag.name.count() + list.count()
    i = i + 1
  }
  print(t)
}
";

/// The same for a shape and an array: the temporary's ok payload is released once, after the
/// statement that copied what it keeps (High).
#[test]
fn an_unbound_shape_or_array_errors_result_is_released_once() {
    assert_balanced_and_clean(
        "c7r2_ec_shape_array_tmp",
        SHAPE_ARRAY_ERRORS_TEMPORARIES,
        "80\n",
    );
}

const BIGNUM_ERRORS_TEMPORARIES: &str = "
function make(n: Int) -> Number<100> errors {
  let a: Number<100> = 1.5
  return a + a
}
function entrypoint() -> Nothing {
  let zero: Number<100> = 0.0
  let i: Int = 0
  while (i < 10) {
    make(i)
    let kept: Number<100> = make(i).or(zero)
    print(kept + make(i).or(zero))
    i = i + 1
  }
}
";

/// A `number<100> errors` result is heap text; its unbound temporary leaked it (High).
#[test]
fn an_unbound_bignum_errors_result_is_released() {
    let expected = "6\n".repeat(10);
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(
            "c7r2_ec_bignum_tmp",
            BIGNUM_ERRORS_TEMPORARIES,
            &expected,
            no_optimize,
        );
        assert_strings_balanced("c7r2_ec_bignum_tmp", &counters, no_optimize);
        assert_asan_clean(
            "c7r2_ec_bignum_tmp",
            BIGNUM_ERRORS_TEMPORARIES,
            &expected,
            no_optimize,
        );
    });
}

const ERRORS_TEMPORARY_HANDED_ON: &str = "
shape Tag {
  name: String
}
function make(n: Int) -> String errors {
  let a: String = `ab`
  return a.toUpperCase()
}
function makeTag(n: Int) -> Tag errors {
  let tag: Tag = { name: `ab` }
  return tag
}
function eat(give text: String) -> Int {
  return text.count()
}
function find(share list: Array<String>, index: Int) -> Maybe<String> {
  return list.get(index)
}
function viaReturn(n: Int) -> String {
  return make(n).or(`z`)
}
function entrypoint() -> Nothing {
  let fallbackTag: Tag = { name: `z` }
  let words: Array<String> = []
  let held: Array<Tag> = []
  let i: Int = 0
  let t: Int = 0
  while (i < 10) {
    t = t + eat(make(i).or(`z`))
    words.add(make(i).or(`z`))
    held.add(makeTag(i).or(fallbackTag).copy())
    t = t + eat(find(words, 0).or(`z`))
    t = t + viaReturn(i).count()
    i = i + 1
  }
  print(t + words.count() + held.count())
}
";

/// An `errors` call a `give`, a store or a `return` reads through `.or` hands a copy on, never the
/// temporary's own payload: nothing is freed twice (High).
#[test]
fn an_errors_temporary_read_by_or_into_a_give_a_store_or_a_return_is_not_freed_twice() {
    assert_balanced_and_clean("c7r2_ec_tmp_handed_on", ERRORS_TEMPORARY_HANDED_ON, "80\n");
}
