use std::io::{BufReader, Read};

/// Counts byte frequencies from any `Read` source.
///
/// Reads the input in buffered chunks and returns a 256-element array
/// where `result[b]` is the number of times byte value `b` appeared.
pub fn count_frequencies(reader: &mut dyn Read) -> std::io::Result<[u64; 256]> {
    let mut freqs = [0u64; 256];
    let mut reader = BufReader::new(reader);
    let mut buf = [0u8; 4096];

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        for &byte in &buf[..n] {
            freqs[byte as usize] += 1;
        }
    }

    Ok(freqs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        let mut empty = std::io::empty();
        let freqs = count_frequencies(&mut empty).unwrap();
        assert!(freqs.iter().all(|&count| count == 0));
    }

    #[test]
    fn single_byte_repeated() {
        let data = vec![0xAB; 100];
        let freqs = count_frequencies(&mut &data[..]).unwrap();
        assert_eq!(freqs[0xAB], 100);
        // All other bins should be zero.
        for b in 0..=255u16 {
            if b != 0xAB {
                assert_eq!(freqs[b as usize], 0);
            }
        }
    }

    #[test]
    fn all_bytes_once() {
        let data: Vec<u8> = (0..=255).collect();
        let freqs = count_frequencies(&mut &data[..]).unwrap();
        for b in 0..=255usize {
            assert_eq!(freqs[b], 1);
        }
    }

    #[test]
    fn varying_counts() {
        let mut data = Vec::new();
        // 200 times 0x00, 150 times 0x01, 100 times 0x02
        data.extend(std::iter::repeat(0x00).take(200));
        data.extend(std::iter::repeat(0x01).take(150));
        data.extend(std::iter::repeat(0x02).take(100));
        let freqs = count_frequencies(&mut &data[..]).unwrap();
        assert_eq!(freqs[0x00], 200);
        assert_eq!(freqs[0x01], 150);
        assert_eq!(freqs[0x02], 100);
    }
}
