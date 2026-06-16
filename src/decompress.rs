use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::bitio::BitReader;
use crate::format;
use crate::huffman::MAX_CODE_LENGTH;

/// Number of bits to use for the fast-prefix lookup table.
const LUT_BITS: u8 = 11;

/// Number of entries in the prefix lookup table.
const LUT_SIZE: usize = 1 << LUT_BITS as usize;

/// Compressed data structure for efficient canonical decoding.
///
/// Decoding uses a two-level strategy:
/// 1. Fast path: a prefix lookup table maps the first `LUT_BITS` bits to
///    `(symbol, code_length)` for all codes of length ≤ `LUT_BITS`.
/// 2. Fallback: for codes longer than `LUT_BITS`, a linear scan grouped by
///    bit length is used (as before).
struct HuffmanDecoder {
    /// Fast-prefix lookup table: maps an `LUT_BITS`-bit prefix to
    /// `Some((symbol, code_len))` if the prefix uniquely identifies a
    /// code, or `None` if the prefix is ambiguous (code longer than
    /// `LUT_BITS`).
    lookup: [Option<(u8, u8)>; LUT_SIZE],
    /// Fallback: codes grouped by length for codes longer than `LUT_BITS`.
    codes_by_len: [Vec<(u64, u8)>; MAX_CODE_LENGTH as usize + 1],
}

impl HuffmanDecoder {
    /// Builds a decoder from a symbol table.
    ///
    /// `symbol_table` is a list of `(byte_value, code_length)` pairs sorted by
    /// byte value, as produced by the compressor.
    ///
    /// Returns an `InvalidData` error if any code length is 0 or exceeds
    /// [`MAX_CODE_LENGTH`].
    fn new(symbol_table: &[(u8, u8)]) -> std::io::Result<Self> {
        // Validate code lengths.
        for &(byte, len) in symbol_table {
            if len == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Byte 0x{byte:02X} has zero-length code"),
                ));
            }
            if len > MAX_CODE_LENGTH {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Byte 0x{byte:02X} code length {len} exceeds maximum of {MAX_CODE_LENGTH}"
                    ),
                ));
            }
        }

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

        // Build the prefix lookup table for codes up to LUT_BITS long.
        let mut lookup: [Option<(u8, u8)>; LUT_SIZE] = [None; LUT_SIZE];
        for len in 1..=LUT_BITS as usize {
            for &(code, symbol) in &codes_by_len[len] {
                // Shift the code to fill LUT_BITS bits, then fill all
                // suffix combinations.
                let shift = LUT_BITS as u64 - len as u64;
                let base = code << shift;
                let count = 1u64 << shift;
                for i in 0..count {
                    lookup[(base | i) as usize] = Some((symbol, len as u8));
                }
            }
        }

        Ok(HuffmanDecoder {
            lookup,
            codes_by_len,
        })
    }

    /// Reads one byte from the bit stream.
    ///
    /// Returns `None` if the reader reaches EOF before a complete code is read.
    fn decode_byte<R: Read>(&self, reader: &mut BitReader<R>) -> std::io::Result<Option<u8>> {
        // Fast path: try the prefix lookup table.
        match reader.peek_bits(LUT_BITS)? {
            Some(prefix) => {
                if let Some((symbol, code_len)) = self.lookup[prefix as usize] {
                    reader.consume_bits(code_len);
                    return Ok(Some(symbol));
                }
            }
            None => return Ok(None),
        }

        // Fallback: linear scan for codes longer than LUT_BITS.
        // Read the first LUT_BITS bits into the accumulator, then continue
        // bit-by-bit checking against codes_by_len for longer lengths.
        let mut value = 0u64;
        for _ in 0..LUT_BITS {
            match reader.read_bit()? {
                Some(bit) => value = (value << 1) | u64::from(bit),
                None => return Ok(None),
            }
        }
        for len in (LUT_BITS as usize + 1)..=MAX_CODE_LENGTH as usize {
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
pub fn decompress(input_path: &Path, output_path: &Path) -> std::io::Result<()> {
    let output_file = File::create(output_path)?;
    let mut writer = BufWriter::new(output_file);
    decode_to_writer(input_path, &mut writer)
}

/// Decompresses `input_path` (a `.cmpr` file) to stdout.
pub fn decompress_to_stdout(input_path: &Path) -> std::io::Result<()> {
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    decode_to_writer(input_path, &mut writer)
}

/// Shared decode implementation: reads a `.cmpr` file and writes the
/// decompressed bytes to `writer`.  CRC-32 is verified for v0x02+ files.
fn decode_to_writer<W: Write>(input_path: &Path, writer: &mut W) -> std::io::Result<()> {
    // ------------------------------------------------------------------
    // 1. Read header
    // ------------------------------------------------------------------
    let mut input_file = File::open(input_path)?;
    let header = format::read_header(&mut input_file)?;

    // Record compressed data boundaries (for CRC verification).
    let header_size = format::FIXED_HEADER_SIZE as u64 + header.symbol_count as u64 * 2;
    let file_len = input_file.metadata()?.len();

    // ------------------------------------------------------------------
    // 2. Reconstruct canonical codes
    // ------------------------------------------------------------------
    let decoder = HuffmanDecoder::new(&header.symbol_table)?;

    // ------------------------------------------------------------------
    // 3. Decode bitstream
    // ------------------------------------------------------------------
    {
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
    } // bit_reader dropped here, input_file is no longer borrowed

    // ------------------------------------------------------------------
    // 4. Verify CRC-32 (v0x02+)
    // ------------------------------------------------------------------
    if header.version >= 0x02 {
        let compressed_len = (file_len - header_size - format::CRC_SIZE) as usize;
        input_file.seek(SeekFrom::Start(header_size))?;
        let mut compressed_data = vec![0u8; compressed_len];
        input_file.read_exact(&mut compressed_data)?;
        let expected_crc = format::crc32(&compressed_data);

        let mut crc_buf = [0u8; 4];
        input_file.read_exact(&mut crc_buf)?;
        let actual_crc = u32::from_le_bytes(crc_buf);

        if actual_crc != expected_crc {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "CRC-32 mismatch: expected {expected_crc:#010x}, got {actual_crc:#010x}"
                ),
            ));
        }
    }

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

    /// Helper: compress, then corrupt one bit in the compressed data.
    #[test]
    fn crc_rejects_corrupted_data() {
        let data = b"The quick brown fox jumps over the lazy dog.";
        let dir = std::env::temp_dir().join("cmprsr_test").join("crc_reject");
        let _ = std::fs::create_dir_all(&dir);
        let subdir = dir.join(unique_prefix());
        let _ = std::fs::create_dir_all(&subdir);

        let input_path = subdir.join("input.bin");
        let cmpr_path = subdir.join("data.cmpr");

        let mut f = File::create(&input_path).unwrap();
        f.write_all(data).unwrap();
        f.flush().unwrap();

        compress(&input_path, &cmpr_path).unwrap();

        // Read the file and determine where the compressed data starts.
        let mut cmpr_data = std::fs::read(&cmpr_path).unwrap();
        // The file must be at least big enough to have header + 1 byte of
        // compressed data + 4 bytes CRC.
        let file_len = cmpr_data.len();
        if file_len > format::FIXED_HEADER_SIZE as usize + 4 + format::CRC_SIZE as usize {
            // Decode header to find the start of compressed data.
            let mut cursor = std::io::Cursor::new(&cmpr_data[..]);
            let header = format::read_header(&mut cursor).unwrap();
            let header_size = format::FIXED_HEADER_SIZE as usize + header.symbol_count as usize * 2;
            // Flip a bit in the compressed data region.
            if header_size < file_len - format::CRC_SIZE as usize {
                cmpr_data[header_size] ^= 1 << 3;
                std::fs::write(&cmpr_path, &cmpr_data).unwrap();

                // Decompression should fail with CRC mismatch.
                let output_path = subdir.join("output.bin");
                let err = decompress(&cmpr_path, &output_path).unwrap_err();
                assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
                assert!(err.to_string().contains("CRC-32 mismatch"));
            }
        }

        let _ = std::fs::remove_dir_all(&subdir);
    }

    /// Decompress a manually-constructed v0x01 file (no CRC trailer).
    #[test]
    fn backward_compat_v0x01() {
        // Build a minimal v0x01 .cmpr file:
        //   - 1 symbol 'A' with 1-bit code
        //   - original_size = 5
        //   - compressed bitstream: 00000 (5 zeros, padded to byte)
        //   - No CRC trailer.
        let mut buf = Vec::new();
        buf.extend_from_slice(b"CMPR");           // magic
        buf.push(0x01);                           // version 0x01
        buf.extend(&5u64.to_le_bytes());          // original_size = 5
        buf.push(3);                              // padding = 3
        buf.extend(&1u16.to_le_bytes());          // symbol_count = 1
        buf.extend(&[b'A', 1]);                   // symbol 'A', code_len = 1
        buf.push(0b0000_0000);                    // compressed data: 5 zero bits + 3 padding

        let dir = std::env::temp_dir().join("cmprsr_test").join("v0x01");
        let _ = std::fs::create_dir_all(&dir);
        let subdir = dir.join(unique_prefix());
        let _ = std::fs::create_dir_all(&subdir);

        let cmpr_path = subdir.join("data.cmpr");
        std::fs::write(&cmpr_path, &buf).unwrap();

        let output_path = subdir.join("output.bin");
        decompress(&cmpr_path, &output_path).unwrap();

        let mut result = Vec::new();
        let mut f = File::open(&output_path).unwrap();
        f.read_to_end(&mut result).unwrap();
        assert_eq!(result, vec![b'A'; 5]);

        let _ = std::fs::remove_dir_all(&subdir);
    }

    #[test]
    fn reject_all_zeros() {
        let dir = std::env::temp_dir().join("cmprsr_test").join("reject_zeros");
        let _ = std::fs::create_dir_all(&dir);
        let subdir = dir.join(unique_prefix());
        let _ = std::fs::create_dir_all(&subdir);

        let cmpr_path = subdir.join("data.cmpr");
        std::fs::write(&cmpr_path, &[0u8; 32]).unwrap();

        let output_path = subdir.join("output.bin");
        let err = decompress(&cmpr_path, &output_path).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);

        let _ = std::fs::remove_dir_all(&subdir);
    }

    #[test]
    fn reject_truncated_header() {
        let dir = std::env::temp_dir().join("cmprsr_test").join("reject_trunc");
        let _ = std::fs::create_dir_all(&dir);
        let subdir = dir.join(unique_prefix());
        let _ = std::fs::create_dir_all(&subdir);

        // Try various truncated lengths.
        for len in [3, 5, 10, 15] {
            let cmpr_path = subdir.join("data.cmpr");
            let mut buf = b"CM PR\x02\x37\x00\x00\x00\x00\x00\x00\x00\x02\x02".to_vec();
            buf.truncate(len);
            std::fs::write(&cmpr_path, &buf).unwrap();

            let output_path = subdir.join("output.bin");
            let err = decompress(&cmpr_path, &output_path);
            assert!(err.is_err(), "Expected error for truncated header len={len}");
        }

        let _ = std::fs::remove_dir_all(&subdir);
    }
}
