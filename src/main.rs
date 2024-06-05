use std::cell::RefCell;
use std::char;
use std::cmp::Ordering;
use std::env::args;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::Path;
use std::process::exit;
use std::collections::{BinaryHeap, BTreeMap, HashMap};
use std::rc::Rc;

#[derive(Debug)]
struct HuffNode {
    weight: u32,
    character: Option<char>,
    left: Option<Rc<RefCell<HuffNode>>>,
    right: Option<Rc<RefCell<HuffNode>>>,
}

impl Ord for HuffNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.weight.cmp(&self.weight)
    }
}

impl PartialOrd for HuffNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
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
    fn new_leaf(weight: u32, character: char) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(HuffNode {
            weight,
            character: Some(character),
            left: None,
            right: None,
        }))
    }

    fn new_internal(weight: u32, left: Rc<RefCell<HuffNode>>, right: Rc<RefCell<HuffNode>>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(HuffNode {
            weight,
            character: None,
            left: Some(left),
            right: Some(right)
        }))
    }
}

fn build_min_heap(char_count: BTreeMap<char, u32>) -> BinaryHeap<Rc<RefCell<HuffNode>>> {
    let mut heap = BinaryHeap::new();
    for (char, weight) in &char_count {
        heap.push(HuffNode::new_leaf(*weight, *char))
    }

    heap
}

fn build_huffman_tree(heap: &mut BinaryHeap<Rc<RefCell<HuffNode>>>) -> Rc<RefCell<HuffNode>> {
    while heap.len() > 1 {
        let left = heap.pop().unwrap();
        let right = heap.pop().unwrap();

        let combined_weight = left.borrow().weight + right.borrow().weight;
        let new_node = HuffNode::new_internal(combined_weight, left, right);

        heap.push(new_node);
    }

    heap.pop().unwrap()
}

fn generate_codes(node: Rc<RefCell<HuffNode>>, prefix: String, codes: &mut HashMap<char, String>) {
    let node_borrowed = node.borrow();

    if let Some(character) = node_borrowed.character {
        // Leaf Node
        codes.insert(character, prefix);
    } else {
        // Internal Node
        if let Some(ref left) = node_borrowed.left {
            generate_codes(left.clone(), format!("{}0", prefix), codes);
        }
        if let Some(ref right) = node_borrowed.right {
            generate_codes(right.clone(), format!("{}1", prefix), codes);
        }
    }
}

fn get_huffman_codes(tree: Rc<RefCell<HuffNode>>) -> HashMap<char, String> {
    let mut codes = HashMap::new();
    generate_codes(tree, String::new(), &mut codes);

    codes
}

fn encode_text(text: &str, codes: &HashMap<char, String>) -> String {
    let mut encoded = String::new();
    for c in text.chars() {
        if let Some(code) = codes.get(&c) {
            encoded.push_str(code);
        }
    }

    encoded
}

fn string_to_bytes(binary_string: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut current_byte = 0u8;
    let mut bit_count = 0;

    for bit in binary_string.chars() {
        if bit == '1' {
            current_byte |= 1 << (7 - bit_count);
        }

        bit_count += 1;

        if bit_count == 8 {
            bytes.push(current_byte);
            current_byte = 0;
            bit_count = 0;
        }
    }

    if bit_count > 0 {
        bytes.push(current_byte);
    }

    bytes
}

fn main() -> io::Result<()> {
    let mut args = args();

    if args.len() != 2 {
        eprintln!("Incorrect number of arguments");
        exit(1);
    }

    let path_string = args.nth(1).expect("Could not find file");
    let file_path = Path::new(&path_string);
    let mut file = File::open(file_path).expect("Could not open file");
    let mut text = String::new();

    file.read_to_string(&mut text).unwrap();

    let mut char_count = BTreeMap::<char, u32>::new();

    for line in text.lines() {
        for c in line.chars() {
            *char_count.entry(c).or_insert(0) += 1;
        }
    }

    let mut heap = build_min_heap(char_count);
    let huffman_tree = build_huffman_tree(&mut heap);

    let codes = get_huffman_codes(huffman_tree);
    let encoded_text = encode_text(&text, &codes);

    println!("Huffman Codes: {:?}", codes);
    println!("ENCODED: {}", encoded_text);

    let bin_data = string_to_bytes(&encoded_text);

    let out_path = Path::new("out.cmpr");
    let mut out_file = File::create(out_path).expect("Could not create file");
    out_file.write_all(&bin_data).expect("Could not write file");

    Ok(())
}
