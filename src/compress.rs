use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::bitio::BitWriter;
use crate::format;
use crate::frequency;
use crate::huffman;

/// Compresses `input_path` and writes the `.cmpr` output to `output_path`.
///
/// The compression pipeline is:
///   1. Count byte frequencies (single pass over the input).
///   2. Build canonical Huffman codes.
///   3. Write the file header (with a placeholder padding byte).
///   4. Encode the original bytes as a canonical bitstream.
///   5. Flush the bit writer and seek back to fill in the actual padding.
///
/// If `output_path` already exists, it is overwritten and a warning is
/// printed to stderr.
pub fn compress(input_path: &Path, output_path: &Path) -> std::io::Result<()> {
    // ------------------------------------------------------------------
    // 1. Frequency analysis
    // ------------------------------------------------------------------
    let original_size = input_path.metadata()?.len();
    let input_file = File::open(input_path)?;
    let freqs = frequency::count_frequencies(input_file)?;

    // ------------------------------------------------------------------
    // 2. Build canonical Huffman codes
    // ------------------------------------------------------------------
    let table = huffman::build_codes(&freqs).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })?;

    // ------------------------------------------------------------------
    // 3. Build symbol table (sorted by byte value)
    // ------------------------------------------------------------------
    let symbol_table: Vec<(u8, u8)> = (0..=255u16)
        .filter(|&b| table.code_len[b as usize] > 0)
        .map(|b| (b as u8, table.code_len[b as usize]))
        .collect();

    // ------------------------------------------------------------------
    // 4. Warn if overwriting an existing file
    // ------------------------------------------------------------------
    if output_path.exists() {
        eprintln!(
            "Warning: overwriting existing file `{}`",
            output_path.display()
        );
    }

    // ------------------------------------------------------------------
    // 5. Write header (placeholder padding = 0)
    // ------------------------------------------------------------------
    let output_file = File::create(output_path)?;
    let mut writer = BufWriter::new(output_file);
    format::write_header(&mut writer, original_size, &symbol_table, 0)?;
    writer.flush()?;

    // Obtain the raw `File` so we can seek back to update the padding
    // byte after the bitstream is complete.
    let mut file = writer.into_inner()?;

    // ------------------------------------------------------------------
    // 6. Encode input bytes as canonical bitstream
    // ------------------------------------------------------------------
    let input_file = File::open(input_path)?;
    let mut reader = BufReader::new(input_file);
    let mut buf = [0u8; 8192];

    let padding = {
        let mut bit_writer = BitWriter::new(&mut file);

        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            for &byte in &buf[..n] {
                let len = table.code_len[byte as usize];
                if len > 0 {
                    bit_writer.write_bits(table.code[byte as usize], len)?;
                }
            }
        }

        // Flush the bit writer and capture the padding count before
        // the borrow of `file` is released.
        bit_writer.flush()?
    };

    // ------------------------------------------------------------------
    // 7. Seek back and write the actual padding byte
    // ------------------------------------------------------------------
    file.seek(SeekFrom::Start(format::PADDING_OFFSET))?;
    file.write_all(&[padding])?;
    file.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_prefix() -> String {
        let pid = std::process::id();
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("{pid:x}_{n:x}")
    }

    /// Helper: write `data` to a temp file, compress it, return the path
    /// of the compressed output.
    fn compress_to_temp(data: &[u8]) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join("cmprsr_test").join("compress");
        let _ = std::fs::create_dir_all(&dir);

        // Each call gets its own subdirectory to avoid parallel-test
        // collisions even when different test modules share the prefix space.
        let subdir = dir.join(unique_prefix());
        let _ = std::fs::create_dir_all(&subdir);

        let input_path = subdir.join("input.bin");
        let output_path = subdir.join("output.cmpr");

        let mut f = File::create(&input_path).unwrap();
        f.write_all(data).unwrap();
        f.flush().unwrap();

        compress(&input_path, &output_path).unwrap();

        (input_path, output_path)
    }

    #[test]
    fn compress_creates_output_file() {
        let (_input, output) = compress_to_temp(b"hello");
        assert!(output.exists());
        let metadata = std::fs::metadata(&output).unwrap();
        assert!(metadata.len() > 0);
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn compress_output_starts_with_magic() {
        let (_input, output) = compress_to_temp(b"hello");
        let mut f = File::open(&output).unwrap();
        let mut magic = [0u8; 4];
        f.read_exact(&mut magic).unwrap();
        assert_eq!(magic, format::MAGIC);
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn compress_empty_file_produces_valid_header() {
        let (_input, output) = compress_to_temp(b"");
        let mut f = File::open(&output).unwrap();
        let header = format::read_header(&mut f).unwrap();
        assert_eq!(header.original_size, 0);
        assert_eq!(header.symbol_count, 0);
        assert_eq!(header.padding, 0);
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn compress_single_byte() {
        let (_input, output) = compress_to_temp(&[0xAB; 100]);
        let mut f = File::open(&output).unwrap();
        let header = format::read_header(&mut f).unwrap();
        assert_eq!(header.original_size, 100);
        assert_eq!(header.symbol_count, 1);
        assert_eq!(header.symbol_table, vec![(0xAB, 1)]);
        // Padding should be 0 because 100 bits * 1-bit code = 100 bits
        // = 12 full bytes + 4 bits, so padding = 4
        assert_eq!(header.padding, 4);
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn compress_reproduces_correct_header() {
        let data = b"The quick brown fox jumps over the lazy dog.";
        let (_input, output) = compress_to_temp(data);
        let mut f = File::open(&output).unwrap();
        let header = format::read_header(&mut f).unwrap();
        assert_eq!(header.original_size, data.len() as u64);
        assert!(header.symbol_count > 0 && header.symbol_count <= data.len() as u16);
        // Padding should be 0..7
        assert!(header.padding <= 7);
        let _ = std::fs::remove_file(&output);
    }
}
