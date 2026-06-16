# cmprsr — A Fast Canonical Huffman Compressor

`cmprsr` is a general-purpose compression tool based on canonical Huffman
coding.  It is designed for daily use — comparable in feel to `gzip`/`xz`
but simpler and faster for typical text and binary workloads.

## Features

- **Canonical Huffman encoding** — fast decode with a prefix lookup table
- **CRC-32 integrity checking** — detects data corruption automatically
- **Streaming I/O** — handles arbitrarily large files
- **Batched bit I/O** — aligned bytes bypass per-bit branching
- **Backward compatible** — reads files created by older v0x01 compressors
- **Minimal dependencies** — only `argh` for CLI parsing

## Installation

### Prerequisites

- Rust 1.70 or newer (for `const` fn support in CRC-32 table generation)

### Build from source

```bash
git clone <repo-url>
cd cmprsr-rs
cargo build --release
```

The binary is placed at `target/release/cmprsr-rs`.  You may wish to symlink
or rename it to `cmprsr` for convenience:

```bash
ln -s target/release/cmprsr-rs /usr/local/bin/cmprsr
```

## Usage

### Compress a file

```bash
cmprsr document.txt          # creates document.txt.cmpr
cmprsr -f document.txt       # force overwrite without warning
cmprsr -c document.txt       # write compressed data to stdout
```

### Decompress a file

```bash
cmprsr -d document.txt.cmpr output.txt
cmprsr -d -c document.txt.cmpr    # decompress to stdout
```

### Inspect a compressed file

```bash
cmprsr -l document.txt.cmpr

# Example output:
#          27           55   49.1%  document.txt.cmpr
```

### Other flags

| Flag | Description |
|---|---|
| `-f`, `--force` | Overwrite output without warning |
| `-k`, `--keep` | Keep input file (default; no-op) |
| `--version` | Print version and exit |
| `--help` | Print usage information |

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Error (bad input, corrupt file, I/O failure) |
| 2 | Usage error (bad flags) |

## File Format

`.cmpr` files use a simple binary format:

```
Offset  Size  Field
------  ----  ----------------------------------------
  0       4   Magic bytes        "CMPR"
  4       1   Version            0x02
  5       8   Original size      little-endian u64
 13       1   Padding bits       0..7
 14       2   Symbol count       little-endian u16 (N)
 16      N*2  Symbol table       N entries of (u8 symbol, u8 code_len)
 16+N*2  ...  Compressed data    Bitstream padded to byte boundary
 EOF-4    4   CRC-32             little-endian u32
```

The symbol table lists only bytes present in the original data, sorted by byte
value.  Each entry gives the byte value and its Huffman code length in bits.
The decoder reconstructs canonical codes from the lengths alone.

## Performance

- **Decode:** O(1) per byte via an 11-bit (2048-entry) prefix lookup table
- **Encode:** O(n) with streaming writes; uses `BufWriter` for efficient I/O
- **Memory:** ~64 KiB for tables + 8 KiB I/O buffers, regardless of file size

## Development

### Running tests

```bash
cargo test
```

### Fuzzing (requires nightly Rust)

```bash
rustup default nightly
cargo fuzz run roundtrip          # round-trip fuzzing
cargo fuzz run decompress_safety  # panic-free decompression
```

## License

MIT
