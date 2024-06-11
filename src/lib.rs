pub mod decoder;
pub mod encoder;
pub mod utils;

use std::collections::{BTreeMap, BinaryHeap, HashMap};

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

pub fn generate_codes(
    node: HuffNode,
    lookup_table: &mut HashMap<char, Vec<u8>>,
    code: String,
) -> Result<()> {
    if let Some(c) = node.character {
        // Encoding carries the length of the encoding and the code itself
        let encoding_len = code.chars().count() as u8;
        let bit_string = string_to_bytes(code)?;

        let mut encoded_data = vec![encoding_len];
        encoded_data.extend(bit_string);

        lookup_table.insert(c, encoded_data);
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
