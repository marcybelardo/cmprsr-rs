use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Maximum allowed Huffman code length in bits.
const MAX_CODE_LENGTH: u8 = 32;

/// Result of canonical code construction for all 256 byte values.
pub struct CodeTable {
    /// Canonical code bit pattern for each byte (right-aligned in the u64).
    pub code: [u64; 256],
    /// Code length in bits for each byte.  0 means the byte never appears.
    pub code_len: [u8; 256],
    /// Number of distinct symbols appearing in the input.
    #[allow(dead_code)]
    pub symbol_count: u16,
}

// ---------------------------------------------------------------------------
// Internal tree representation (arena-allocated binary tree)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct Node {
    freq: u64,
    byte: Option<u8>,       // Some(byte) for leaves, None for internal nodes
    left: Option<usize>,
    right: Option<usize>,
}

struct HuffTree {
    nodes: Vec<Node>,
    root: usize,
}

// ---------------------------------------------------------------------------
// Tree construction from a frequency table
// ---------------------------------------------------------------------------

/// Builds a Huffman tree from the 256-entry frequency array.
///
/// Leaf nodes are created for every byte with non-zero frequency and pushed
/// into a min-heap ordered by frequency.  The two smallest nodes are repeatedly
/// merged into a parent node until one node remains -- the root.
fn build_tree(freqs: &[u64; 256]) -> HuffTree {
    let mut nodes: Vec<Node> = Vec::new();
    let mut heap: BinaryHeap<Reverse<(u64, usize)>> = BinaryHeap::new();

    // Create leaf nodes.
    for byte in 0..=255u16 {
        let freq = freqs[byte as usize];
        if freq > 0 {
            let idx = nodes.len();
            nodes.push(Node {
                freq,
                byte: Some(byte as u8),
                left: None,
                right: None,
            });
            heap.push(Reverse((freq, idx)));
        }
    }

    // Empty input -- return a tree with a single dummy node.
    if heap.is_empty() {
        nodes.push(Node {
            freq: 0,
            byte: None,
            left: None,
            right: None,
        });
        return HuffTree { nodes, root: 0 };
    }

    // Repeatedly merge the two smallest nodes.
    while heap.len() > 1 {
        let Reverse((freq_a, idx_a)) = heap.pop().unwrap();
        let Reverse((freq_b, idx_b)) = heap.pop().unwrap();

        let parent_idx = nodes.len();
        nodes.push(Node {
            freq: freq_a + freq_b,
            byte: None,
            left: Some(idx_a),
            right: Some(idx_b),
        });
        heap.push(Reverse((freq_a + freq_b, parent_idx)));
    }

    let Reverse((_, root)) = heap.pop().unwrap();
    HuffTree { nodes, root }
}

// ---------------------------------------------------------------------------
// Extract code lengths from a finished tree
// ---------------------------------------------------------------------------

/// Walks the tree and records the depth (code length) of every leaf byte.
fn compute_lengths(tree: &HuffTree) -> [u8; 256] {
    let mut lengths = [0u8; 256];
    let mut stack = vec![(tree.root, 0u8)];

    while let Some((node_idx, depth)) = stack.pop() {
        let node = &tree.nodes[node_idx];
        if let Some(byte) = node.byte {
            lengths[byte as usize] = depth;
        } else {
            // Push children -- right first so left is processed first (DFS order is
            // irrelevant for correctness, only code-length assignment matters).
            if let Some(right) = node.right {
                stack.push((right, depth + 1));
            }
            if let Some(left) = node.left {
                stack.push((left, depth + 1));
            }
        }
    }

    lengths
}

// ---------------------------------------------------------------------------
// Canonical code generation
// ---------------------------------------------------------------------------

/// Converts raw Huffman code lengths into canonical codes.
///
/// Canonical Huffman codes are normalised by:
///   1. Sorting symbols by (code_len, byte_value).
///   2. Assigning codes with the standard formula:
///        next_code[1] = 0
///        for len in 2..=MAX_BITS:
///            next_code[len] = (next_code[len-1] + count[len-1]) << 1
///
/// Returns (code_len_array, code_array, symbol_count).
fn canonicalize(code_len: &[u8; 256]) -> Result<([u8; 256], [u64; 256], u16), String> {
    let mut canonical_len = [0u8; 256];
    let mut canonical_code = [0u64; 256];
    let mut symbol_count = 0u16;

    // --- Count symbols at each length and reject excessive lengths ---
    let mut count_by_len = [0u16; MAX_CODE_LENGTH as usize + 1];
    for byte in 0..=255usize {
        let len = code_len[byte];
        if len > 0 {
            if len > MAX_CODE_LENGTH {
                return Err(format!(
                    "Code length {len} for byte 0x{byte:02X} exceeds maximum of {MAX_CODE_LENGTH}"
                ));
            }
            count_by_len[len as usize] += 1;
            symbol_count += 1;
        }
    }

    // --- Single-symbol edge case: force a 1-bit code ---
    if symbol_count == 1 {
        for byte in 0..=255usize {
            if code_len[byte] > 0 {
                canonical_len[byte] = 1;
                canonical_code[byte] = 0;
                break;
            }
        }
        return Ok((canonical_len, canonical_code, symbol_count));
    }

    // --- Compute starting code for each length ---
    let mut next_code = [0u64; MAX_CODE_LENGTH as usize + 2];
    let mut code = 0u64;
    for len in 1..=MAX_CODE_LENGTH as usize {
        code = (code + count_by_len[len - 1] as u64) << 1;
        next_code[len] = code;
    }

    // --- Collect and sort (code_len, byte) pairs ---
    let mut symbols: Vec<(u8, u8)> = Vec::with_capacity(symbol_count as usize);
    for byte in 0..=255usize {
        let len = code_len[byte];
        if len > 0 {
            symbols.push((len, byte as u8));
        }
    }
    symbols.sort(); // Sorts by .0 first (code_len), then .1 (byte value)

    // --- Assign canonical codes in sorted order ---
    for (len, byte) in &symbols {
        let len = *len as usize;
        let b = *byte as usize;
        canonical_len[b] = len as u8;
        canonical_code[b] = next_code[len];
        next_code[len] += 1;
    }

    Ok((canonical_len, canonical_code, symbol_count))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build canonical Huffman codes from a full frequency table.
pub fn build_codes(freqs: &[u64; 256]) -> Result<CodeTable, String> {
    let tree = build_tree(freqs);
    let mut code_len = compute_lengths(&tree);

    // Single-symbol workaround: a tree with one leaf gives depth 0, but the
    // bit encoder/decoder needs at least 1 bit per code.
    let distinct_count = freqs.iter().filter(|&&f| f > 0).count();
    if distinct_count == 1 {
        for byte in 0..=255usize {
            if freqs[byte] > 0 {
                code_len[byte] = 1;
                break;
            }
        }
    }

    let (code_len, code, symbol_count) = canonicalize(&code_len)?;

    Ok(CodeTable {
        code,
        code_len,
        symbol_count,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_frequencies() {
        let table = build_codes(&[0u64; 256]).unwrap();
        assert_eq!(table.symbol_count, 0);
        assert!(table.code_len.iter().all(|&len| len == 0));
    }

    #[test]
    fn single_byte() {
        let mut freqs = [0u64; 256];
        freqs[0x41] = 100; // 'A' appears 100 times
        let table = build_codes(&freqs).unwrap();
        assert_eq!(table.symbol_count, 1);
        assert_eq!(table.code_len[0x41], 1); // forced to 1-bit code
        assert_eq!(table.code[0x41], 0);
    }

    #[test]
    fn two_bytes_equal_freq() {
        let mut freqs = [0u64; 256];
        freqs[0x00] = 50;
        freqs[0x01] = 50;
        let table = build_codes(&freqs).unwrap();
        assert_eq!(table.symbol_count, 2);
        // Both should have a 1-bit code.
        assert_eq!(table.code_len[0x00], 1);
        assert_eq!(table.code_len[0x01], 1);
        // Smaller byte gets 0, larger gets 1.
        assert_eq!(table.code[0x00], 0);
        assert_eq!(table.code[0x01], 1);
    }

    #[test]
    fn three_bytes_skewed() {
        let mut freqs = [0u64; 256];
        freqs[0x41] = 100; // A
        freqs[0x42] = 50;  // B
        freqs[0x43] = 30;  // C
        let table = build_codes(&freqs).unwrap();
        assert_eq!(table.symbol_count, 3);

        // Most frequent gets the shortest code (should be 1 bit).
        assert_eq!(table.code_len[0x41], 1);
        assert_eq!(table.code[0x41], 0);

        // Remaining two get 2-bit codes starting at 2 (binary 10).
        assert_eq!(table.code_len[0x42], 2);
        assert_eq!(table.code_len[0x43], 2);
        assert_eq!(table.code[0x42], 2); // 10
        assert_eq!(table.code[0x43], 3); // 11
    }

    #[test]
    fn codes_are_prefix_free() {
        let mut freqs = [0u64; 256];
        // Spread across many bytes so we get a varied tree.
        for i in 0..100u8 {
            freqs[i as usize] = (100 - i as u64) * 10;
        }
        let table = build_codes(&freqs).unwrap();

        // Check that no code is a prefix of another.
        // Canonical codes are written MSB-first and stored right-aligned
        // in the u64, so the prefix check compares high-order bits.
        for a in 0..=255usize {
            let len_a = table.code_len[a];
            if len_a == 0 {
                continue;
            }
            for b in 0..=255usize {
                if a == b || table.code_len[b] == 0 {
                    continue;
                }
                let len_b = table.code_len[b];
                if len_a <= len_b && len_a < len_b {
                    // Check if a's code (len_a bits) is a prefix of b's code.
                    // The top len_a bits of b must equal a's code.
                    let shifted_b = table.code[b] >> (len_b - len_a);
                    assert_ne!(
                        shifted_b, table.code[a],
                        "Byte {a:#04x} code is prefix of byte {b:#04x}"
                    );
                } else if len_b < len_a {
                    // Check if b's code (len_b bits) is a prefix of a's code.
                    let shifted_a = table.code[a] >> (len_a - len_b);
                    assert_ne!(
                        shifted_a, table.code[b],
                        "Byte {b:#04x} code is prefix of byte {a:#04x}"
                    );
                }
            }
        }
    }

    #[test]
    fn all_256_bytes() {
        let mut freqs = [0u64; 256];
        for i in 0..=255usize {
            freqs[i] = (i + 1) as u64; // increasing frequencies
        }
        let table = build_codes(&freqs).unwrap();
        assert_eq!(table.symbol_count, 256);
        // Every byte should have a code length >= 1.
        for byte in 0..=255usize {
            assert!(table.code_len[byte] > 0, "Byte {byte} has no code");
        }
    }

    #[test]
    fn zero_freq_byte_has_no_code() {
        let mut freqs = [0u64; 256];
        freqs[0x00] = 10;
        freqs[0x10] = 20;
        let table = build_codes(&freqs).unwrap();
        assert_eq!(table.code_len[0x05], 0); // never appeared
        assert_eq!(table.code_len[0x00], 1);
        assert_eq!(table.code_len[0x10], 1);
    }
}
