pub mod decoder;
pub mod encoder;
pub mod huffman;
pub mod utils;

use crate::encoder::Code;
use crate::huffman::HuffNode;

use anyhow::Result;

pub fn generate_base_codes(node: HuffNode, base_codes: &mut Vec<Code>, code: String) -> Result<()> {
    if let Some(c) = node.character {
        base_codes.push(Code { code, c });
    } else {
        if let Some(l) = node.left {
            generate_base_codes(*l, base_codes, code.clone() + "0")?;
        }
        if let Some(r) = node.right {
            generate_base_codes(*r, base_codes, code.clone() + "1")?;
        }
    }

    Ok(())
}

pub fn generate_canonical_codes(base_codes: Vec<Code>) -> Vec<Code> {
    Vec::<Code>::new()
}
