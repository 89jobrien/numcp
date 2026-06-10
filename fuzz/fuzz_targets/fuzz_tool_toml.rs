#![no_main]

use libfuzzer_sys::fuzz_target;
use numcp::config::Config;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Must not panic — only return Ok or a handled error.
        let _ = Config::parse(s);
    }
});
