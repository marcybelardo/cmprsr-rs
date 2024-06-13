use std::collections::BTreeMap;

use crate::canonical::Canonical;

/// Handle encoding of data from text to binary
pub struct Encoder {
    text: String,
    pub char_map: BTreeMap<char, u32>,
}

impl Encoder {
    pub fn new(text: String) -> Self {
        Self {
            text,
            char_map: BTreeMap::<char, u32>::new(),
        }
    }

    pub fn count_chars(&mut self) {
        for line in self.text.lines() {
            for c in line.chars() {
                *self.char_map.entry(c).or_insert(0) += 1;
            }
        }
    }

    pub fn write_header_data(&self, canon: Canonical) -> Vec<u8> {
        let header = Vec::<u8>::new();
        let mut i = 0;

        while i < canon.canon_codes.len() {
            let mut bit_counter = 0;
            let mut current = 0u8;
        }
    }
}

//pub fn encode(&self) -> Result<Vec<u8>> {
//    let mut code = Vec::<u8>::new();
//
//    for c in self.text.chars() {
//        if let Some(bits) = self.lookup_table.get(&c) {
//            let bits_len = bits.len() as u8;
//            code.push(bits_len);
//            code.extend(bits);
//        }
//    }
//
//    Ok(code)
//}
//
//fn append_header_data(bytes: &mut Vec<u8>, character: u32, encoding: &[u8]) {
//    bytes.extend_from_slice(&character.to_le_bytes());
//    let encoding_len = encoding.len() as u8;
//    bytes.push(encoding_len);
//    bytes.extend(encoding);
//}
//}
