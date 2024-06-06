use std::{
    collections::{
        BinaryHeap,
        BTreeMap,
        HashMap,
    },
    env,
    fs::File,
    io::prelude::*,
    io::Result,
};

#[derive(Debug)]
struct HuffNode {
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

fn build_huffman_tree(char_map: &mut BTreeMap<char, u32>) -> Option<Box<HuffNode>> {
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

    heap.pop().map(Box::new)
}

fn generate_codes(node: HuffNode, lookup_table: &mut HashMap<char, String>, code: String) {
    if let Some(c) = node.character {
        lookup_table.insert(c, code);
    } else {
        if let Some(l) = node.left {
            generate_codes(*l, lookup_table, code.clone() + "0");
        }
        if let Some(r) = node.right {
            generate_codes(*r, lookup_table, code.clone() + "1");
        }
    }
}

fn encode(text: String, lookup: HashMap<char, String>) -> String {
    let mut code = String::new();

    for c in text.chars() {
        if let Some(bits) = lookup.get(&c) {
            code.push_str(bits);
        }
    }

    code
}

fn string_to_bytes(binary_string: String) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut current = 0u8;
    let mut bit_count = 0;

    for bit in binary_string.chars() {
        if bit == '1' {
            current |= 1 << (7 - bit_count);
        }

        bit_count += 1;

        if bit_count == 8 {
            bytes.push(current);
            current = 0;
            bit_count = 0;
        }
    }

    if bit_count > 0 {
        bytes.push(current);
    }

    bytes
}

fn write_bytes_to_file(filename: &str, bytes: &[u8]) -> Result<()> {
    let mut file = File::create(filename)?;
    file.write_all(bytes)?;
    Ok(())
}

fn main() -> Result<()> {
    let mut args = env::args();

    let filename = args.nth(1).unwrap();

    let mut file = File::open(filename)?;
    let mut text = String::new();

    file.read_to_string(&mut text).unwrap();

    let mut char_map = BTreeMap::<char, u32>::new();

    for line in text.lines() {
        for c in line.chars() {
            *char_map.entry(c).or_insert(0) += 1;
        }
    }

    let option_root = build_huffman_tree(&mut char_map);
    let mut lookup_table = HashMap::<char, String>::new();

    if let Some(root) = option_root {
        generate_codes(*root, &mut lookup_table, String::new());
    }

    let encoded = encode(text, lookup_table);

    let bin_data = string_to_bytes(encoded);
    write_bytes_to_file("out.cmpr", &bin_data)?;

    Ok(())
}
