use std::collections::{BTreeMap, HashMap};

use anyhow::Result;

use crate::utils::bytes_to_string;

pub const START_PATTERN: [u8; 2] = [0xAB, 0xCD];
pub const END_PATTERN: [u8; 2] = [0xEF, 01];

pub struct Encoder {
    text: String,
    pub char_map: BTreeMap<char, u32>,
    pub lookup_table: HashMap<char, Vec<u8>>,
}

impl Encoder {
    pub fn new(text: String) -> Self {
        Self {
            text,
            char_map: BTreeMap::<char, u32>::new(),
            lookup_table: HashMap::<char, Vec<u8>>::new(),
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

        for (c, encoding) in self.lookup_table.iter() {
            Encoder::append_header_data(&mut bytes, *c as u32, encoding);
        }

        bytes.extend_from_slice(&END_PATTERN);

        bytes
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut code = Vec::<u8>::new();

        for c in self.text.chars() {
            if let Some(bits) = self.lookup_table.get(&c) {
                let bits_len = bits.len() as u8;
                code.push(bits_len);
                code.extend(bits);
            }
        }

        Ok(code)
    }

    fn append_header_data(bytes: &mut Vec<u8>, character: u32, encoding: &[u8]) {
        bytes.extend_from_slice(&character.to_le_bytes());
        let encoding_len = encoding.len() as u8;
        bytes.push(encoding_len);
        bytes.extend(encoding);
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
            ('h', 1),
            ('e', 1),
            ('l', 3),
            ('o', 2),
            (' ', 1),
            ('w', 1),
            ('r', 1),
            ('d', 1),
        ]
        .into_iter()
        .collect();

        assert_eq!(encoder.char_map, expected_counts);
    }

    #[test]
    fn test_write_header_data() {
        let text = "hello";
        let mut encoder = Encoder::new(text.to_string());

        // Simulate a lookup table
        encoder
            .lookup_table
            .insert('h', string_to_bytes("00".to_owned()).unwrap());
        encoder
            .lookup_table
            .insert('e', string_to_bytes("01".to_owned()).unwrap());
        encoder
            .lookup_table
            .insert('l', string_to_bytes("10".to_owned()).unwrap());
        encoder
            .lookup_table
            .insert('o', string_to_bytes("11".to_owned()).unwrap());

        let header_data = encoder.write_header_data();

        let mut i = 0;

        while i < header_data.len() {
            if i == 0 {
                assert_eq!(header_data[i..=i + 1], START_PATTERN);
                i += 2;
            }

            let current_char = u32::from_le_bytes([
                header_data[i],
                header_data[i + 1],
                header_data[i + 2],
                header_data[i + 3],
            ]);

            if let Some(encode) = encoder
                .lookup_table
                .get(&char::from_u32(current_char).unwrap())
            {
                match char::from_u32(current_char).unwrap() {
                    'h' => assert_eq!(*encode, string_to_bytes("00".to_owned()).unwrap()),
                    'e' => assert_eq!(*encode, string_to_bytes("01".to_owned()).unwrap()),
                    'l' => assert_eq!(*encode, string_to_bytes("10".to_owned()).unwrap()),
                    'o' => assert_eq!(*encode, string_to_bytes("11".to_owned()).unwrap()),
                    _ => panic!("no char found!"),
                }
            }
            i += 4;
            i += 1;
            i += 1;

            if i == header_data.len() - 2 {
                assert_eq!(
                    header_data[header_data.len() - 2..=header_data.len() - 1],
                    END_PATTERN
                );

                break;
            }
        }
    }

    #[test]
    fn test_append_header_data() {
        let mut bytes = vec![];
        Encoder::append_header_data(
            &mut bytes,
            'a' as u32,
            &string_to_bytes("1".to_owned()).unwrap(),
        );

        let expected_bytes = vec![
            'a' as u32 as u8,
            ('a' as u32 >> 8) as u8,
            ('a' as u32 >> 16) as u8,
            ('a' as u32 >> 24) as u8,
            1, // Length of the encoding
            0b0000_0001,
        ];

        assert_eq!(bytes, expected_bytes);
    }
}
