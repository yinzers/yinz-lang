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
  count: int
}
shape Outer {
  inner: R
  name: string
}
function entrypoint() -> nothing {
  let r: R = { count: 1 }
  print(r)
  let inner: R = { count: 2 }
  let o: Outer = { inner: inner, name: `box` }
  print(o)
  let rows: array<R> = [{ count: 3 }, { count: 4 }]
  print(rows)
  let i: int = 0
  while (i < 10) {
    print(r)
    i = i + 1
  }
}
";

const PRINTED_CONTAINERS: &str = "
shape R {
  count: int
  tags: array<string>
}
function entrypoint() -> nothing {
  let r: R = { count: 1, tags: [`a`, `b`] }
  print(r)
  let rows: array<array<int>> = [[1], [2, 3]]
  print(rows)
  let words: array<string> = [`p`, `q`]
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

/// The debug text a non-string `print` argument builds (a shape's, an array's, and the nested
/// text of a shape field or an array item) is a heap string nobody released (High).
#[test]
fn the_debug_text_a_print_builds_is_released() {
    let expected = printed_shapes_expected();
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_r1_debug_text", PRINTED_SHAPES, &expected, no_optimize);
        assert_heap_balanced("c7_r1_debug_text", &counters, no_optimize);
        assert_strings_balanced("c7_r1_debug_text", &counters, no_optimize);
        assert_asan_clean("c7_r1_debug_text", PRINTED_SHAPES, &expected, no_optimize);
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

const STRING_LOOP: &str = "
function entrypoint() -> nothing {
  let total: int = 0
  for (c in `hello`) {
    total = total + c.count()
  }
  let word: string = `wor`.toUpperCase()
  for (letter in word) {
    if (letter == `O`) {
      total = total + 10
    }
  }
  print(total)
}
";

/// The per-character string of a string `for` loop variable is never released (High).
#[test]
fn a_string_loops_per_character_string_is_released() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_r1_for_string", STRING_LOOP, "15\n", no_optimize);
        assert_strings_balanced("c7_r1_for_string", &counters, no_optimize);
        assert_asan_clean("c7_r1_for_string", STRING_LOOP, "15\n", no_optimize);
    });
}

const BIGNUM_LOCAL: &str = "
function entrypoint() -> nothing {
  let a: number<100> = 1.5
  let b: number<100> = 2.25
  let c: number<100> = a + b
  print(c)
}
";

const BIGNUM_TEMPORARY: &str = "
function entrypoint() -> nothing {
  let a: number<100> = 1.5
  let b: number<100> = 2.25
  print(a + b)
}
";

const BIGNUM_LOOP: &str = "
function entrypoint() -> nothing {
  let a: number<100> = 1.5
  let b: number<100> = 2.25
  let i: int = 0
  while (i < 100) {
    print(a + b)
    let c: number<100> = a + b
    print(c)
    i = i + 1
  }
}
";

/// A `number` wider than 34 digits is heap text; a local of that type and the temporary a
/// `print(a + b)` builds were never released (High).
#[test]
fn a_bignum_local_and_a_bignum_temporary_are_released() {
    let loop_expected = "3.75\n".repeat(200);
    at_both_optimization_levels(|no_optimize| {
        for (label, program, expected) in [
            ("c7_r1_bignum_local", BIGNUM_LOCAL, "3.75\n"),
            ("c7_r1_bignum_temporary", BIGNUM_TEMPORARY, "3.75\n"),
            ("c7_r1_bignum_loop", BIGNUM_LOOP, loop_expected.as_str()),
        ] {
            let counters = run_balanced(label, program, expected, no_optimize);
            assert_strings_balanced(label, &counters, no_optimize);
            assert_asan_clean(label, program, expected, no_optimize);
        }
    });
}

const STRING_LOOP_KEEPS: &str = "
function firstVowel(word: string) -> string {
  for (letter in word) {
    if (letter == `e` || letter == `o`) {
      return letter
    }
  }
  return `none`
}

function entrypoint() -> nothing {
  let kept: array<string> = []
  let last: string = `-`
  for (c in `abc`) {
    let same: string = c
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

/// A string loop's variable stored, assigned, bound or returned inside the body leaves the
/// per-character release exactly one owner each: no leak and no double free (High).
#[test]
fn a_string_loop_variable_that_is_kept_is_copied_not_released_twice() {
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

const BIGNUM_HOLDERS: &str = "
shape Money {
  amount: number<100>
  label: string
}
function twice(share x: number<100>) -> number<100> {
  let y: number<100> = x + x
  return y
}
function pass(x: number<100>) -> number<100> {
  return x
}
function entrypoint() -> nothing {
  let a: number<100> = 1.5
  let b: number<100> = 2.25
  let c: number<100> = a + b
  let d: number<100> = c
  print(d)
  c = c + a
  print(c)
  print(twice(c))
  let e: number<100> = pass(d)
  print(e)
  let m: Money = { amount: c, label: `x` }
  print(m)
  if (a < b) {
    let f: number<100> = a * b
    print(f)
  }
}
";

/// A bignum is owned text, so every second holder copies it (a `let`, a field, a returned
/// parameter) and the release frees each copy once: no leak and no double free (High).
#[test]
fn a_bignum_copied_into_a_let_a_field_and_a_return_is_released_once_each() {
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

const ERRORS_NUMBER: &str = "
function price(n: int) -> number errors {
  let total: number = 1.5
  return total
}

function priceTwo(n: int) -> number errors {
  return 2.5
}

function entrypoint() -> nothing {
  let i: int = 0
  let sum: number = 0.0
  while (i < 50) {
    let a = price(i)
    let b = priceTwo(i)
    sum = sum + a.or(0.0) + b.or(0.0)
    i = i + 1
  }
  print(sum)
}
";

/// The payload of an `errors` result of type `number` is a heap cell the `return` made; the
/// result's release left it (High).
#[test]
fn an_errors_result_of_number_releases_its_payload_cell() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_r1_ec_number", ERRORS_NUMBER, "200.0\n", no_optimize);
        assert_heap_balanced("c7_r1_ec_number", &counters, no_optimize);
        assert_asan_clean("c7_r1_ec_number", ERRORS_NUMBER, "200.0\n", no_optimize);
    });
}

const ERRORS_SHAPE: &str = "
shape Tiny {
  flag: boolean
}
shape Big {
  label: string
  count: int
}
shape Thing = Tiny | Big

function makeBig(count: int) -> Big errors {
  let big: Big = { label: `made`, count: count }
  return big
}

function makeThing(count: int) -> Thing errors {
  let big: Big = { label: `made`, count: count }
  return big
}

function describe(share thing: Thing) -> int {
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

function entrypoint() -> nothing {
  let fallback: Big = { label: `none`, count: 0 }
  let seed: Big = { label: `seed`, count: 0 }
  let fallbackThing: Thing = seed
  let i: int = 0
  let total: int = 0
  while (i < 50) {
    let one = makeBig(i)
    let two = makeThing(i)
    let got: Big = one.or(fallback)
    total = total + got.count + describe(two.or(fallbackThing))
    i = i + 1
  }
  print(total)
}
";

/// A shape local returned through an `errors` result moves into the result: the `return` neither
/// copies it a second time nor leaves the local's strings behind (High).
#[test]
fn a_shape_returned_through_an_errors_result_is_moved_not_copied_twice() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_r1_ec_shape", ERRORS_SHAPE, "2450\n", no_optimize);
        assert_heap_balanced("c7_r1_ec_shape", &counters, no_optimize);
        assert_strings_balanced("c7_r1_ec_shape", &counters, no_optimize);
        assert_asan_clean("c7_r1_ec_shape", ERRORS_SHAPE, "2450\n", no_optimize);
    });
}

const ERRORS_MAYBE: &str = "
function pick(n: int) -> maybe<int> errors {
  let items: array<int> = [4, 5]
  let got = items.get(n)
  return got
}

function pickWord(n: int) -> maybe<string> errors {
  let words: array<string> = [`a`, `b`]
  let got = words.get(n)
  return got
}

function entrypoint() -> nothing {
  let i: int = 0
  let total: int = 0
  let none_int: maybe<int> = none
  while (i < 30) {
    let one = pick(i % 3)
    let word = pickWord(1)
    total = total + one.or(none_int).or(0) + 1
    i = i + 1
  }
  print(total)
}
";

/// The envelope cell an `errors` result of a `maybe` carries is owned by the caller's binding, so
/// its release frees it (High: it was `NotOwner`, so every call leaked a cell).
#[test]
fn an_errors_result_of_maybe_releases_its_envelope_cell() {
    let expected = "120\n";
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("c7_r1_ec_maybe", ERRORS_MAYBE, expected, no_optimize);
        assert_heap_balanced("c7_r1_ec_maybe", &counters, no_optimize);
        assert_asan_clean("c7_r1_ec_maybe", ERRORS_MAYBE, expected, no_optimize);
    });
}
