pub mod utils;
pub mod encoder;
pub mod decoder;

use std::collections::{
    BinaryHeap,
    BTreeMap,
    HashMap,
};

use anyhow::Result;
use utils::string_to_bytes;

#[derive(Debug)]
pub struct HuffNode {
    character: Option<char>,
    weight: u32,
    left: Option<Box<HuffNode>>,
    right: Option<Box<HuffNode>>,
}

impl Ord for HuffNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.weight.cmp(&self.weight)
    }
}

impl PartialOrd for HuffNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for HuffNode {
    fn eq(&self, other: &Self) -> bool {
        self.weight == other.weight
    }
}

impl Eq for HuffNode {}

impl HuffNode {
    fn new_leaf(character: char, weight: u32) -> Self {
        Self {
            character: Some(character),
            weight,
            left: None,
            right: None,
        }
    }

    fn new_internal(weight: u32, left: HuffNode, right: HuffNode) -> Self {
        Self {
            character: None,
            weight,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        }
    }
}

pub fn build_huffman_tree(char_map: &mut BTreeMap<char, u32>) -> Option<Box<HuffNode>> {
    let mut heap = BinaryHeap::new();

    for (c, w) in char_map.iter() {
        heap.push(HuffNode::new_leaf(*c, *w));
    }

    while heap.len() > 1 {
        let left = heap.pop().unwrap();
        let right = heap.pop().unwrap();

        let combined_weight = left.weight + right.weight;
        let new_node = HuffNode::new_internal(combined_weight, left, right);

        heap.push(new_node);
    }

    if let Some(node) = heap.pop() {
        Some(Box::new(node))
    } else {
        None
    }
}

pub fn generate_codes(node: HuffNode, lookup_table: &mut HashMap<Vec<u8>, char>, code: String) -> Result<()> {
    if let Some(c) = node.character {
        // Encoding carries the length of the encoding and the code itself
        let bit_string_len = stringify!(code.chars().count().to_le_bytes());
        let bit_string = stringify!(code);
        lookup_table.insert(string_to_bytes(bit_string_len.to_owned() + bit_string)?, c);
    } else {
        if let Some(l) = node.left {
            generate_codes(*l, lookup_table, code.clone() + "0")?;
        }
        if let Some(r) = node.right {
            generate_codes(*r, lookup_table, code.clone() + "1")?;
        }
    }

    Ok(())
}


