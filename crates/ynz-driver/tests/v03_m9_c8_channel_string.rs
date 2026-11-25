//! v0.3-M9 C8 (card #385, round 2): the strings a `channel<string>` still holds when its last
//! reference goes are released with the channel. A C6 defect found after close: C3 made strings
//! owned (counted, released), but the channel's element classification kept `string` glue-less
//! ("immortal bytes"), so a string sent and never received leaked at every teardown.
//!
//! Each program is balanced (heap and strings) and AddressSanitizer clean at both optimization
//! levels, each in its own process.

mod support;

use support::assert_source_balanced_and_asan_clean as assert_balanced;

/// One named test per way a string meets a `channel<string>`'s teardown.
macro_rules! channel_string_high_case {
    ($($case_name:ident, $label:literal, $source:literal, $expected_stdout:literal;)+) => { $(
        // bug: a string buffered in, parked on or given through a channel<string> leaks or is freed twice at teardown, silently at exit 0 | tier: High
        #[test]
        fn $case_name() {
            assert_balanced($label, $source, $expected_stdout);
        }
    )+ };
}

channel_string_high_case! {
    // case: a buffered string and a parked (pending) send's string leak with the channel; a capacity-1 channel buffers the first string and parks the second send, the third never runs
    strings_buffered_and_parked_in_a_channel_are_released_with_it, "channel_string_unreceived", "
function producer(wire: Channel<String>) -> Nothing {
  wire.send(`a`.toUpperCase())
  wire.send(`b`.toUpperCase())
  wire.send(`c`.toUpperCase())
  print(`sent`)
}

function entrypoint() -> Nothing {
  let wire: Channel<String> = Channel<String>(1)
  background producer(wire)
  wait sleep(50)
  print(`parent`)
}
", "parent\n";
    // case: giving the channel a glue for strings must not make a task's own string a double owner; a string a task was given and sends on leaves the task's ladder (`release_ladder_payload`), so the receiver reads it intact after the task retired and frees it once
    a_given_string_a_task_sends_is_received_intact_and_freed_once, "channel_string_given_then_received", "
function producer(wire: Channel<String>, give text: String) -> Nothing {
  wire.send(text)
}

function entrypoint() -> Nothing {
  let wire: Channel<String> = Channel<String>(2)
  let text: String = `hello`.toUpperCase()
  background producer(wire, text)
  wait sleep(50)
  let delivered = wire.receive()
  print(delivered.or(`none`))
}
", "HELLO\n";
}
