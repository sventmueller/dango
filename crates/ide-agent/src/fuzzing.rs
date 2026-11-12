//! Entry points for the fuzz targets in `fuzz/`.

use serde_json::Value;

use crate::sse::Events;

/// Reads `bytes` as a server-sent event stream and hands its events to the stream readers of all three providers.
pub fn stream(bytes: &[u8]) {
    let chunks: Vec<Value> = Events::new(bytes)
        .map_while(Result::ok)
        .filter_map(|event| crate::http::parse(&event.data).ok())
        .collect();
    crate::anthropic::read_all(&chunks);
    crate::openai::read_all(&chunks);
    crate::gemini::read_all(&chunks);
}
