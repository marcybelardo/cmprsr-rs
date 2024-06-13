# cmprsr
## A Huffman Compression Tool!

### Program Implementation
#### Compression
1. Text is read from the file and a Huffman Tree is generated from the text data
2. The lengths of the huffman codes and the characters are inserted into a vector
3. The vector is sorted, first by the code length, then by lexographical order (use tuples?)
4. Given that info, canonical Huffman codes are generated
5. Codebook information is stored, using a list of the lengths of the canonical codes, followed by a list of the characters themselves
6. The codebook information is written as the header of the file, followed by the encoded information

#### Decompression
1. The codebook information is read, and the list of code lengths and list of symbols are read to memory
2. The code lengths are used to reconstruct the canonical codes for every character
3. Original text is regenerated from the encoded data, and is then written back into a file
