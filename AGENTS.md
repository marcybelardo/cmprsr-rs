# AGENTS.md — Architectural Plan

This file captures the architectural plan for the `cmprsr` canonical Huffman
compressor, so that any agent or developer picking up the project understands
the design decisions and intended structure.

## CLI Interface

| Command | Action |
|---|---|
| `cmprsr <file>` | Compress `<file>` to `<file>.cmpr` |
| `cmprsr -d <input.cmpr> <output>` | Decompress `<input.cmpr>` to `<output>` |
| `cmprsr -l <file.cmpr>` | List information (header only) |
| `cmprsr -c <file>` | Write compressed data to stdout |
| `cmprsr -d -c <input.cmpr>` | Decompress to stdout |
| `cmprsr -f <file>` | Force overwrite without warning |
| `cmprsr --help` | Print usage |
| `cmprsr --version` | Print version |

CLI parsing is handled by `argh` (the single external dependency).

## Module Layout

```
src/
  lib.rs        -- Crate root, re-exports all modules
  main.rs       -- CLI entry point, dispatch to compress/decompress/list
  frequency.rs  -- Single-pass byte-frequency histogram -> [u64; 256]
  huffman.rs    -- Huffman tree construction, canonical code generation
  bitio.rs      -- Bit-level reader/writer with peek/consume and batched I/O
  format.rs     -- .cmpr binary format constants, header read/write, CRC-32
  compress.rs   -- Compression orchestration
  decompress.rs -- Decompression orchestration with LUT-based decode

fuzz/
  Cargo.toml                        -- Fuzz target configuration (needs nightly)
  fuzz_targets/roundtrip.rs          -- Round-trip fuzz target
  fuzz_targets/decompress_safety.rs  -- Decompress-without-panicking fuzz target
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

The decoder only needs each symbol's bit length (not the full tree) to
reconstruct the same codes.

## Edge-Case Policy

| Condition | Behaviour |
|---|---|
| Single unique byte | Compresses normally; header overhead may exceed input size (acceptable) |
| Empty file | Produces valid .cmpr with header only; decompresses to empty output |
| Code length > 32 bits | Reject with `InvalidData` error |
| Output file exists with no `-f` | Print warning to stderr |
| Corrupt .cmpr (bad magic, version, bounds, CRC) | Validate fields; return `InvalidData` error |
| Very large files | Stream via `BufReader`/`BufWriter`; never load entire file into memory |
| Decoding performance | Prefix lookup table (11-bit, 2048 entries) for O(1) decode |

## .cmpr Binary Format (v0x02)

Since format version **0x02**, a CRC-32 trailer is appended after the compressed
bitstream for integrity verification.  Files written by older compressors
(version 0x01) are still readable — the decoder detects the version and skips
CRC verification for v0x01 files.

```
Offset  Size  Field
------  ----  ----------------------------------------
  0       4   Magic bytes        "CMPR"  (0x43 0x4D 0x50 0x52)
  4       1   Version            0x02
  5       8   Original size      little-endian u64
 13       1   Padding bits       0..7 (unused bits in final byte)
 14       2   Symbol count       little-endian u16 (N)
 16      N*2  Symbol table       N entries of (u8 symbol, u8 code_len)
 16+N*2  ...  Compressed data    Bitstream padded to byte boundary
 EOF-4    4   CRC-32             little-endian u32 of compressed data
```

The symbol table lists only bytes with non-zero frequency, sorted by byte value.

The CRC-32 uses the Ethernet/ISO-HDLC polynomial `0xEDB88320` and covers every
byte from the start of the compressed data through the last padded byte
(excluding the CRC trailer itself).

## Performance Features

1. **Prefix lookup table (LUT):** An 11-bit (2048-entry) table maps the first
   11 bits of any code to `(symbol, code_length)` for codes ≤ 11 bits.
   Lookup is O(1); fallback to linear scan only for codes longer than 11 bits.

2. **Batched bit I/O:** `BitWriter::write_bits` writes full bytes directly when
   the internal buffer is aligned.  `BitReader::read_bits` reads full bytes
   directly from the underlying reader when no buffered bits remain.

## Implementation Order

1. `frequency.rs` — one pass over input bytes, produce `[u64; 256]`
2. `huffman.rs` — build tree, compute lengths, generate canonical codes
3. `bitio.rs` — `BitReader<R>` and `BitWriter<W>` structs with bit-level I/O
4. `format.rs` — header constants, serialization, deserialization, validation, CRC-32
5. `compress.rs` — orchestrate: count → build codes → write header → encode → CRC
6. `decompress.rs` — orchestrate: read header → rebuild codes → decode → verify CRC
7. `main.rs` — CLI parsing with `argh`, dispatch
