# cmprsr

A canonical Huffman compressor written in Rust. Compresses arbitrary files using Huffman coding with a minimal binary format and no external dependencies beyond the CLI parser.

## Usage

```
# Compress a file (produces <file>.cmpr)
cmprsr input.txt

# Decompress a .cmpr file
cmprsr -d input.cmpr output.txt

# Show help
cmprsr --help
```

## How It Works

1. **Frequency analysis** -- Single pass over the input counts how often each byte value (0--255) appears.
2. **Huffman tree** -- A binary-heap-based priority queue builds the optimal prefix-code tree.
3. **Canonical codes** -- Instead of storing the full tree, only each symbol's code length is written to the header. The decoder reconstructs identical codes deterministically.
4. **Streaming** -- Input and output are processed through buffered readers and writers. The entire file is never loaded into memory at once.

## File Format

See [AGENTS.md](AGENTS.md) for the full `.cmpr` binary format specification.

## Limits

- Code lengths are capped at 32 bits. Files requiring deeper trees are rejected with an error.
- The header adds a fixed overhead (about 20 bytes plus up to 512 bytes for the symbol table), so very small files may not compress.
