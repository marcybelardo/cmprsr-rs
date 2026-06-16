#![no_main]

use cmprsr_rs::decompress;
use std::io::Write;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    // Feed arbitrary bytes as a .cmpr file to the decompressor.
    // The decompressor should never panic — only return an error.
    let dir = match std::env::temp_dir().join("cmprsr_fuzz_decomp") {
        d => {
            let _ = std::fs::create_dir_all(&d);
            d
        }
    };
    let cmpr_path = dir.join("data.cmpr");
    let output_path = dir.join("output.bin");

    if let Ok(mut f) = std::fs::File::create(&cmpr_path) {
        let _ = f.write_all(data);
        let _ = f.flush();
    }

    // Decompress — this must not panic.
    let _ = decompress::decompress(&cmpr_path, &output_path);

    // Cleanup (best-effort).
    let _ = std::fs::remove_file(&cmpr_path);
    let _ = std::fs::remove_file(&output_path);
});
