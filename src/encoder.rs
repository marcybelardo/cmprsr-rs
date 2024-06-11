use std::collections::{
    BTreeMap,
    HashMap
};

use anyhow::Result;

use crate::utils::string_to_bytes;

pub const START_PATTERN: [u8; 2] = [0xAB, 0xCD];
pub const END_PATTERN: [u8; 2] = [0xEF, 01];

pub struct Encoder {
    text: String,
    pub char_map: BTreeMap<char, u32>,
    pub lookup_table: HashMap<Vec<u8>, char>
}

impl Encoder {
    pub fn new(text: String) -> Self {
        Self {
            text,
            char_map: BTreeMap::<char, u32>::new(),
            lookup_table: HashMap::<Vec<u8>, char>::new() 
        }
    }

    pub fn count_chars(&mut self) {
        for line in self.text.lines() {
            for c in line.chars() {
                *self.char_map.entry(c).or_insert(0) += 1;
            }
        }
    }

    pub fn write_header_data(&self) -> Vec<u8> {
        let mut bytes = Vec::<u8>::new();

        bytes.extend_from_slice(&START_PATTERN);

        for (encoding, c) in self.lookup_table.iter() {
            Encoder::append_header_data(&mut bytes, *c as u32, encoding);
        }

        bytes.extend_from_slice(&END_PATTERN);

        bytes
    }

    pub fn encode(&self) -> Result<String> {
        let mut code = String::new();

        for c in self.text.chars() {
        }

        Ok(code) 
    }

    fn append_header_data(bytes: &mut Vec<u8>, character: u32, encoding: &[u8]) {
        bytes.extend_from_slice(&character.to_le_bytes());
        
        let encoding_len = encoding.len() as u8;
        bytes.push(encoding_len);

        bytes.extend_from_slice(encoding);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::string_to_bytes;

    #[test]
    fn test_count_chars() {
        let text = "hello world";
        let mut encoder = Encoder::new(text.to_string());

        encoder.count_chars();

        let expected_counts = vec![
            ('h', 1), ('e', 1), ('l', 3), ('o', 2),
            (' ', 1), ('w', 1), ('r', 1), ('d', 1),
        ].into_iter().collect();

        assert_eq!(encoder.char_map, expected_counts);
    }

    #[test]
    fn test_write_header_data() {
        let text = "hello";
        let mut encoder = Encoder::new(text.to_string());

        // Simulate a lookup table
        encoder.lookup_table.insert(string_to_bytes("00".to_owned()).unwrap(), 'h');
        encoder.lookup_table.insert(string_to_bytes("01".to_owned()).unwrap(), 'e');
        encoder.lookup_table.insert(string_to_bytes("10".to_owned()).unwrap(), 'l');
        encoder.lookup_table.insert(string_to_bytes("11".to_owned()).unwrap(), 'o');

        let header_data = encoder.write_header_data();

        let mut expected_bytes = vec![];
        expected_bytes.extend_from_slice(&START_PATTERN);
        Encoder::append_header_data(&mut expected_bytes, 'h' as u32, &string_to_bytes("00".to_owned()).unwrap());
        Encoder::append_header_data(&mut expected_bytes, 'e' as u32, &string_to_bytes("01".to_owned()).unwrap());
        Encoder::append_header_data(&mut expected_bytes, 'l' as u32, &string_to_bytes("10".to_owned()).unwrap());
        Encoder::append_header_data(&mut expected_bytes, 'o' as u32, &string_to_bytes("11".to_owned()).unwrap());
        expected_bytes.extend_from_slice(&END_PATTERN);

        assert_eq!(header_data, expected_bytes);
    }

    #[test]
    fn test_append_header_data() {
        let mut bytes = vec![];
        Encoder::append_header_data(&mut bytes, 'a' as u32, &string_to_bytes("01".to_owned()).unwrap());

        let expected_bytes = vec![
            'a' as u32 as u8,
            ('a' as u32 >> 8) as u8,
            ('a' as u32 >> 16) as u8,
            ('a' as u32 >> 24) as u8,
            2, // Length of the encoding
            0b00000001,
        ];

        assert_eq!(bytes, expected_bytes);
    }
}
