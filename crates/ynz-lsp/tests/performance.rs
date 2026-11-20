// WHY: Performance regression tests guard the LSP's responsiveness SLAs.
// All tests are #[ignore] — run via:
//   cargo test --release -p ynz-lsp -- --ignored
// These tests measure wall-clock time from the test's perspective (includes
// thread scheduling overhead), so thresholds include generous headroom over
// the raw algorithmic cost.

mod harness;
use harness::InProcessHarness;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Shared fixture content
// ---------------------------------------------------------------------------

const HUNDRED_LINE_FIXTURE: &str = "\
function entrypoint() -> Nothing {\n\
    let a: Int = 1\n\
    let b: Int = 2\n\
    let c: Int = a + b\n\
    let d: Int = c * 3\n\
    let e: Int = d - 1\n\
    let f: Int = e + 10\n\
    let g: Int = f * 2\n\
    let h: Int = g + 5\n\
    let i: Int = h - 3\n\
    let j: Int = i + 7\n\
    let k: Int = j * 4\n\
    let l: Int = k - 2\n\
    let m: Int = l + 9\n\
    let n: Int = m * 1\n\
    let o: Int = n + 0\n\
    let p: Int = o - 4\n\
    let q: Int = p + 6\n\
    let r: Int = q * 3\n\
    let s: Int = r + 1\n\
    let t: Int = s - 5\n\
    let u: Int = t + 8\n\
    let v: Int = u * 2\n\
    let w: Int = v + 3\n\
    let x: Int = w - 1\n\
    let y: Int = x + 4\n\
    let z: Int = y * 5\n\
    print(z)\n\
    let aa: Int = z + 1\n\
    let ab: Int = aa + 2\n\
    let ac: Int = ab + 3\n\
    let ad: Int = ac + 4\n\
    let ae: Int = ad + 5\n\
    let af: Int = ae + 6\n\
    let ag: Int = af + 7\n\
    let ah: Int = ag + 8\n\
    let ai: Int = ah + 9\n\
    let aj: Int = ai + 10\n\
    let ak: Int = aj + 11\n\
    let al: Int = ak + 12\n\
    let am: Int = al + 13\n\
    let an: Int = am + 14\n\
    let ao: Int = an + 15\n\
    let ap: Int = ao + 16\n\
    let aq: Int = ap + 17\n\
    let ar: Int = aq + 18\n\
    let as2: Int = ar + 19\n\
    let at: Int = as2 + 20\n\
    let au: Int = at + 21\n\
    let av: Int = au + 22\n\
    let aw: Int = av + 23\n\
    let ax: Int = aw + 24\n\
    let ay: Int = ax + 25\n\
    let az: Int = ay + 26\n\
    let ba: Int = az + 27\n\
    let bb: Int = ba + 28\n\
    let bc: Int = bb + 29\n\
    let bd: Int = bc + 30\n\
    let be: Int = bd + 31\n\
    let bf: Int = be + 32\n\
    let bg: Int = bf + 33\n\
    let bh: Int = bg + 34\n\
    let bi: Int = bh + 35\n\
    let bj: Int = bi + 36\n\
    let bk: Int = bj + 37\n\
    let bl: Int = bk + 38\n\
    let bm: Int = bl + 39\n\
    let bn: Int = bm + 40\n\
    let bo: Int = bn + 41\n\
    let bp: Int = bo + 42\n\
    let bq: Int = bp + 43\n\
    let br: Int = bq + 44\n\
    let bs: Int = br + 45\n\
    print(bs)\n\
}\n";

// ---------------------------------------------------------------------------
// Cold initialize + first diagnostics
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn perf_cold_initialize_under_500ms() {
    // WHY: From cold server start to first diagnostic notification must complete
    // in under 500ms. If it exceeds this threshold, the LSP feels sluggish on
    // every file open. 500ms is the upper bound for "responsive on file open"
    // per standard editor UX guidance.
    let start = Instant::now();

    let client = InProcessHarness::new().start_server();
    client.initialize();
    client.did_open("file:///perf_cold.ynz", HUNDRED_LINE_FIXTURE);

    // Block until the publishDiagnostics notification arrives.
    let msg = client.recv_response();
    let elapsed = start.elapsed();

    assert!(
        !msg.is_null(),
        "publishDiagnostics notification must arrive"
    );
    assert!(
        elapsed < Duration::from_millis(500),
        "cold initialize + first diagnostics took {:?}; must be < 500ms",
        elapsed
    );
}

// ---------------------------------------------------------------------------
// Incremental change latency
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn perf_incremental_change_under_100ms() {
    // WHY: After a single-character edit, the next publishDiagnostics must arrive
    // within 100ms. This is the latency budget for "as-you-type" diagnostics.
    // Salsa's incremental computation makes this achievable; if it regresses,
    // the IDE feels broken on every keystroke.
    let client = InProcessHarness::new().start_server();
    client.initialize();
    client.did_open("file:///perf_incr.ynz", HUNDRED_LINE_FIXTURE);
    // Drain the initial diagnostic.
    client.recv_response();

    let text_v2 = format!("{HUNDRED_LINE_FIXTURE}// single char added\n");

    let start = Instant::now();
    client.did_change("file:///perf_incr.ynz", &text_v2, 2);
    let msg = client.recv_response();
    let elapsed = start.elapsed();

    assert!(
        !msg.is_null(),
        "publishDiagnostics notification must arrive after incremental change"
    );
    assert!(
        elapsed < Duration::from_millis(100),
        "incremental change diagnostic latency {:?} must be < 100ms",
        elapsed
    );
}

// ---------------------------------------------------------------------------
// Completion latency
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn perf_completion_under_50ms() {
    // WHY: Completion requests must respond within 50ms on a warmed server.
    // This is the upper bound for "instant" autocomplete feedback per standard
    // editor UX guidance. Salsa memoization should make repeated completions
    // on unchanged files effectively free after the first compile.
    let client = InProcessHarness::new().start_server();
    client.initialize();
    client.did_open("file:///perf_comp.ynz", HUNDRED_LINE_FIXTURE);
    client.recv_response(); // drain diagnostic — server is now warm

    let start = Instant::now();
    client
        .conn
        .sender
        .send(lsp_server::Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(10i32),
            method: "textDocument/completion".to_string(),
            params: serde_json::json!({
                "textDocument": { "uri": "file:///perf_comp.ynz" },
                "position": { "line": 0, "character": 0 }
            }),
        }))
        .expect("completion send must succeed");
    let response = client.recv_response();
    let elapsed = start.elapsed();

    assert!(
        response.get("error").is_none(),
        "completion must not return an error: {response:?}"
    );
    assert!(
        elapsed < Duration::from_millis(50),
        "completion latency {:?} must be < 50ms on warmed server",
        elapsed
    );
}

// ---------------------------------------------------------------------------
// Hover latency
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn perf_hover_under_50ms() {
    // WHY: Hover requests must respond within 50ms on a warmed server.
    // Same reasoning as completion — registry lookups and token-at-offset are
    // O(1) operations; 50ms is generous headroom for thread scheduling overhead.
    let client = InProcessHarness::new().start_server();
    client.initialize();
    client.did_open("file:///perf_hover.ynz", HUNDRED_LINE_FIXTURE);
    client.recv_response(); // drain diagnostic — server is now warm

    let start = Instant::now();
    client
        .conn
        .sender
        .send(lsp_server::Message::Request(lsp_server::Request {
            id: lsp_server::RequestId::from(20i32),
            method: "textDocument/hover".to_string(),
            params: serde_json::json!({
                "textDocument": { "uri": "file:///perf_hover.ynz" },
                "position": { "line": 0, "character": 0 }
            }),
        }))
        .expect("hover send must succeed");
    let response = client.recv_response();
    let elapsed = start.elapsed();

    assert!(
        response.get("error").is_none(),
        "hover must not return an error: {response:?}"
    );
    assert!(
        elapsed < Duration::from_millis(50),
        "hover latency {:?} must be < 50ms on warmed server",
        elapsed
    );
}
