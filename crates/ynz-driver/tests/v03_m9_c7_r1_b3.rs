//! v0.3-M9 C7 review round 1, builder 3: the plain-path leak producers the fixer round found
//! (debug text of a printed value, the per-character string of a string loop, a `number` wider
//! than 34 digits, an `errors` result's payload).
//!
//! Counters assert what the emitter RELEASES; AddressSanitizer asserts it releases nothing a
//! second holder still owns. Both optimization levels run at once per test.

mod support;

use support::{
    assert_asan_clean, assert_heap_balanced, assert_strings_balanced, at_both_optimization_levels,
    run_balanced,
};

const PRINTED_SHAPES: &str = "
shape R {
  count: Int
}
shape Outer {
  inner: R
  name: String
}
function entrypoint() -> Nothing {
  let r: R = { count: 1 }
  print(r)
  let inner: R = { count: 2 }
  let o: Outer = { inner: inner, name: `box` }
  print(o)
  let rows: Array<R> = [{ count: 3 }, { count: 4 }]
  print(rows)
  let i: Int = 0
  while (i < 10) {
    print(r)
    i = i + 1
  }
}
";

const PRINTED_CONTAINERS: &str = "
shape R {
  count: Int
  tags: Array<String>
}
function entrypoint() -> Nothing {
  let r: R = { count: 1, tags: [`a`, `b`] }
  print(r)
  let rows: Array<Array<Int>> = [[1], [2, 3]]
  print(rows)
  let words: Array<String> = [`p`, `q`]
  print(words)
  print(`n is ${r.count}`)
}
";

fn printed_shapes_expected() -> String {
    let mut text = String::from(
        "R { count: 1 }\nOuter { inner: R { count: 2 }, name: box }\n[R { count: 3 }, R { count: 4 }]\n",
    );
    text.push_str(&"R { count: 1 }\n".repeat(10));
    text
}

const STRING_LOOP: &str = "
function entrypoint() -> Nothing {
  let total: Int = 0
  for (c in `hello`) {
    total = total + c.count()
  }
  let word: String = `wor`.toUpperCase()
  for (letter in word) {
    if (letter == `O`) {
      total = total + 10
    }
  }
  print(total)
}
";

/// Counters say every string was freed; AddressSanitizer says none was freed twice or early.
fn assert_strings_balanced_and_clean(label: &str, program: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(label, program, expected, no_optimize);
        assert_strings_balanced(label, &counters, no_optimize);
        assert_asan_clean(label, program, expected, no_optimize);
    });
}

const BIGNUM_LOCAL: &str = "
function entrypoint() -> Nothing {
  let a: Number<100> = 1.5
  let b: Number<100> = 2.25
  let c: Number<100> = a + b
  print(c)
}
";

const BIGNUM_TEMPORARY: &str = "
function entrypoint() -> Nothing {
  let a: Number<100> = 1.5
  let b: Number<100> = 2.25
  print(a + b)
}
";

const BIGNUM_LOOP: &str = "
function entrypoint() -> Nothing {
  let a: Number<100> = 1.5
  let b: Number<100> = 2.25
  let i: Int = 0
  while (i < 100) {
    print(a + b)
    let c: Number<100> = a + b
    print(c)
    i = i + 1
  }
}
";

const STRING_LOOP_KEEPS: &str = "
function firstVowel(word: String) -> String {
  for (letter in word) {
    if (letter == `e` || letter == `o`) {
      return letter
    }
  }
  return `none`
}

function entrypoint() -> Nothing {
  let kept: Array<String> = []
  let last: String = `-`
  for (c in `abc`) {
    let same: String = c
    kept.add(c)
    last = c
    print(same)
  }
  print(kept.count())
  print(last)
  print(firstVowel(`hello`))
  print(firstVowel(`xyz`))
}
";

const BIGNUM_HOLDERS: &str = "
shape Money {
  amount: Number<100>
  label: String
}
function twice(share x: Number<100>) -> Number<100> {
  let y: Number<100> = x + x
  return y
}
function pass(x: Number<100>) -> Number<100> {
  return x
}
function entrypoint() -> Nothing {
  let a: Number<100> = 1.5
  let b: Number<100> = 2.25
  let c: Number<100> = a + b
  let d: Number<100> = c
  print(d)
  c = c + a
  print(c)
  print(twice(c))
  let e: Number<100> = pass(d)
  print(e)
  let m: Money = { amount: c, label: `x` }
  print(m)
  if (a < b) {
    let f: Number<100> = a * b
    print(f)
  }
}
";

const ERRORS_NUMBER: &str = "
function price(n: Int) -> Number errors {
  let total: Number = 1.5
  return total
}

function priceTwo(n: Int) -> Number errors {
  return 2.5
}

function relayPrice(n: Int) -> Number errors {
  let inner = price(n)
  return inner
}

function entrypoint() -> Nothing {
  let i: Int = 0
  let sum: Number = 0.0
  while (i < 50) {
    let a = price(i)
    let b = priceTwo(i)
    let c = relayPrice(i)
    sum = sum + a.or(0.0) + b.or(0.0) + c.or(0.0)
    i = i + 1
  }
  print(sum)
}
";

const ERRORS_SHAPE: &str = "
shape Tiny {
  flag: Boolean
}
shape Big {
  label: String
  count: Int
}
shape Thing = Tiny | Big

function makeBig(count: Int) -> Big errors {
  let big: Big = { label: `made`, count: count }
  return big
}

function makeThing(count: Int) -> Thing errors {
  let big: Big = { label: `made`, count: count }
  return big
}

function describe(share thing: Thing) -> Int {
  if (thing) {
    is Tiny => {
      return 0
    }
    is Big => {
      return thing.count
    }
  }
  return -1
}

function relayBig(count: Int) -> Big errors {
  let inner = makeBig(count)
  return inner
}

function entrypoint() -> Nothing {
  let fallback: Big = { label: `none`, count: 0 }
  let seed: Big = { label: `seed`, count: 0 }
  let fallbackThing: Thing = seed
  let i: Int = 0
  let total: Int = 0
  while (i < 50) {
    let one = makeBig(i)
    let two = makeThing(i)
    let three = relayBig(i)
    let relayed: Big = three.or(fallback)
    let got: Big = one.or(fallback)
    total = total + got.count + relayed.count + describe(two.or(fallbackThing))
    i = i + 1
  }
  print(total)
}
";

const ERRORS_MAYBE: &str = "
function pick(n: Int) -> Maybe<Int> errors {
  let items: Array<Int> = [4, 5]
  let got = items.get(n)
  return got
}

function pickWord(n: Int) -> Maybe<String> errors {
  let words: Array<String> = [`a`, `b`]
  let got = words.get(n)
  return got
}

function entrypoint() -> Nothing {
  let i: Int = 0
  let total: Int = 0
  let none_int: Maybe<Int> = none
  while (i < 30) {
    let one = pick(i % 3)
    let word = pickWord(1)
    total = total + one.or(none_int).or(0) + 1
    i = i + 1
  }
  print(total)
}
";

const OPTIONS_TEXT: &str = "
options Level { low, high }

shape Reading {
  count: Int
  level: Level
}

function entrypoint() -> Nothing {
  let level: Level = Level.high
  let name: String = `yinz`
  print(level.toString())
  print(`${name}-${level}`)
  let reading: Reading = { count: 2, level: Level.low }
  print(reading)
}
";

const UNION_ARGUMENTS: &str = "
shape Circle {
  radius: Float
}
shape Mixed = Circle | String

function kindShare(share value: Mixed) -> Int {
  if (value) {
    is Circle => {
      return 1
    }
    else => {
      return 2
    }
  }
}

function kindBare(value: Mixed) -> Int {
  if (value) {
    is Circle => {
      return 1
    }
    else => {
      return 2
    }
  }
}

function kindGive(give value: Mixed) -> Int {
  if (value) {
    is Circle => {
      return 1
    }
    else => {
      return 2
    }
  }
}

function entrypoint() -> Nothing {
  let circle: Circle = { radius: 1.5 }
  let name: String = `yinz`.toUpperCase()
  let i: Int = 0
  let total: Int = 0
  while (i < 20) {
    total = total + kindShare(`literal`)
    total = total + kindBare(`literal`)
    total = total + kindGive(`literal`)
    total = total + kindShare(name)
    total = total + kindShare(`fresh`.toUpperCase())
    total = total + kindGive(name.toLowerCase())
    total = total + kindShare(circle)
    i = i + 1
  }
  print(total)
  print(name)
}
";

/// One named test per producer of the `plain-path leak producers` run unit; each case runs at both optimization levels (counters and AddressSanitizer).
macro_rules! r1_b3_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a plain-path producer's temporary or second holder (print debug text, string-loop character, wide number, errors payload, options text, union envelope) leaks or is freed twice, silently at exit 0 | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

r1_b3_high_case! {
    // case: The debug text a non-string `print` argument builds (a shape's, an array's, and the nested text of a shape field or an array item) is a heap string nobody released
    the_debug_text_a_print_builds_is_released => {
        let expected = printed_shapes_expected();
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced("c7_r1_debug_text", PRINTED_SHAPES, &expected, no_optimize);
            assert_heap_balanced("c7_r1_debug_text", &counters, no_optimize);
            assert_strings_balanced("c7_r1_debug_text", &counters, no_optimize);
            assert_asan_clean("c7_r1_debug_text", PRINTED_SHAPES, &expected, no_optimize);
        });
    }
    // case: The debug text of a shape with an array field, of nested arrays and of an array of strings is a heap string nobody released
    the_debug_text_of_printed_containers_is_released => {
        at_both_optimization_levels(|no_optimize| {
            let containers = "R { count: 1, tags: [a, b] }\n[[1], [2, 3]]\n[p, q]\nn is 1\n";
            let counters = run_balanced(
                "c7_r1_debug_containers",
                PRINTED_CONTAINERS,
                containers,
                no_optimize,
            );
            assert_heap_balanced("c7_r1_debug_containers", &counters, no_optimize);
            assert_strings_balanced("c7_r1_debug_containers", &counters, no_optimize);
            assert_asan_clean(
                "c7_r1_debug_containers",
                PRINTED_CONTAINERS,
                containers,
                no_optimize,
            );
        });
    }
    // case: The per-character string of a string `for` loop variable is never released
    a_string_loops_per_character_string_is_released => {
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced("c7_r1_for_string", STRING_LOOP, "15\n", no_optimize);
            assert_strings_balanced("c7_r1_for_string", &counters, no_optimize);
            assert_asan_clean("c7_r1_for_string", STRING_LOOP, "15\n", no_optimize);
        });
    }
    // case: A `number` wider than 34 digits is heap text; a local of that type was never released
    a_bignum_local_is_released => {
        assert_strings_balanced_and_clean("c7_r1_bignum_local", BIGNUM_LOCAL, "3.75\n");
    }
    // case: The temporary a `print(a + b)` of two wide numbers builds is heap text nobody released
    a_bignum_temporary_is_released => {
        assert_strings_balanced_and_clean("c7_r1_bignum_temporary", BIGNUM_TEMPORARY, "3.75\n");
    }
    // case: The wide-number local and temporary of a loop leak per iteration
    a_bignum_local_and_temporary_in_a_loop_are_released => {
        let loop_expected = "3.75\n".repeat(200);
        assert_strings_balanced_and_clean("c7_r1_bignum_loop", BIGNUM_LOOP, &loop_expected);
    }
    // case: A string loop's variable stored, assigned, bound or returned inside the body leaves the per-character release exactly one owner each: no leak and no double free
    a_string_loop_variable_that_is_kept_is_copied_not_released_twice => {
        let expected = "a\nb\nc\n3\nc\ne\nnone\n";
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced(
                "c7_r1_for_string_keep",
                STRING_LOOP_KEEPS,
                expected,
                no_optimize,
            );
            assert_strings_balanced("c7_r1_for_string_keep", &counters, no_optimize);
            assert_asan_clean(
                "c7_r1_for_string_keep",
                STRING_LOOP_KEEPS,
                expected,
                no_optimize,
            );
        });
    }
    // case: A bignum is owned text, so every second holder copies it (a `let`, a field, a returned parameter) and the release frees each copy once: no leak and no double free
    a_bignum_copied_into_a_let_a_field_and_a_return_is_released_once_each => {
        let expected = "3.75\n5.25\n10.5\n3.75\nMoney { amount: 5.25, label: x }\n3.375\n";
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced(
                "c7_r1_bignum_holders",
                BIGNUM_HOLDERS,
                expected,
                no_optimize,
            );
            assert_strings_balanced("c7_r1_bignum_holders", &counters, no_optimize);
            assert_asan_clean(
                "c7_r1_bignum_holders",
                BIGNUM_HOLDERS,
                expected,
                no_optimize,
            );
        });
    }
    // case: The payload of an `errors` result of type `number` is a heap cell the `return` made; the result's release left it
    an_errors_result_of_number_releases_its_payload_cell => {
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced("c7_r1_ec_number", ERRORS_NUMBER, "275.0\n", no_optimize);
            assert_heap_balanced("c7_r1_ec_number", &counters, no_optimize);
            assert_asan_clean("c7_r1_ec_number", ERRORS_NUMBER, "275.0\n", no_optimize);
        });
    }
    // case: A shape local returned through an `errors` result moves into the result: the `return` neither copies it a second time nor leaves the local's strings behind
    a_shape_returned_through_an_errors_result_is_moved_not_copied_twice => {
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced("c7_r1_ec_shape", ERRORS_SHAPE, "3675\n", no_optimize);
            assert_heap_balanced("c7_r1_ec_shape", &counters, no_optimize);
            assert_strings_balanced("c7_r1_ec_shape", &counters, no_optimize);
            assert_asan_clean("c7_r1_ec_shape", ERRORS_SHAPE, "3675\n", no_optimize);
        });
    }
    // case: The envelope cell an `errors` result of a `maybe` carries is owned by the caller's binding, so its release frees it (High: it was `NotOwner`, so every call leaked a cell).
    an_errors_result_of_maybe_releases_its_envelope_cell => {
        let expected = "120\n";
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced("c7_r1_ec_maybe", ERRORS_MAYBE, expected, no_optimize);
            assert_heap_balanced("c7_r1_ec_maybe", &counters, no_optimize);
            assert_asan_clean("c7_r1_ec_maybe", ERRORS_MAYBE, expected, no_optimize);
        });
    }
    // case: The name an `options` value turns into (`toString`, `${}`, the field of a printed shape) is a counted string `ynz_string_from_static` makes: it is released like any other text (High: it was `borrowed`/not fresh, so every use leaked one string).
    the_text_of_an_options_value_is_released => {
        let expected = "high\nyinz-high\nReading { count: 2, level: low }\n";
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced("c7_r1_options_text", OPTIONS_TEXT, expected, no_optimize);
            assert_strings_balanced("c7_r1_options_text", &counters, no_optimize);
            assert_asan_clean("c7_r1_options_text", OPTIONS_TEXT, expected, no_optimize);
        });
    }
    // case: A string coerced into a union at a call argument is copied into an envelope the callee only reads (`share` or bare): the call's statement releases it. A `give` argument is the callee's, so the caller releases nothing (High: the envelope leaked one string per call; a caller release of a given envelope would be a double free).
    a_union_envelope_minted_for_a_call_argument_is_released_unless_given => {
        let expected = format!("{}\nYINZ\n", 20 * (2 + 2 + 2 + 2 + 2 + 2 + 1));
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced("c7_r1_union_args", UNION_ARGUMENTS, &expected, no_optimize);
            assert_strings_balanced("c7_r1_union_args", &counters, no_optimize);
            assert_asan_clean("c7_r1_union_args", UNION_ARGUMENTS, &expected, no_optimize);
        });
    }
}
