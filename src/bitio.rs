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
    ///
    /// When the internal buffer is aligned and `n_bits >= 8`, full bytes are
    /// written directly to the underlying writer without per-bit branching.
    pub fn write_bits(&mut self, value: u64, mut n_bits: u8) -> std::io::Result<()> {
        // Fast path: aligned full-byte writes
        if n_bits >= 8 && self.bits_in_buffer == 0 {
            while n_bits >= 8 {
                let shift = n_bits - 8;
                let byte = (value >> shift) as u8;
                self.writer.write_all(&[byte])?;
                n_bits -= 8;
            }
            if n_bits > 0 {
                // Remaining bits (< 8) go into the buffer, positioned at the
                // top of the byte (MSB-first).
                self.buffer = (value as u8) << (8 - n_bits);
                self.bits_in_buffer = n_bits;
            }
            return Ok(());
        }

        // Slow path: mixed alignment or small writes
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
///
/// The internal buffer holds up to 64 bits, enabling efficient peek-ahead
/// without consuming from the underlying reader.
pub struct BitReader<R: Read> {
    reader: BufReader<R>,
    buffer: u64,           // Accumulated bits, left-aligned.
    bits_remaining: u8,    // 0..64 unconsumed bits in `buffer`.
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

    /// Fills the internal buffer with at least `n` bits by reading from the
    /// underlying reader.
    ///
    /// Returns `true` if enough bits were buffered, `false` on EOF before
    /// filling the request.
    fn fill_bits(&mut self, n: u8) -> std::io::Result<bool> {
        while self.bits_remaining < n {
            let mut byte = [0u8; 1];
            match self.reader.read(&mut byte)? {
                0 => return Ok(self.bits_remaining >= n),
                _ => {
                    self.buffer = (self.buffer << 8) | u64::from(byte[0]);
                    self.bits_remaining += 8;
                }
            }
        }
        Ok(true)
    }

    /// Reads a single bit.  Returns `Ok(Some(true))` for 1, `Ok(Some(false))` for 0,
    /// or `Ok(None)` when the end of the input is reached.
    pub fn read_bit(&mut self) -> std::io::Result<Option<bool>> {
        if !self.fill_bits(1)? {
            return Ok(None);
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
        if n == 0 {
            return Ok(Some(0));
        }

        // Fast path: aligned full-byte reads from the underlying reader.
        if n >= 8 && self.bits_remaining == 0 {
            let mut value = 0u64;
            let mut remaining = n;
            while remaining >= 8 {
                let mut byte = [0u8; 1];
                match self.reader.read(&mut byte)? {
                    0 if remaining == n => return Ok(None),
                    0 => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "Unexpected EOF during multi-byte read",
                        ));
                    }
                    _ => {
                        value = (value << 8) | u64::from(byte[0]);
                        remaining -= 8;
                    }
                }
            }
            if remaining > 0 {
                // Read the partial byte for remaining bits
                let mut byte = [0u8; 1];
                match self.reader.read(&mut byte)? {
                    0 => {} // no more data, return what we have
                    _ => {
                        self.buffer = u64::from(byte[0]);
                        self.bits_remaining = 8;
                        value = (value << remaining) | (self.buffer >> (8 - remaining));
                        self.bits_remaining -= remaining;
                    }
                }
            }
            return Ok(Some(value));
        }

        // General path: use the buffer.
        if !self.fill_bits(n)? {
            return Ok(None);
        }
        // Mask to the unconsumed window, then extract the top n bits.
        let mask = if self.bits_remaining >= 64 {
            !0u64
        } else {
            (1u64 << self.bits_remaining) - 1
        };
        let window = self.buffer & mask;
        let shift = self.bits_remaining - n;
        let value = window >> shift;
        self.bits_remaining -= n;
        Ok(Some(value))
    }

    /// Peeks up to `n` bits from the bit stream without consuming them.
    ///
    /// Returns the peeked bits left-aligned within `n` bits (i.e., the first
    /// bit read occupies the most significant position of the returned value,
    /// and any unavailable trailing bits are zero).  Returns `None` only if
    /// no bits at all are available.
    ///
    /// The stream position is unchanged; call
    /// [`consume_bits`](BitReader::consume_bits) to advance past the peeked
    /// bits.
    pub fn peek_bits(&mut self, n: u8) -> std::io::Result<Option<u64>> {
        if n == 0 {
            return Ok(Some(0));
        }
        // Fill what we can; if nothing at all, return None.
        self.fill_bits(n)?;
        if self.bits_remaining == 0 {
            return Ok(None);
        }
        let mask = if self.bits_remaining >= 64 {
            !0u64
        } else {
            (1u64 << self.bits_remaining) - 1
        };
        let window = self.buffer & mask;
        let take = self.bits_remaining.min(n);
        let shift = self.bits_remaining - take;
        let value = window >> shift;
        // Left-align the value within n bits.
        Ok(Some(value << (n - take)))
    }

    /// Consumes `n` bits from the buffer after a prior [`peek_bits`](BitReader::peek_bits)
    /// or other read operation.
    ///
    /// Silently caps at the number of remaining bits.
    pub fn consume_bits(&mut self, n: u8) {
        self.bits_remaining = self.bits_remaining.saturating_sub(n);
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

    // ---- BitWriter batched I/O tests ----

    #[test]
    fn write_bits_batched_aligned() {
        let mut buf = Vec::new();
        {
            let mut bw = BitWriter::new(&mut buf);
            // Write 16 bits in one call: 0xABCD
            bw.write_bits(0xABCD, 16).unwrap();
            bw.flush().unwrap();
        }
        assert_eq!(buf, vec![0xAB, 0xCD]);
    }

    #[test]
    fn write_bits_batched_plus_remainder() {
        let mut buf = Vec::new();
        {
            let mut bw = BitWriter::new(&mut buf);
            // 10 bits: 10101010 11
            bw.write_bits(0b10101010_11, 10).unwrap();
            bw.flush().unwrap();
        }
        // First byte: 10101010 = 0xAA, second byte: 11_000000 = 0xC0
        assert_eq!(buf, vec![0xAA, 0xC0]);
    }

    #[test]
    fn write_bits_batched_multi_byte() {
        let mut buf = Vec::new();
        {
            let mut bw = BitWriter::new(&mut buf);
            // Write 24 bits: 0x123456
            bw.write_bits(0x123456, 24).unwrap();
            bw.flush().unwrap();
        }
        assert_eq!(buf, vec![0x12, 0x34, 0x56]);
    }

    #[test]
    fn write_bits_batched_small_then_large() {
        let mut buf = Vec::new();
        {
            let mut bw = BitWriter::new(&mut buf);
            // 3 bits first
            bw.write_bits(0b101, 3).unwrap();
            // Then 16 bits (this will hit the slow path due to non-empty buffer)
            bw.write_bits(0xABCD, 16).unwrap();
            bw.flush().unwrap();
        }
        // Bitstream: 101 10101011 11001101
        //           1 0 1 1 0 1 0 1  0 1 1 1 1 0 0 1  1 0 1 0 0 0 0 0
        // First byte: 1011_0101 = 0xB5
        // Second byte: 0111_1001 = 0x79
        // Third byte: 1010_0000 = 0xA0
        assert_eq!(buf, vec![0xB5, 0x79, 0xA0]);
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

    // ---- BitReader peek/consume tests ----

    #[test]
    fn peek_bits_small() {
        let data = vec![0b1101_1010];
        let mut br = BitReader::new(Cursor::new(data));
        // Peek 4 bits without consuming
        assert_eq!(br.peek_bits(4).unwrap(), Some(0b1101));
        // Position unchanged -- read should return same value
        assert_eq!(br.read_bits(4).unwrap(), Some(0b1101));
        // Now peek the remaining bits
        assert_eq!(br.peek_bits(4).unwrap(), Some(0b1010));
        assert_eq!(br.read_bits(4).unwrap(), Some(0b1010));
    }

    #[test]
    fn peek_then_consume() {
        let data = vec![0b1101_1010];
        let mut br = BitReader::new(Cursor::new(data));
        // Peek 4 bits
        assert_eq!(br.peek_bits(4).unwrap(), Some(0b1101));
        // Consume without reading
        br.consume_bits(4);
        // Now read the rest
        assert_eq!(br.read_bits(4).unwrap(), Some(0b1010));
    }

    #[test]
    fn peek_across_byte_boundary() {
        let data = vec![0b0000_1111, 0b1010_0000];
        let mut br = BitReader::new(Cursor::new(data));
        // Read 4 bits
        assert_eq!(br.read_bits(4).unwrap(), Some(0b0000));
        // Peek 8 bits (crosses byte boundary)
        assert_eq!(br.peek_bits(8).unwrap(), Some(0b1111_1010));
        // Consume 4
        br.consume_bits(4);
        // Read remaining 4
        assert_eq!(br.read_bits(4).unwrap(), Some(0b1010));
    }

    #[test]
    fn peek_bits_large() {
        let data = vec![0xAB, 0xCD, 0xEF];
        let mut br = BitReader::new(Cursor::new(data));
        // Peek 20 bits (spanning all 3 bytes)
        assert_eq!(br.peek_bits(20).unwrap(), Some(0xABCDE));
        assert_eq!(br.read_bits(20).unwrap(), Some(0xABCDE));
        // 4 bits remaining (0xF)
        assert_eq!(br.read_bits(4).unwrap(), Some(0xF));
    }

    #[test]
    fn peek_bits_all_exactly() {
        let data = vec![0b1010_1010];
        let mut br = BitReader::new(Cursor::new(data));
        assert_eq!(br.peek_bits(8).unwrap(), Some(0b1010_1010));
        br.consume_bits(8);
        assert_eq!(br.read_bit().unwrap(), None);
    }

    // ---- BitReader batched I/O tests ----

    #[test]
    fn read_bits_batched_aligned() {
        let data = vec![0xAB, 0xCD];
        let mut br = BitReader::new(Cursor::new(data));
        assert_eq!(br.read_bits(16).unwrap(), Some(0xABCD));
    }

    #[test]
    fn read_bits_batched_small_first() {
        let data = vec![0xAB, 0xCD];
        let mut br = BitReader::new(Cursor::new(data));
        // First read some bits non-aligned
        assert_eq!(br.read_bits(4).unwrap(), Some(0xA));
        // Then read full bytes -- should use fast path since bits_remaining == 0
        assert_eq!(br.read_bits(12).unwrap(), Some(0xBCD));
    }

    #[test]
    fn read_bits_batched_large_then_small() {
        let data = vec![0x12, 0x34, 0x56, 0x78];
        let mut br = BitReader::new(Cursor::new(data));
        assert_eq!(br.read_bits(24).unwrap(), Some(0x123456));
        assert_eq!(br.read_bits(8).unwrap(), Some(0x78));
    }
}
