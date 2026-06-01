use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::Path;

use crate::bitio::BitReader;
use crate::format;

/// Maximum Huffman code length in bits (must match `huffman::MAX_CODE_LENGTH`).
const MAX_CODE_LENGTH: u8 = 32;

/// Compressed data structure for efficient canonical decoding.
///
/// At each bit length we store all `(code, symbol)` pairs assigned at that
/// length, so decoding can check only the relevant set after each bit.
struct HuffmanDecoder {
    codes_by_len: [Vec<(u64, u8)>; MAX_CODE_LENGTH as usize + 1],
}

impl HuffmanDecoder {
    /// Builds a decoder from a symbol table.
    ///
    /// `symbol_table` is a list of `(byte_value, code_length)` pairs sorted by
    /// byte value, as produced by the compressor.
    fn new(symbol_table: &[(u8, u8)]) -> Self {
        // symbol_table entries are (byte, code_len) from the file header.
        // Sort by (code_len, byte_value) for canonical ordering.
        let mut symbols: Vec<(u8, u8)> = symbol_table.to_vec();
        symbols.sort_by_key(|&(byte, len)| (len, byte));

        // Count how many symbols have each code length.
        let mut count_by_len = [0u16; MAX_CODE_LENGTH as usize + 1];
        for &(_, len) in &symbols {
            count_by_len[len as usize] += 1;
        }

        // Compute starting code for each length using the canonical formula.
        let mut next_code = [0u64; MAX_CODE_LENGTH as usize + 1];
        let mut code = 0u64;
        for len in 1..=MAX_CODE_LENGTH as usize {
            code = (code + count_by_len[len - 1] as u64) << 1;
            next_code[len] = code;
        }

        // Assign canonical codes, group by length.
        let mut codes_by_len: [Vec<(u64, u8)>; MAX_CODE_LENGTH as usize + 1] =
            array_init();
        for &(byte, len) in &symbols {
            let len = len as usize;
            let c = next_code[len];
            codes_by_len[len].push((c, byte));
            next_code[len] = c.wrapping_add(1);
        }

        HuffmanDecoder { codes_by_len }
    }

    /// Reads one byte from the bit stream.
    ///
    /// Returns `None` if the reader reaches EOF before a complete code is read.
    fn decode_byte<R: Read>(&self, reader: &mut BitReader<R>) -> std::io::Result<Option<u8>> {
        let mut value = 0u64;

        for len in 1..=MAX_CODE_LENGTH as usize {
            match reader.read_bit()? {
                Some(bit) => {
                    value = (value << 1) | u64::from(bit);
                    for &(code, symbol) in &self.codes_by_len[len] {
                        if code == value {
                            return Ok(Some(symbol));
                        }
                    }
                }
                None => return Ok(None),
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Code length exceeded maximum of 32 bits",
        ))
    }
}

/// Helper to create an array of empty Vecs (since `[Vec; 33]` doesn't
/// implement `Default` for arbitrary sizes).
fn array_init<T, const N: usize>() -> [Vec<T>; N] {
    [(); N].map(|_| Vec::new())
}

/// Decompresses `input_path` (a `.cmpr` file) to `output_path`.
///
/// The decompression pipeline is:
///   1. Read and validate the file header.
///   2. Reconstruct canonical Huffman codes from the symbol table.
///   3. Decode the bitstream, writing the original bytes to the output.
///
/// If `output_path` already exists, it is overwritten and a warning is
/// printed to stderr.
pub fn decompress(input_path: &Path, output_path: &Path) -> std::io::Result<()> {
    // ------------------------------------------------------------------
    // 1. Read header
    // ------------------------------------------------------------------
    let mut input_file = File::open(input_path)?;
    let header = format::read_header(&mut input_file)?;

    // ------------------------------------------------------------------
    // 2. Reconstruct canonical codes
    // ------------------------------------------------------------------
    let decoder = HuffmanDecoder::new(&header.symbol_table);

    // ------------------------------------------------------------------
    // 3. Warn if overwriting an existing file
    // ------------------------------------------------------------------
    if output_path.exists() {
        eprintln!(
            "Warning: overwriting existing file `{}`",
            output_path.display()
        );
    }

    // ------------------------------------------------------------------
    // 4. Create output file
    // ------------------------------------------------------------------
    let output_file = File::create(output_path)?;
    let mut writer = BufWriter::new(output_file);

    // ------------------------------------------------------------------
    // 5. Decode bitstream
    // ------------------------------------------------------------------
    let mut bit_reader = BitReader::new(&mut input_file);
    let mut out_buf = [0u8; 4096];
    let mut buf_pos = 0;
    let mut decoded: u64 = 0;

    while decoded < header.original_size {
        let byte = decoder.decode_byte(&mut bit_reader)?;
        match byte {
            Some(b) => {
                out_buf[buf_pos] = b;
                buf_pos += 1;
                decoded += 1;

                if buf_pos == out_buf.len() {
                    writer.write_all(&out_buf)?;
                    buf_pos = 0;
                }
            }
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Unexpected end of compressed bitstream",
                ));
            }
        }
    }

    // Flush remaining buffered output.
    if buf_pos > 0 {
        writer.write_all(&out_buf[..buf_pos])?;
    }
    writer.flush()?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compress::compress;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_prefix() -> String {
        let pid = std::process::id();
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("{pid:x}_{n:x}")
    }

    /// Helper: compress `data`, then decompress and return the result.
    fn round_trip(data: &[u8]) -> Vec<u8> {
        let dir = std::env::temp_dir().join("cmprsr_test").join("decompress");
        let _ = std::fs::create_dir_all(&dir);

        let subdir = dir.join(unique_prefix());
        let _ = std::fs::create_dir_all(&subdir);

        let input_path = subdir.join("input.bin");
        let cmpr_path = subdir.join("data.cmpr");
        let output_path = subdir.join("output.bin");

        // Write input.
        let mut f = File::create(&input_path).unwrap();
        f.write_all(data).unwrap();
        f.flush().unwrap();

        // Compress.
        compress(&input_path, &cmpr_path).unwrap();

        // Decompress.
        decompress(&cmpr_path, &output_path).unwrap();

        // Read back.
        let mut result = Vec::new();
        let mut f = File::open(&output_path).unwrap();
        f.read_to_end(&mut result).unwrap();

        // Cleanup.
        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&cmpr_path);
        let _ = std::fs::remove_file(&output_path);

        result
    }

    #[test]
    fn round_trip_empty() {
        let result = round_trip(b"");
        assert_eq!(result, b"");
    }

    #[test]
    fn round_trip_single_byte() {
        let result = round_trip(&[0xAB; 100]);
        assert_eq!(result, vec![0xABu8; 100]);
    }

    #[test]
    fn round_trip_two_bytes() {
        let data: Vec<u8> = [0x00u8, 0xFF].iter().cycle().take(200).copied().collect();
        let result = round_trip(&data);
        assert_eq!(result, data);
    }

    #[test]
    fn round_trip_varied_text() {
        let data = b"The quick brown fox jumps over the lazy dog.";
        let result = round_trip(data);
        assert_eq!(result, data);
    }

    #[test]
    fn round_trip_all_256_bytes() {
        let data: Vec<u8> = (0..=255).collect();
        let result = round_trip(&data);
        assert_eq!(result, data);
    }

    #[test]
    fn round_trip_large_data() {
        let data: Vec<u8> = (0..=255u8).cycle().take(10_000).collect();
        let result = round_trip(&data);
        assert_eq!(result, data);
    }
}
