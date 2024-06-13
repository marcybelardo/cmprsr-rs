use std::collections::BTreeMap;

pub struct Encoder {
    text: String,
    pub char_map: BTreeMap<char, u32>,
    pub base_codes: Vec<Code>,
    pub canon_codes: Vec<Code>,
}

#[derive(Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct Code {
    pub code: String,
    pub c: char,
}

impl Encoder {
    pub fn new(text: String) -> Self {
        Self {
            text,
            char_map: BTreeMap::<char, u32>::new(),
            base_codes: Vec::<Code>::new(),
            canon_codes: Vec::<Code>::new(),
        }
    }

    pub fn count_chars(&mut self) {
        for line in self.text.lines() {
            for c in line.chars() {
                *self.char_map.entry(c).or_insert(0) += 1;
            }
        }
    }

    pub fn sort_base(&mut self) {
        self.base_codes.sort()
    }

    //pub fn write_header_data(&self) -> Vec<u8> {
    //    let mut bytes = Vec::<u8>::new();
    //
    //    bytes.extend_from_slice(&START_PATTERN);
    //
    //    for (c, encoding) in self.lookup_table.iter() {
    //        Encoder::append_header_data(&mut bytes, *c as u32, encoding);
    //    }
    //
    //    bytes.extend_from_slice(&END_PATTERN);
    //
    //    bytes
    //}
    //
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
}
