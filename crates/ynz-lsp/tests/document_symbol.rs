// WHY: workspace/symbol integration cases verify the full LSP round-trip:
// params → handler → serialised SymbolInformation response. Document symbols
// (a Low-tier outline) are not tested here.

mod harness;
use harness::InProcessHarness;
use serde_json::json;

macro_rules! workspace_symbol_case {
    ($name:ident, $query:expr, $expect_entrypoint:expr) => {
        // bug: workspace/symbol errors, returns null instead of an array, or misses a case-insensitive substring match on an open file | tier: Medium
        #[test]
        fn $name() {
            let client = InProcessHarness::new().start_server();
            client.initialize();
            client.did_open(
                "file:///ws_symbols.ynz",
                "function entrypoint() -> Nothing { }",
            );
            client.try_recv_timeout(std::time::Duration::from_millis(100));
            let result = client.request(10, "workspace/symbol", json!({ "query": $query }));
            let symbols = result
                .as_array()
                .unwrap_or_else(|| panic!("workspace/symbol must return an array; got {result}"));
            let found = symbols
                .iter()
                .any(|symbol| symbol["name"] == "entrypoint");
            assert_eq!(
                found, $expect_entrypoint,
                "query {:?} returned {symbols:?}",
                $query
            );
        }
    };
}

workspace_symbol_case!(empty_query_returns_every_open_symbol, "", true);
workspace_symbol_case!(
    non_matching_query_returns_an_empty_array,
    "zzznomatch",
    false
);
workspace_symbol_case!(query_matches_case_insensitively, "ENTRY", true);
