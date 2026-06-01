# AGENTS.md -- Implementation Plan

This file captures the architectural plan for the `cmprsr` Huffman compressor, so that any agent or developer picking up the project understands the design decisions and intended structure.

## CLI Interface

| Command | Action |
|---|---|
| `cmprsr <file>` | Compress `<file>` to `<file>.cmpr` |
| `cmprsr -d <input.cmpr> <output>` | Decompress `<input.cmpr>` to `<output>` |
| `cmprsr --help` | Print usage |
| `cmprsr --version` | Print version |

CLI parsing is handled by `argh` (the single external dependency).

## Module Layout

```
src/
  main.rs       -- CLI entry point, dispatch to compress/decompress
  frequency.rs  -- Single-pass byte-frequency histogram -> [u64; 256]
  huffman.rs    -- Huffman tree construction, canonical code generation
  bitio.rs      -- Bit-level reader/writer wrappers over BufRead/BufWrite
  format.rs     -- .cmpr binary format constants and header read/write
  compress.rs   -- Compression orchestration
  decompress.rs -- Decompression orchestration
```

## Canonical Huffman Encoding

Standard Huffman produces variable-length codes. Canonical Huffman normalises them:

1. Build a standard Huffman tree via a priority queue (binary heap) of nodes.
2. Compute the bit length of each symbol's code (tree depth).
3. Sort symbols by `(bit_length, byte_value)` ascending.
4. Assign canonical codes using the standard formula:

```
next_code[1] = 0
for len in 2..=MAX_BITS:
    next_code[len] = (next_code[len-1] + count[len-1]) << 1
```

Symbols at each length receive codes sequentially from `next_code[len]`.

The decoder only needs each symbol's bit length (not the full tree) to reconstruct the same codes.

## Edge-Case Policy

| Condition | Behaviour |
|---|---|
| Single unique byte | Compresses normally; header overhead may exceed input size (acceptable) |
| Empty file | Produces valid .cmpr with header only; decompresses to empty output |
| Code length > 32 bits | Reject with a clear error message |
| Output file exists | Overwrite with a warning printed to stderr |
| Corrupt .cmpr (bad magic, version, bounds) | Validate header fields; panic with descriptive `Err` |
| Very large files | Stream via `BufReader`/`BufWriter`; never load entire file into memory |
| Decoding performance | Start with a simple sequential bit-at-a-time decoder; optimise later if needed |

## .cmpr Binary Format

```
Offset  Size  Field
------  ----  ----------------------------------------
  0       4   Magic bytes        "CM PR"  (0x43 0x4D 0x50 0x52)
  4       1   Version            0x01
  5       8   Original size      little-endian u64
 13       1   Padding bits       0..7 (unused bits in final byte)
 14       2   Symbol count       little-endian u16 (N)
 16      N*2  Symbol table       N entries of (u8 symbol, u8 code_len)
 16+N*2  ...  Compressed data    Bitstream padded to byte boundary
```

The symbol table lists only bytes with non-zero frequency, sorted by byte value.

## Implementation Order

1. `frequency.rs` -- one pass over input bytes, produce `[u64; 256]`
2. `huffman.rs` -- build tree, compute lengths, generate canonical codes
3. `bitio.rs` -- `BitReader<R>` and `BitWriter<W>` structs with bit-level I/O
4. `format.rs` -- header constants, serialization, deserialization, validation
5. `compress.rs` -- orchestrate: count -> build codes -> write header -> encode
6. `decompress.rs` -- orchestrate: read header -> rebuild codes -> decode -> write
7. `main.rs` -- CLI parsing with `argh`, dispatch
