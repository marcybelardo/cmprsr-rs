use crate::encoder::{END_PATTERN, START_PATTERN};

use std::collections::HashMap;

use anyhow::{anyhow, Result};

#[derive(Debug)]
pub struct Decoder {
    header: Vec<(u32, Vec<u8>)>,
    data: Vec<u8>,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            header: Vec::new(),
            data: Vec::new(),
        }
    }

    pub fn parse_header_and_data(&mut self, raw_bytes: &[u8]) -> Result<()> {
        let mut i = 0;

        if raw_bytes[i..=i + 1].to_vec() != START_PATTERN {
            Err(anyhow!("Invalid file"))
        } else {
            i += 2;

            while i < raw_bytes.len() {
                let character = u32::from_le_bytes([
                    raw_bytes[i],
                    raw_bytes[i + 1],
                    raw_bytes[i + 2],
                    raw_bytes[i + 3],
                ]);
                i += 4;

                let mut encoding = Vec::new();

                let encoding_len = raw_bytes[i];
                // Push length of encoding first
                encoding.push(encoding_len);
                i += 1;

                encoding.extend(&raw_bytes[i..i + encoding_len as usize]);
                i += encoding_len as usize;

                self.header.push((character, encoding));

                if raw_bytes[i..=i + 1].to_vec() == END_PATTERN {
                    i += 2;
                    break;
                }
            }

            while i < raw_bytes.len() {
                self.data.push(raw_bytes[i]);
                i += 1;
            }

            Ok(())
        }
    }

    pub fn header_to_lookup_table(&self) -> HashMap<Vec<u8>, char> {
        let mut lookup_table = HashMap::<Vec<u8>, char>::new();

        for (char_bytes, encoding) in self.header.iter() {
            if let Some(c) = char::from_u32(*char_bytes) {
                let encoding_bytes = encoding[1..].to_vec();
                lookup_table.insert(encoding_bytes, c);
            }
        }

        lookup_table
    }

    pub fn decode(&self, lookup_table: HashMap<Vec<u8>, char>) -> Result<String> {
        let mut i = 0;
        let mut text = String::new();

        while i < self.data.len() {
            let encoding_len = self.data[i];
            i += 1;

            let encoding = &self.data[i..i + encoding_len as usize];
            i += encoding_len as usize;

            if let Some(&c) = lookup_table.get(encoding) {
                text.push(c);
            }
        }

        Ok(text)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // Helper function to create raw bytes for testing
    fn create_test_data() -> Vec<u8> {
        let mut raw_bytes: Vec<u8> = Vec::new();

        // Start pattern
        raw_bytes.extend_from_slice(&START_PATTERN);

        // Header: character 'A' (0x41), encoding length 2, encoding [0x01, 0x02]
        raw_bytes.extend_from_slice(&[0x41, 0x00, 0x00, 0x00, 2, 0x01, 0x02]);

        // Header: character 'B' (0x42), encoding length 2, encoding [0x03, 0x04]
        raw_bytes.extend_from_slice(&[0x42, 0x00, 0x00, 0x00, 2, 0x03, 0x04]);

        // End pattern
        raw_bytes.extend_from_slice(&END_PATTERN);

        // Data: encoding [0x01, 0x02] (for 'A') and [0x03, 0x04] (for 'B')
        raw_bytes.extend_from_slice(&[2, 0x01, 0x02, 2, 0x03, 0x04]);

        raw_bytes
    }

    #[test]
    fn test_parse_header_and_data() -> Result<()> {
        let raw_bytes = create_test_data();
        let mut decoder = Decoder::new();

        decoder.parse_header_and_data(&raw_bytes)?;

        // Check header
        assert_eq!(decoder.header.len(), 2);
        assert_eq!(decoder.header[0].0, 0x41);
        assert_eq!(decoder.header[0].1, vec![2, 0x01, 0x02]);
        assert_eq!(decoder.header[1].0, 0x42);
        assert_eq!(decoder.header[1].1, vec![2, 0x03, 0x04]);

        // Check data
        assert_eq!(decoder.data, vec![2, 0x01, 0x02, 2, 0x03, 0x04]);

        Ok(())
    }

    #[test]
    fn test_header_to_lookup_table() -> Result<()> {
        let raw_bytes = create_test_data();
        let mut decoder = Decoder::new();

        decoder.parse_header_and_data(&raw_bytes)?;
        let lookup_table = decoder.header_to_lookup_table();

        // Check lookup table
        assert_eq!(lookup_table.len(), 2);
        assert_eq!(lookup_table[&vec![0x01, 0x02]], 'A');
        assert_eq!(lookup_table[&vec![0x03, 0x04]], 'B');

        Ok(())
    }

    #[test]
    fn test_decode() -> Result<()> {
        let raw_bytes = create_test_data();
        let mut decoder = Decoder::new();

        decoder.parse_header_and_data(&raw_bytes)?;
        let lookup_table = decoder.header_to_lookup_table();
        let decoded_text = decoder.decode(lookup_table.clone())?;

        println!("{:?}", lookup_table);
        println!("{:?}", decoder);

        // Check decoded text
        assert_eq!(decoded_text, "AB");

        Ok(())
    }
}
