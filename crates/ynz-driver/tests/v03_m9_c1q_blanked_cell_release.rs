//! v0.3-M9 C1q Foundation 1: the release of a list skips a BLANKED (zeroed) cell, for every kind
//! of list item a loop row can be handed over from: string, array, map, a shape stored inline,
//! an inline union, and the cells of a `fixed` list (shape box and array).
//!
//! Tier: High (memory safety: C1q hands a loop row to another place and blanks the row's cell in
//! the list it came from; a release walk that frees a blank cell, or frees a handed-over row
//! twice, is a double free, and one that skips a live cell leaks it).
//!
//! Bug caught: a type's free step (`ynz_release_<type>`, `ynz_array_release`'s walk, the `fixed`
//! cell walk) that does not treat an all-zero cell as "owns nothing" would crash or free address
//! 0 once C1q blanks a cell.
//!
//! Each case builds one program at O0 (the list `f` is the only heap local), patches the emitted
//! IR so that, just before the list's scope-exit release, cell 0 is copied out and then blanked
//! (`ynz_array_blank_cell`, or a zero store into a `fixed` cell), and, just after the release,
//! the copied-out row is released by the item's own glue, as its new holder would. The patched
//! program runs under AddressSanitizer with the alloc counters on: it must exit 0, report no
//! sanitizer error, and free exactly what it allocated (cell 0's row is freed once, by the new
//! holder, and the blank cell is skipped by the list's release).

mod support;

use support::{build_asan_from_ir, build_source, run_asan_binary_counted};

const SHAPES: &str = "\
shape Tag {
  label: string
  nums: array<int>
}
shape Big {
  name: string
  weight: int
}
shape Tiny {
  flag: boolean
}
shape Thing = Big | Tiny
";

/// How the patched program reads the row out of cell 0.
enum Outer {
    /// `array<T>`: the heap list's cells; `ynz_array_get` copies the cell out.
    Array,
    /// `fixed<T>`: i64 cells in the frame; the cell is a pointer to the item.
    Fixed,
}

struct Case {
    label: &'static str,
    /// The body of `entrypoint`: must declare `f` and print something.
    body: &'static str,
    outer: Outer,
}

fn program(case: &Case) -> String {
    format!(
        "{SHAPES}\nfunction entrypoint() -> nothing {{\n{}\n}}\n",
        case.body
    )
}

/// The text between `define internal void @"<name>"(ptr %0) {` and its closing `}`.
fn glue_body<'ir>(ir: &'ir str, glue_name: &str) -> &'ir str {
    let header = format!("define internal void @\"{glue_name}\"(ptr %0) {{");
    let start = ir
        .find(&header)
        .unwrap_or_else(|| panic!("glue `{glue_name}` not defined in the IR"))
        + header.len();
    let end = ir[start..]
        .find("\n}\n")
        .unwrap_or_else(|| panic!("glue `{glue_name}` has no end"));
    &ir[start..start + end]
}

/// The item glue the list's release calls: `(callee token, items_inline)`.
fn item_glue(ir: &str, outer_glue: &str, outer: &Outer) -> (String, bool) {
    let body = glue_body(ir, outer_glue);
    match outer {
        Outer::Array => {
            let call = body
                .lines()
                .find(|line| line.contains("@ynz_array_release("))
                .unwrap_or_else(|| panic!("`{outer_glue}` does not call ynz_array_release"));
            let arguments = call.split("@ynz_array_release(").nth(1).expect("arguments");
            let parts: Vec<&str> = arguments.trim_end_matches(')').split(", ").collect();
            let glue = parts[1].strip_prefix("ptr ").expect("glue argument");
            assert_ne!(glue, "null", "{outer_glue}: the items own nothing");
            let inline = parts[2].trim() == "i32 1";
            (glue.to_string(), inline)
        }
        Outer::Fixed => {
            let call = body
                .lines()
                .find(|line| {
                    line.trim_start().starts_with("call void @") && line.contains("_stored")
                })
                .unwrap_or_else(|| panic!("`{outer_glue}` releases no cell"));
            let rest = call
                .trim_start()
                .strip_prefix("call void ")
                .expect("a call");
            let callee = match rest.strip_prefix("@\"") {
                Some(quoted) => format!("@\"{}\"", quoted.split('"').next().expect("name")),
                None => rest.split('(').next().expect("name").to_string(),
            };
            (callee, false)
        }
    }
}

/// Patch the O0 IR: copy cell 0 out and blank it before the release of `f`, release the copy
/// after.
fn patch(ir: &str, outer: &Outer) -> String {
    let release_line = ir
        .lines()
        .find(|line| line.contains("(ptr %f_release_value)") && line.contains("call void @"))
        .unwrap_or_else(|| panic!("no release of `f` in the IR:\n{ir}"));
    let outer_glue = release_line
        .split("@\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("outer glue name");
    let (glue, inline) = item_glue(ir, outer_glue, outer);
    let before = match outer {
        Outer::Array => {
            "  %probe_saved = alloca [128 x i8], align 8\n  \
             %probe_has = call i64 @ynz_array_get(ptr %f_release_value, i64 0, ptr %probe_saved)\n  \
             call void @ynz_array_blank_cell(ptr %f_release_value, i64 0)\n"
        }
        Outer::Fixed => {
            "  %probe_item = load ptr, ptr %f_release_value, align 8\n  \
             store i64 0, ptr %f_release_value, align 8\n"
        }
    };
    let after = match (outer, inline) {
        (Outer::Array, true) => format!("  call void {glue}(ptr %probe_saved)\n"),
        (Outer::Array, false) => format!(
            "  %probe_item = load ptr, ptr %probe_saved, align 8\n  call void {glue}(ptr %probe_item)\n"
        ),
        (Outer::Fixed, _) => format!("  call void {glue}(ptr %probe_item)\n"),
    };
    let mut patched = String::new();
    for line in ir.lines() {
        if line == release_line {
            patched.push_str(before);
            patched.push_str(line);
            patched.push('\n');
            patched.push_str(&after);
        } else {
            patched.push_str(line);
            patched.push('\n');
        }
    }
    let mut declarations = String::new();
    if !patched.contains("declare i64 @ynz_array_get(") {
        declarations.push_str("declare i64 @ynz_array_get(ptr, i64, ptr)\n");
    }
    if matches!(outer, Outer::Array) && !patched.contains("declare void @ynz_array_blank_cell(") {
        declarations.push_str("declare void @ynz_array_blank_cell(ptr, i64)\n");
    }
    // Declarations go before the first existing `declare`, after the module header lines.
    let at = patched
        .find("declare ")
        .expect("the IR declares the runtime");
    patched.insert_str(at, &declarations);
    patched
}

fn assert_blank_cell_is_skipped(case: &Case) {
    let built = build_source(&program(case), case.label, true);
    let ir = std::fs::read_to_string(&built.ir).expect("read IR");
    let patched = patch(&ir, &case.outer);
    let directory = built.ir.parent().expect("IR directory").to_path_buf();
    let binary = build_asan_from_ir(&directory, &patched);
    let (output, counters) = run_asan_binary_counted(&binary);
    assert!(
        !output.stderr.contains("AddressSanitizer"),
        "{}: sanitizer error after blanking cell 0:\n{}",
        case.label,
        output.stderr
    );
    assert_eq!(
        output.exit_code, 0,
        "{}: stderr:\n{}",
        case.label, output.stderr
    );
    assert_eq!(
        (counters.alloc, counters.string_alloc),
        (counters.free, counters.string_free),
        "{}: frees must equal allocs: {counters:?}",
        case.label
    );
    assert!(counters.alloc > 0, "{}: allocated nothing", case.label);
}

macro_rules! blank_case {
    ($name:ident, $outer:expr, $body:literal, $bug:literal) => {
        #[doc = $bug]
        #[test]
        fn $name() {
            assert_blank_cell_is_skipped(&Case {
                label: stringify!($name),
                body: $body,
                outer: $outer,
            });
        }
    };
}

blank_case!(
    a_blanked_string_cell_is_skipped,
    Outer::Array,
    "  let f: array<string> = [`a`.toUpperCase(), `b`.toUpperCase()]\n  print(f.count())",
    "a string list release that frees a null string cell (High)"
);
blank_case!(
    a_blanked_array_cell_is_skipped,
    Outer::Array,
    "  let f: array<array<int>> = [[1, 2], [3, 4]]\n  print(f.count())",
    "an array-of-arrays release that frees a null row (High)"
);
blank_case!(
    a_blanked_map_cell_is_skipped,
    Outer::Array,
    "  let f: array<map<string, int>> = [{ `a`: 1 }, { `b`: 2 }]\n  print(f.count())",
    "an array-of-maps release that frees a null map (High)"
);
blank_case!(
    a_blanked_shape_inline_cell_is_skipped,
    Outer::Array,
    "  let f: array<Tag> = [{ label: `a`.toUpperCase(), nums: [1] }, { label: `b`.toUpperCase(), nums: [2] }]\n  print(f.count())",
    "an array-of-shapes release that frees the heap fields of an all-zero inline shape cell (High)"
);
blank_case!(
    a_blanked_union_inline_cell_is_skipped,
    Outer::Array,
    "  let f: array<Thing> = []\n  let first: Big = { name: `n`.toUpperCase(), weight: 1 }\n  f.add(first)\n  let second: Big = { name: `m`.toUpperCase(), weight: 2 }\n  f.add(second)\n  print(f.count())",
    "an array-of-unions release that frees the payload of an all-zero slot (tag 0 is the heap variant `Big`) (High)"
);
blank_case!(
    a_blanked_fixed_array_cell_is_skipped,
    Outer::Fixed,
    "  let f: fixed<array<int>> = [[1, 2], [3, 4]]\n  print(f.count())",
    "a fixed-list release that frees a zeroed array cell (High)"
);
blank_case!(
    a_blanked_fixed_shape_cell_is_skipped,
    Outer::Fixed,
    "  let f: fixed<Tag> = [{ label: `a`.toUpperCase(), nums: [1] }, { label: `b`.toUpperCase(), nums: [2] }]\n  print(f.count())",
    "a fixed-list release that frees a zeroed shape-box cell (High)"
);
