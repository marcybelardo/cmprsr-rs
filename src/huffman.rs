use std::collections::{BTreeMap, BinaryHeap, HashMap};

use anyhow::Result;

/// Nodes for the Huffman Tree
#[derive(Debug)]
pub struct HuffNode {
    pub character: Option<char>,
    weight: u32,
    pub left: Option<Box<HuffNode>>,
    pub right: Option<Box<HuffNode>>,
}

pub struct HuffTree {
    pub root: Box<HuffNode>,
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

impl HuffTree {
    pub fn build_huffman_tree(char_map: &mut BTreeMap<char, u32>) -> Option<HuffTree> {
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
            Some(HuffTree {
                root: Box::new(node),
            })
        } else {
            None
        }
    }
}

pub fn generate_base_codes(
    node: HuffNode,
    base_codes: &mut HashMap<char, String>,
    code: String,
) -> Result<()> {
    if let Some(c) = node.character {
        base_codes.insert(c, code);
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
