#![no_main]

use libfuzzer_sys::fuzz_target;
use numcp::config::Config;
use numcp::registry::ToolRegistry;

// A static valid config — the fuzzer explores arbitrary argument JSON.
const STATIC_CONFIG: &str = r#"
[[tool]]
name        = "echo"
description = "echo"
handler     = "/dev/null"

[tool.parameters.msg]
type        = "string"
description = "message"
required    = false
"#;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Parse arbitrary JSON as tool call arguments — must not panic.
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
            // Also exercise registry deserialization path.
            if let Ok(cfg) = Config::parse(STATIC_CONFIG) {
                let base = std::path::Path::new("/");
                let _reg = ToolRegistry::load_unchecked(&cfg, base);
            }
            let _ = v; // args would be forwarded to executor in real call
        }
    }
});
