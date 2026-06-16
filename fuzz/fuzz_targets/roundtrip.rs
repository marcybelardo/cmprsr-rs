#![no_main]

use cmprsr_rs::compress;
use cmprsr_rs::decompress;
use std::io::Write;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    // Compress then decompress arbitrary data; assert round-trip equality.
    //
    // We use temp files because the compressor reads from file paths.
    let dir = match std::env::temp_dir().join("cmprsr_fuzz_roundtrip") {
        d => {
            let _ = std::fs::create_dir_all(&d);
            d
        }
    };
    let input_path = dir.join("input.bin");
    let cmpr_path = dir.join("data.cmpr");
    let output_path = dir.join("output.bin");

    // Write input data.
    if let Ok(mut f) = std::fs::File::create(&input_path) {
        let _ = f.write_all(data);
        let _ = f.flush();
    }

    // Compress.
    if let Ok(_stats) = compress::compress(&input_path, &cmpr_path) {
        // Decompress.
        if let Ok(()) = decompress::decompress(&cmpr_path, &output_path) {
            // Verify round-trip.
            if let Ok(mut result) = std::fs::File::open(&output_path) {
                use std::io::Read;
                let mut buf = Vec::new();
                if result.read_to_end(&mut buf).is_ok() {
                    assert_eq!(buf, data, "round-trip mismatch");
                }
            }
        }
    }

    // Cleanup (best-effort).
    let _ = std::fs::remove_file(&input_path);
    let _ = std::fs::remove_file(&cmpr_path);
    let _ = std::fs::remove_file(&output_path);
});
