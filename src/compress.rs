use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::bitio::BitWriter;
use crate::format;
use crate::frequency;
use crate::huffman;

/// Compresses `input_path` and writes the `.cmpr` output to `output_path`.
///
/// Returns `(original_size, compressed_file_size)` on success for statistics.
pub fn compress(input_path: &Path, output_path: &Path) -> std::io::Result<(u64, u64)> {
    let (original_size, symbol_table, table) = build_codes(input_path)?;

    let output_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(output_path)?;
    let mut writer = BufWriter::new(output_file);
    format::write_header(&mut writer, original_size, &symbol_table, 0)?;
    writer.flush()?;

    let mut file = writer.into_inner()?;
    encode_and_finalize(&mut file, &mut File::open(input_path)?, &table)?;

    let compressed_size = file.metadata()?.len();
    Ok((original_size, compressed_size))
}

/// Compresses `input_path` and writes the `.cmpr` output to stdout.
///
/// Returns `(original_size, compressed_size)` on success for statistics.
pub fn compress_to_stdout(input_path: &Path) -> std::io::Result<(u64, u64)> {
    let (original_size, symbol_table, table) = build_codes(input_path)?;

    // Buffer everything in memory since we need to seek back for padding.
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    format::write_header(&mut cursor, original_size, &symbol_table, 0)?;

    let compressed_end = encode_and_finalize(&mut cursor, &mut File::open(input_path)?, &table)?;

    let compressed_size = compressed_end;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    out.write_all(&buf)?;
    out.flush()?;
    Ok((original_size, compressed_size))
}

// ---------------------------------------------------------------------------
// Shared internal helpers
// ---------------------------------------------------------------------------

/// Counts frequencies and builds the Huffman code table.
fn build_codes(input_path: &Path) -> std::io::Result<(u64, Vec<(u8, u8)>, huffman::CodeTable)> {
    let original_size = input_path.metadata()?.len();
    let mut input_file = File::open(input_path)?;
    let freqs = frequency::count_frequencies(&mut input_file)?;

    let table = huffman::build_codes(&freqs).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })?;

    let mut symbol_table: Vec<(u8, u8)> =
        Vec::with_capacity(table.symbol_count as usize);
    for b in 0..=255u16 {
        let len = table.code_len[b as usize];
        if len > 0 {
            symbol_table.push((b as u8, len));
        }
    }

    Ok((original_size, symbol_table, table))
}

/// Encodes input data through the bit writer, flushes, writes the real padding
/// byte, and appends the CRC-32 trailer.  `file` must support seeking (regular
/// File or Cursor).
fn encode_and_finalize<W: Write + Read + Seek>(
    file: &mut W,
    input: &mut File,
    table: &huffman::CodeTable,
) -> std::io::Result<u64> {
    // Encode bitstream
    input.seek(SeekFrom::Start(0))?;
    let mut reader = BufReader::new(input);
    let mut buf = [0u8; 8192];

    let padding = {
        let mut bit_writer = BitWriter::new(&mut *file);

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

        bit_writer.flush()?
    };

    // Record compressed data end offset
    let compressed_end = file.stream_position()?;

    // Seek back and write the actual padding byte (the header placeholder
    // was 0).  We need the file position for this, which is header-local.
    let symbol_count = table.symbol_count;
    let header_size = format::FIXED_HEADER_SIZE as u64 + symbol_count as u64 * 2;
    let padding_offset = format::PADDING_OFFSET;
    file.seek(SeekFrom::Start(padding_offset))?;
    file.write_all(&[padding])?;

    // Compute and append CRC-32 over the compressed data bytes
    file.seek(SeekFrom::Start(header_size))?;
    let compressed_len = (compressed_end - header_size) as usize;
    let mut compressed_data = vec![0u8; compressed_len];
    file.read_exact(&mut compressed_data)?;

    let crc = format::crc32(&compressed_data);
    file.seek(SeekFrom::Start(compressed_end))?;
    file.write_all(&crc.to_le_bytes())?;
    file.flush()?;

    Ok(compressed_end + format::CRC_SIZE)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
