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
function entrypoint() -> nothing {\n\
    let a: int = 1\n\
    let b: int = 2\n\
    let c: int = a + b\n\
    let d: int = c * 3\n\
    let e: int = d - 1\n\
    let f: int = e + 10\n\
    let g: int = f * 2\n\
    let h: int = g + 5\n\
    let i: int = h - 3\n\
    let j: int = i + 7\n\
    let k: int = j * 4\n\
    let l: int = k - 2\n\
    let m: int = l + 9\n\
    let n: int = m * 1\n\
    let o: int = n + 0\n\
    let p: int = o - 4\n\
    let q: int = p + 6\n\
    let r: int = q * 3\n\
    let s: int = r + 1\n\
    let t: int = s - 5\n\
    let u: int = t + 8\n\
    let v: int = u * 2\n\
    let w: int = v + 3\n\
    let x: int = w - 1\n\
    let y: int = x + 4\n\
    let z: int = y * 5\n\
    print(z)\n\
    let aa: int = z + 1\n\
    let ab: int = aa + 2\n\
    let ac: int = ab + 3\n\
    let ad: int = ac + 4\n\
    let ae: int = ad + 5\n\
    let af: int = ae + 6\n\
    let ag: int = af + 7\n\
    let ah: int = ag + 8\n\
    let ai: int = ah + 9\n\
    let aj: int = ai + 10\n\
    let ak: int = aj + 11\n\
    let al: int = ak + 12\n\
    let am: int = al + 13\n\
    let an: int = am + 14\n\
    let ao: int = an + 15\n\
    let ap: int = ao + 16\n\
    let aq: int = ap + 17\n\
    let ar: int = aq + 18\n\
    let as2: int = ar + 19\n\
    let at: int = as2 + 20\n\
    let au: int = at + 21\n\
    let av: int = au + 22\n\
    let aw: int = av + 23\n\
    let ax: int = aw + 24\n\
    let ay: int = ax + 25\n\
    let az: int = ay + 26\n\
    let ba: int = az + 27\n\
    let bb: int = ba + 28\n\
    let bc: int = bb + 29\n\
    let bd: int = bc + 30\n\
    let be: int = bd + 31\n\
    let bf: int = be + 32\n\
    let bg: int = bf + 33\n\
    let bh: int = bg + 34\n\
    let bi: int = bh + 35\n\
    let bj: int = bi + 36\n\
    let bk: int = bj + 37\n\
    let bl: int = bk + 38\n\
    let bm: int = bl + 39\n\
    let bn: int = bm + 40\n\
    let bo: int = bn + 41\n\
    let bp: int = bo + 42\n\
    let bq: int = bp + 43\n\
    let br: int = bq + 44\n\
    let bs: int = br + 45\n\
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
