#![no_main]

use libfuzzer_sys::fuzz_target;
use process_diagnostics::{explain_transcript, replay_transcript};

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    if let Ok(transcript) = replay_transcript(&input) {
        let _ = explain_transcript(&transcript);
    }
});
