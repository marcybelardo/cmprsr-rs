# cmprsr
## A Huffman Compression Tool!

### Implementing the Header Data
First four bytes : 0xAB, 0xCD

PER CHARACTER ENCODING:
- 32 bytes for char data
- 8 bytes for length of its Huffman Encoding
- Some number of bytes equivalent to the previous length

Last four bytes: 0xEF, 0x01
