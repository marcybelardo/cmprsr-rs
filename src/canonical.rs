use std::collections::HashMap;

#[derive(Default)]
pub struct Canonical {
    pub base_codes: HashMap<char, String>,

    /// A list of Codes, that is, characters along with their canonical encoding
    pub canon_codes: HashMap<char, String>,

    /// List of the code lengths of the characters in the encoded object
    pub code_lens: Vec<usize>,

    /// List of the characters in the encoded object
    pub code_chars: Vec<char>,
}

impl Canonical {
    pub fn generate_canon_codes(&mut self) {
        for base in self.base_codes {
            self.code_lens.push(base.1.len());
            self.code_chars.push(base.0);
        }

        let mut symbols_lens: Vec<(usize, char)> = self
            .code_lens
            .into_iter()
            .zip(self.code_chars.into_iter())
            .collect();

        symbols_lens.sort();

        let mut code = 0;
        let mut prev_len = 0;

        for (len, sym) in symbols_lens {
            if len != prev_len {
                code <<= len - prev_len;
                prev_len = len;
            }

            let code_str = format!("{:0len$b}", code, len = len);

            self.canon_codes.insert(sym, code_str);

            code += 1;
        }
    }
}
