use std::io::{Read, Write};

// ---------------------------------------------------------------------------
// Format constants
// ---------------------------------------------------------------------------

/// Magic bytes identifying a `.cmpr` file: `"CM PR"`.
pub const MAGIC: [u8; 4] = [0x43, 0x4D, 0x50, 0x52];

/// Current file format version.
pub const VERSION: u8 = 0x01;

/// Offset of the padding byte in the header.
#[allow(dead_code)]
pub const PADDING_OFFSET: u64 = 13;

/// Size of the fixed portion of the header (before the symbol table).
#[allow(dead_code)]
pub const FIXED_HEADER_SIZE: u64 = 16;

// ---------------------------------------------------------------------------
// Header type
// ---------------------------------------------------------------------------

/// Parsed `.cmpr` file header.
#[derive(Debug, PartialEq)]
pub struct Header {
    /// Original uncompressed file size in bytes.
    pub original_size: u64,
    /// Number of padding bits (0..7) in the final byte of the bitstream.
    pub padding: u8,
    /// Number of distinct symbols in the symbol table.
    pub symbol_count: u16,
    /// Symbol table entries: (byte_value, code_length_in_bits).
    /// Sorted by byte value.
    pub symbol_table: Vec<(u8, u8)>,
}

// ---------------------------------------------------------------------------
// Write helpers
// ---------------------------------------------------------------------------

/// Writes a complete `.cmpr` header to `writer`.
///
/// `symbol_table` must be sorted by byte value.  The caller is responsible
/// for seeking back to [`PADDING_OFFSET`] and writing the correct padding
/// byte after the compressed bitstream is complete.
pub fn write_header<W: Write>(
    writer: &mut W,
    original_size: u64,
    symbol_table: &[(u8, u8)],
    padding: u8,
) -> std::io::Result<()> {
    debug_assert!(padding <= 7, "padding must be 0..7");

    writer.write_all(&MAGIC)?;
    writer.write_all(&[VERSION])?;
    writer.write_all(&original_size.to_le_bytes())?;
    writer.write_all(&[padding])?;
    writer.write_all(&(symbol_table.len() as u16).to_le_bytes())?;
    for &(symbol, code_len) in symbol_table {
        writer.write_all(&[symbol, code_len])?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Read helpers
// ---------------------------------------------------------------------------

/// Reads and validates a `.cmpr` header from `reader`.
///
/// Returns an `InvalidData` error if the magic, version, padding, or symbol
/// count are out of range.
pub fn read_header<R: Read>(reader: &mut R) -> std::io::Result<Header> {
    // --- Magic ---
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if magic != MAGIC {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "Invalid magic bytes: expected {MAGIC:02X?}, got {magic:02X?}"
            ),
        ));
    }

    // --- Version ---
    let mut version = [0u8; 1];
    reader.read_exact(&mut version)?;
    if version[0] != VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Unsupported format version {}", version[0]),
        ));
    }

    // --- Original size ---
    let mut size_buf = [0u8; 8];
    reader.read_exact(&mut size_buf)?;
    let original_size = u64::from_le_bytes(size_buf);

    // --- Padding ---
    let mut padding_buf = [0u8; 1];
    reader.read_exact(&mut padding_buf)?;
    let padding = padding_buf[0];
    if padding > 7 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Invalid padding {padding}: must be 0..7"),
        ));
    }

    // --- Symbol count ---
    let mut count_buf = [0u8; 2];
    reader.read_exact(&mut count_buf)?;
    let symbol_count = u16::from_le_bytes(count_buf);
    if symbol_count > 256 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Invalid symbol count {symbol_count}"),
        ));
    }

    // --- Symbol table ---
    let mut symbol_table = Vec::with_capacity(symbol_count as usize);
    for _ in 0..symbol_count {
        let mut entry = [0u8; 2];
        reader.read_exact(&mut entry)?;
        symbol_table.push((entry[0], entry[1]));
    }

    Ok(Header {
        original_size,
        padding,
        symbol_count,
        symbol_table,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Helper: write then immediately read back a header.
    fn round_trip(symbol_table: Vec<(u8, u8)>, original_size: u64, padding: u8) -> Header {
        let mut buf = Vec::new();
        write_header(&mut buf, original_size, &symbol_table, padding).unwrap();
        let mut cursor = Cursor::new(buf);
        read_header(&mut cursor).unwrap()
    }

    #[test]
    fn round_trip_small_table() {
        let table = vec![(0x41, 3), (0x42, 3), (0x43, 2)];
        let h = round_trip(table.clone(), 1000, 3);
        assert_eq!(h.original_size, 1000);
        assert_eq!(h.padding, 3);
        assert_eq!(h.symbol_count, 3);
        assert_eq!(h.symbol_table, table);
    }

    #[test]
    fn round_trip_all_256() {
        let table: Vec<(u8, u8)> = (0..=255).map(|b| (b, 8)).collect();
        let h = round_trip(table.clone(), 1_000_000, 0);
        assert_eq!(h.original_size, 1_000_000);
        assert_eq!(h.padding, 0);
        assert_eq!(h.symbol_count, 256);
        assert_eq!(h.symbol_table, table);
    }

    #[test]
    fn reject_bad_magic() {
        let buf: Vec<u8> = vec![0x00, 0x00, 0x00, 0x00];
        let mut cursor = Cursor::new(buf);
        let err = read_header(&mut cursor).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("magic"));
    }

    #[test]
    fn reject_bad_version() {
        let mut buf = MAGIC.to_vec();
        buf.push(0xFF); // bad version
        let mut cursor = Cursor::new(buf);
        let err = read_header(&mut cursor).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("version"));
    }

    #[test]
    fn reject_bad_padding() {
        let mut buf = MAGIC.to_vec();
        buf.push(VERSION);
        buf.extend(&0u64.to_le_bytes()); // size
        buf.push(8); // padding > 7
        let mut cursor = Cursor::new(buf);
        let err = read_header(&mut cursor).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("padding"));
    }

    #[test]
    fn reject_bad_symbol_count() {
        let mut buf = MAGIC.to_vec();
        buf.push(VERSION);
        buf.extend(&0u64.to_le_bytes()); // size
        buf.push(0); // padding
        buf.extend(&257u16.to_le_bytes()); // symbol_count = 257 (invalid)
        let mut cursor = Cursor::new(buf);
        let err = read_header(&mut cursor).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("symbol count"));
    }

    #[test]
    fn header_too_short_triggers_unexpected_eof() {
        let buf = MAGIC.to_vec(); // only 4 bytes
        let mut cursor = Cursor::new(buf);
        let err = read_header(&mut cursor).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn header_size_calculation() {
        let table = vec![(b'A', 3), (b'B', 4)];
        let mut buf = Vec::new();
        write_header(&mut buf, 100, &table, 1).unwrap();
        // Fixed header: 4 + 1 + 8 + 1 + 2 = 16 bytes
        // Symbol table: 2 entries * 2 bytes = 4 bytes
        // Total: 20 bytes
        assert_eq!(buf.len(), 20);
    }
}
