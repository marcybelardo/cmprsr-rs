use std::io::{BufReader, BufWriter, Read, Write};

// ---------------------------------------------------------------------------
// BitWriter -- accumulate bits, flush entire bytes to the underlying writer
// ---------------------------------------------------------------------------

/// Wraps a `BufWriter<W>` and provides bit-level write operations.
///
/// Bits are written MSB-first within each byte.  Call [`flush`](BitWriter::flush)
/// after the final write to flush any remaining partial byte (padded with zeros)
/// and obtain the padding count for the file header.
pub struct BitWriter<W: Write> {
    writer: BufWriter<W>,
    buffer: u8,          // Partial byte being accumulated.
    bits_in_buffer: u8,  // 0..8 bits currently held in `buffer`.
}

impl<W: Write> BitWriter<W> {
    /// Creates a new `BitWriter` wrapping the given writer.
    pub fn new(inner: W) -> Self {
        BitWriter {
            writer: BufWriter::new(inner),
            buffer: 0,
            bits_in_buffer: 0,
        }
    }

    /// Writes a single bit (true = 1, false = 0).
    pub fn write_bit(&mut self, bit: bool) -> std::io::Result<()> {
        if bit {
            self.buffer |= 1 << (7 - self.bits_in_buffer);
        }
        self.bits_in_buffer += 1;

        if self.bits_in_buffer == 8 {
            self.writer.write_all(&[self.buffer])?;
            self.buffer = 0;
            self.bits_in_buffer = 0;
        }

        Ok(())
    }

    /// Writes `n_bits` from `value`, MSB first.
    ///
    /// Only the bottom `n_bits` of `value` are meaningful; higher bits are ignored.
    pub fn write_bits(&mut self, value: u64, n_bits: u8) -> std::io::Result<()> {
        for i in (0..n_bits).rev() {
            let bit = ((value >> i) & 1) != 0;
            self.write_bit(bit)?;
        }
        Ok(())
    }

    /// Flushes any remaining partial byte (padded with zero bits) and flushes
    /// the underlying `BufWriter`.
    ///
    /// Returns the number of padding bits added (0..7).  This value must be
    /// stored in the `.cmpr` header so the decompressor can ignore trailing
    /// padding.
    pub fn flush(&mut self) -> std::io::Result<u8> {
        let padding = if self.bits_in_buffer > 0 {
            let pad = 8 - self.bits_in_buffer;
            self.writer.write_all(&[self.buffer])?;
            self.buffer = 0;
            self.bits_in_buffer = 0;
            pad
        } else {
            0
        };

        self.writer.flush()?;
        Ok(padding)
    }

    /// Consumes the writer and returns the inner `BufWriter<W>`.
    ///
    /// **Important**: call [`flush`](BitWriter::flush) first, or any buffered
    /// bits will be lost.
    #[allow(dead_code)]
    pub fn into_inner(self) -> BufWriter<W> {
        self.writer
    }
}

// ---------------------------------------------------------------------------
// BitReader -- read bits from an underlying buffered reader
// ---------------------------------------------------------------------------

/// Wraps a `BufReader<R>` and provides bit-level read operations.
///
/// Bits are read MSB-first from each byte, matching the write order of
/// `BitWriter`.
pub struct BitReader<R: Read> {
    reader: BufReader<R>,
    buffer: u8,           // Current byte being consumed.
    bits_remaining: u8,   // 0..8 unconsumed bits in `buffer`.
}

impl<R: Read> BitReader<R> {
    /// Creates a new `BitReader` wrapping the given reader.
    pub fn new(inner: R) -> Self {
        BitReader {
            reader: BufReader::new(inner),
            buffer: 0,
            bits_remaining: 0,
        }
    }

    /// Reads a single bit.  Returns `Ok(Some(true))` for 1, `Ok(Some(false))` for 0,
    /// or `Ok(None)` when the end of the input is reached.
    pub fn read_bit(&mut self) -> std::io::Result<Option<bool>> {
        if self.bits_remaining == 0 {
            let mut byte_buf = [0u8; 1];
            match self.reader.read(&mut byte_buf)? {
                0 => return Ok(None),
                _ => {
                    self.buffer = byte_buf[0];
                    self.bits_remaining = 8;
                }
            }
        }

        let bit = (self.buffer >> (self.bits_remaining - 1)) & 1;
        self.bits_remaining -= 1;
        Ok(Some(bit != 0))
    }

    /// Reads `n` bits and returns them as a right-aligned `u64`, or `None` at EOF.
    ///
    /// Bits are read MSB-first and packed into the low bits of the result
    /// (the first bit read becomes the MSB of the returned value).
    #[allow(dead_code)]
    pub fn read_bits(&mut self, n: u8) -> std::io::Result<Option<u64>> {
        let mut value = 0u64;
        for _ in 0..n {
            match self.read_bit()? {
                Some(bit) => value = (value << 1) | u64::from(bit),
                None => return Ok(None),
            }
        }
        Ok(Some(value))
    }

    /// Consumes the reader and returns the inner `BufReader<R>`.
    ///
    /// Any remaining buffered bits are discarded.
    #[allow(dead_code)]
    pub fn into_inner(self) -> BufReader<R> {
        self.reader
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // ---- BitWriter tests ----

    #[test]
    fn write_single_bit_round_trip() {
        let mut buf = Vec::new();
        {
            let mut bw = BitWriter::new(&mut buf);
            bw.write_bit(true).unwrap();
            bw.write_bit(false).unwrap();
            bw.write_bit(true).unwrap();
            bw.flush().unwrap();
        }
        // Binary: 101_00000 = 0xA0
        assert_eq!(buf, vec![0xA0]);
    }

    #[test]
    fn write_exactly_one_byte() {
        let mut buf = Vec::new();
        {
            let mut bw = BitWriter::new(&mut buf);
            // Write bits: 1 1 0 0 1 0 1 0
            for &b in &[true, true, false, false, true, false, true, false] {
                bw.write_bit(b).unwrap();
            }
            bw.flush().unwrap();
        }
        assert_eq!(buf, vec![0b1100_1010]);
    }

    #[test]
    fn write_multiple_bytes() {
        let mut buf = Vec::new();
        {
            let mut bw = BitWriter::new(&mut buf);
            // 16 bits: 10101010 01010101
            for _ in 0..4 {
                bw.write_bit(true).unwrap();
                bw.write_bit(false).unwrap();
            }
            for _ in 0..4 {
                bw.write_bit(false).unwrap();
                bw.write_bit(true).unwrap();
            }
            bw.flush().unwrap();
        }
        assert_eq!(buf, vec![0xAA, 0x55]);
    }

    #[test]
    fn write_bits_utility() {
        let mut buf = Vec::new();
        {
            let mut bw = BitWriter::new(&mut buf);
            bw.write_bits(0b1101, 4).unwrap();
            bw.write_bits(0b1010, 4).unwrap();
            bw.flush().unwrap();
        }
        assert_eq!(buf, vec![0b1101_1010]);
    }

    #[test]
    fn flush_padding_count() {
        let mut buf = Vec::new();
        let padding = {
            let mut bw = BitWriter::new(&mut buf);
            bw.write_bit(true).unwrap(); // 1 bit only
            bw.flush().unwrap()
        };
        assert_eq!(padding, 7);
        assert_eq!(buf, vec![0x80]); // 1000_0000
    }

    #[test]
    fn flush_when_already_aligned() {
        let mut buf = Vec::new();
        let padding = {
            let mut bw = BitWriter::new(&mut buf);
            bw.write_bits(0xFF, 8).unwrap();
            bw.flush().unwrap()
        };
        assert_eq!(padding, 0);
        assert_eq!(buf, vec![0xFF]);
    }

    // ---- BitReader tests ----

    #[test]
    fn read_single_bit() {
        let data = vec![0b1010_0000];
        let mut br = BitReader::new(Cursor::new(data));
        assert_eq!(br.read_bit().unwrap(), Some(true));
        assert_eq!(br.read_bit().unwrap(), Some(false));
        assert_eq!(br.read_bit().unwrap(), Some(true));
        assert_eq!(br.read_bit().unwrap(), Some(false));
        // Remaining bits are zeros
        for _ in 0..4 {
            assert_eq!(br.read_bit().unwrap(), Some(false));
        }
        // Now at EOF
        assert_eq!(br.read_bit().unwrap(), None);
    }

    #[test]
    fn read_bits_utility() {
        let data = vec![0b1101_1010];
        let mut br = BitReader::new(Cursor::new(data));
        assert_eq!(br.read_bits(4).unwrap(), Some(0b1101));
        assert_eq!(br.read_bits(4).unwrap(), Some(0b1010));
    }

    #[test]
    fn read_bits_eof_incomplete() {
        let data = vec![0b1111_0000];
        let mut br = BitReader::new(Cursor::new(data));
        assert_eq!(br.read_bits(8).unwrap(), Some(0b1111_0000));
        assert_eq!(br.read_bits(1).unwrap(), None);
    }

    #[test]
    fn round_trip_write_then_read() {
        let mut compressed = Vec::new();
        let padding;
        // Write phase
        {
            let mut bw = BitWriter::new(&mut compressed);
            bw.write_bits(0b0010, 4).unwrap();  // 0010
            bw.write_bit(true).unwrap();        // 1
            bw.write_bit(false).unwrap();       // 0
            bw.write_bits(0b1101_1100, 8).unwrap(); // 1101_1100
            padding = bw.flush().unwrap();
        }
        // Verify we wrote exactly what we expect
        // Bitstream: 0010 1 0 1101_1100
        //           0 0 1 0 1 0 1 1 0 1 1 1 0 0
        // First byte: 0010_1011 = 0x2B
        // Second byte: 01_1100_00 (padded) = 0x70
        assert_eq!(padding, 2);
        assert_eq!(compressed.len(), 2);

        // Read phase (skip padding automatically -- the decoder must know where to stop)
        let mut br = BitReader::new(Cursor::new(compressed));
        assert_eq!(br.read_bits(4).unwrap(), Some(0b0010));
        assert_eq!(br.read_bit().unwrap(), Some(true));
        assert_eq!(br.read_bit().unwrap(), Some(false));
        assert_eq!(br.read_bits(8).unwrap(), Some(0b1101_1100));
        // The remaining 2 bits are padding (zeros)
        assert_eq!(br.read_bit().unwrap(), Some(false));
        assert_eq!(br.read_bit().unwrap(), Some(false));
        assert_eq!(br.read_bit().unwrap(), None);
    }

    #[test]
    fn empty_reader_returns_none() {
        let mut br = BitReader::new(Cursor::new(vec![]));
        assert_eq!(br.read_bit().unwrap(), None);
        assert_eq!(br.read_bits(4).unwrap(), None);
    }

    #[test]
    fn read_across_byte_boundary() {
        let data = vec![0b0000_1111, 0b1010_0000];
        let mut br = BitReader::new(Cursor::new(data));
        assert_eq!(br.read_bits(4).unwrap(), Some(0b0000));
        assert_eq!(br.read_bits(4).unwrap(), Some(0b1111));
        assert_eq!(br.read_bits(4).unwrap(), Some(0b1010));
    }
}
