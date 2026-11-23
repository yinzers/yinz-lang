//! v0.3-M9 C8 (card #385, round 2): the strings a `channel<string>` still holds when its last
//! reference goes are released with the channel. A C6 defect found after close: C3 made strings
//! owned (counted, released), but the channel's element classification kept `string` glue-less
//! ("immortal bytes"), so a string sent and never received leaked at every teardown.
//!
//! Each program is balanced (heap and strings) and AddressSanitizer clean at both optimization
//! levels, each in its own process.

mod support;

use support::assert_source_balanced_and_asan_clean as assert_balanced;

// bug: a string buffered in a channel, and a parked send's string, leaked when the channel's
// last reference went | tier: High
#[test]
fn strings_buffered_and_parked_in_a_channel_are_released_with_it() {
    let sources_and_labels = [
        (
            "channel_string_unreceived",
            "
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
",
            "parent\n",
        ),
    ];
    for (label, source, expected) in sources_and_labels {
        assert_balanced(label, source, expected);
    }
}
