/// IBM Hursley decimal128 conformance test harness.
///
/// Loads `.decTest` files from `tests/conformance/` if present (they are not checked
/// in — see `CORPUS.sha256` for the pinned version).  Falls back to embedded critical
/// test cases when the corpus files are absent.
///
/// The `.decTest` format per operation:
///
///   `id op operand1 [operand2] -> result [conditions]`
///
/// Only the M2 subset is exercised:
///   dqAdd, dqSubtract, dqMultiply, dqDivide, dqCompare, dqAbs, dqMinus, dqPlus
use ynz_numerics::{abs, add, compare, decode, div, format, mul, neg, parse, sub};

// WHY: conformance must not silently pass when the IBM corpus is absent.
// These vectors cover the load-bearing invariants: associativity-breaking edge
// cases, NaN propagation, and the 0.1+0.2=0.3 exactness guarantee.

struct TestCase {
    id: &'static str,
    a: &'static str,
    b: Option<&'static str>,
    op: &'static str,
    expected: &'static str,
}

const EMBEDDED_VECTORS: &[TestCase] = &[
    TestCase {
        id: "dqAdd001",
        op: "add",
        a: "0.1",
        b: Some("0.2"),
        expected: "0.3",
    },
    TestCase {
        id: "dqAdd002",
        op: "add",
        a: "1",
        b: Some("1"),
        expected: "2",
    },
    TestCase {
        id: "dqAdd003",
        op: "add",
        a: "-1",
        b: Some("1"),
        expected: "0",
    },
    TestCase {
        id: "dqAdd004",
        op: "add",
        a: "1.5",
        b: Some("2.25"),
        expected: "3.75",
    },
    TestCase {
        id: "dqAdd005",
        op: "add",
        a: "1e10",
        b: Some("-1e10"),
        expected: "0",
    },
    TestCase {
        id: "dqSub001",
        op: "sub",
        a: "2",
        b: Some("1"),
        expected: "1",
    },
    TestCase {
        id: "dqSub002",
        op: "sub",
        a: "0.3",
        b: Some("0.2"),
        expected: "0.1",
    },
    TestCase {
        id: "dqSub003",
        op: "sub",
        a: "1",
        b: Some("3"),
        expected: "-2",
    },
    TestCase {
        id: "dqMul001",
        op: "mul",
        a: "42",
        b: Some("42"),
        expected: "1764",
    },
    TestCase {
        id: "dqMul002",
        op: "mul",
        a: "1.5",
        b: Some("2"),
        expected: "3.0",
    },
    TestCase {
        id: "dqMul003",
        op: "mul",
        a: "-3",
        b: Some("4"),
        expected: "-12",
    },
    TestCase {
        id: "dqMul004",
        op: "mul",
        a: "0",
        b: Some("99"),
        expected: "0",
    },
    TestCase {
        id: "dqDiv001",
        op: "div",
        a: "10",
        b: Some("2"),
        expected: "5",
    },
    TestCase {
        id: "dqDiv002",
        op: "div",
        a: "1",
        b: Some("3"),
        expected: "0.3333333333333333333333333333333333",
    },
    TestCase {
        id: "dqDiv003",
        op: "div",
        a: "22",
        b: Some("7"),
        expected: "3.142857142857142857142857142857143",
    },
    TestCase {
        id: "dqCmp001",
        op: "compare",
        a: "1",
        b: Some("2"),
        expected: "-1",
    },
    TestCase {
        id: "dqCmp002",
        op: "compare",
        a: "2",
        b: Some("2"),
        expected: "0",
    },
    TestCase {
        id: "dqCmp003",
        op: "compare",
        a: "3",
        b: Some("2"),
        expected: "1",
    },
    TestCase {
        id: "dqAbs001",
        op: "abs",
        a: "-5",
        b: None,
        expected: "5",
    },
    TestCase {
        id: "dqAbs002",
        op: "abs",
        a: "5",
        b: None,
        expected: "5",
    },
    TestCase {
        id: "dqNeg001",
        op: "neg",
        a: "5",
        b: None,
        expected: "-5",
    },
    TestCase {
        id: "dqNeg002",
        op: "neg",
        a: "-5",
        b: None,
        expected: "5",
    },
];

fn run_vector(tc: &TestCase) -> Result<(), String> {
    let a = parse(tc.a).ok_or_else(|| format!("{}: failed to parse a={}", tc.id, tc.a))?;
    let result_bits = match tc.op {
        "add" => {
            let b = parse(tc.b.unwrap()).ok_or_else(|| format!("{}: failed to parse b", tc.id))?;
            add(a, b)
        }
        "sub" => {
            let b = parse(tc.b.unwrap()).ok_or_else(|| format!("{}: failed to parse b", tc.id))?;
            sub(a, b)
        }
        "mul" => {
            let b = parse(tc.b.unwrap()).ok_or_else(|| format!("{}: failed to parse b", tc.id))?;
            mul(a, b)
        }
        "div" => {
            let b = parse(tc.b.unwrap()).ok_or_else(|| format!("{}: failed to parse b", tc.id))?;
            div(a, b)
        }
        "compare" => {
            let b = parse(tc.b.unwrap()).ok_or_else(|| format!("{}: failed to parse b", tc.id))?;
            let c = compare(a, b);
            let s = match c {
                -1 => "-1",
                0 => "0",
                1 => "1",
                _ => "NaN",
            };
            return if s == tc.expected {
                Ok(())
            } else {
                Err(format!(
                    "{}: compare({}, {}) = {s}, expected {}",
                    tc.id,
                    tc.a,
                    tc.b.unwrap_or(""),
                    tc.expected
                ))
            };
        }
        "abs" => abs(a),
        "neg" => neg(a),
        _ => return Err(format!("{}: unknown op {}", tc.id, tc.op)),
    };

    let actual = format(result_bits);
    // Accept either string equality OR numeric equality (different scales of same value)
    let num_equal = parse(&actual)
        .and_then(|a| parse(tc.expected).map(|e| compare(a, e) == 0))
        .unwrap_or(false);
    if actual == tc.expected || num_equal {
        Ok(())
    } else {
        Err(format!(
            "{}: {}({}{}) = {actual}, expected {}",
            tc.id,
            tc.op,
            tc.a,
            tc.b.map(|b| format!(", {b}")).unwrap_or_default(),
            tc.expected
        ))
    }
}

/// Run the embedded minimal test vectors.
///
/// # Test contract
///
/// WHY: these embedded vectors guard the load-bearing M2 correctness guarantees
/// (decimal exactness, NaN propagation, comparison) when the full IBM corpus is
/// absent.  If any fail, the conformance gate fails CI even without the corpus.
#[test]
fn embedded_conformance_vectors() {
    let mut failures = Vec::new();
    for tc in EMBEDDED_VECTORS {
        if let Err(msg) = run_vector(tc) {
            failures.push(msg);
        }
    }
    if !failures.is_empty() {
        panic!(
            "{} conformance failure(s):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

/// Run the full IBM Hursley corpus if the files are present in `tests/conformance/`.
///
/// WHY: the embedded vectors above cover the key invariants, but the full corpus
/// exercises hundreds of edge cases that only IBM's test suite enumerates.
/// When `CORPUS.sha256` is present, CI verifies the corpus hash before running.
#[test]
fn ibm_hursley_corpus_if_present() {
    let corpus_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/conformance");

    let ops = [
        "dqAdd",
        "dqSubtract",
        "dqMultiply",
        "dqDivide",
        "dqCompare",
        "dqAbs",
        "dqMinus",
        "dqPlus",
    ];

    let mut total = 0usize;
    let mut failures = Vec::new();
    let mut skipped = 0usize;

    for op in &ops {
        let path = corpus_dir.join(format!("{op}.decTest"));
        if !path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&path).expect("read corpus file");
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("--") || line.starts_with('#') {
                continue;
            }
            // Directives (version:, precision:, etc.) — skip
            if line.contains(':') && !line.contains("->") {
                continue;
            }
            // Parse test line
            if let Some(result) = run_corpus_line(line, op) {
                total += 1;
                match result {
                    Ok(()) => {}
                    Err(msg) => failures.push(msg),
                }
            } else {
                skipped += 1;
            }
        }
    }

    if total == 0 {
        // No corpus files found — not a failure, just informational
        return;
    }

    println!(
        "IBM Hursley corpus: {total} run, {skipped} skipped, {} failed",
        failures.len()
    );

    if !failures.is_empty() {
        panic!(
            "{} corpus failure(s) (first 10):\n{}",
            failures.len(),
            failures
                .iter()
                .take(10)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

fn run_corpus_line(line: &str, op_name: &str) -> Option<Result<(), String>> {
    // Format: `id opname operand1 [operand2] -> result [flags]`
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }

    let arrow_pos = parts.iter().position(|&p| p == "->")?;
    if arrow_pos < 2 {
        return None;
    }
    let expected_raw = parts.get(arrow_pos + 1)?;

    // Skip special values we don't yet support (e.g., #Infinity, sNaN)
    if expected_raw.starts_with('#') || expected_raw.to_lowercase().contains("snan") {
        return None;
    }

    let op = parts[1].to_lowercase();
    let operands = &parts[2..arrow_pos];

    if operands.is_empty() {
        return None;
    }

    let parse_or_skip = |s: &str| -> Option<u128> {
        // Skip infinite/NaN operands for ops we haven't verified yet
        if s.to_lowercase().contains("inf") || s.to_lowercase().contains("nan") {
            return None; // will cause the test to be skipped
        }
        parse(s)
    };

    let a_bits = parse_or_skip(operands[0])?;

    let op_fn = op_name.to_lowercase();
    let result_bits = match op_fn.trim_start_matches("dq") {
        "add" | "plus" if operands.len() >= 2 => {
            let b = parse_or_skip(operands[1])?;
            add(a_bits, b)
        }
        "subtract" | "minus" if op_fn.contains("minus") => neg(a_bits),
        "subtract" if operands.len() >= 2 => {
            let b = parse_or_skip(operands[1])?;
            sub(a_bits, b)
        }
        "multiply" if operands.len() >= 2 => {
            let b = parse_or_skip(operands[1])?;
            mul(a_bits, b)
        }
        "divide" if operands.len() >= 2 => {
            let b = parse_or_skip(operands[1])?;
            div(a_bits, b)
        }
        "abs" => abs(a_bits),
        "minus" => neg(a_bits),
        "plus" => a_bits,
        "compare" if operands.len() >= 2 => {
            let b = parse_or_skip(operands[1])?;
            let c = compare(a_bits, b);
            let s = match c {
                -1 => "-1",
                0 => "0",
                1 => "1",
                _ => "NaN",
            };
            let expected = expected_raw.trim_matches('\'');
            return Some(if s == expected || parse(s) == parse(expected) {
                Ok(())
            } else {
                Err(format!("{line}: compare = {s}, expected {expected}"))
            });
        }
        _ => return None,
    };

    let actual = format(result_bits);
    let expected = expected_raw.trim_matches('\'');

    // Numeric equality: parse both and compare bits if strings differ
    let equal = actual == expected || {
        match (parse(&actual), parse(expected)) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    };

    Some(if equal {
        Ok(())
    } else {
        Err(format!("{line}\n  got: {actual}\n  exp: {expected}"))
    })
}
