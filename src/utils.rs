use std::fs::{self, File};
use std::io::{Read, Write};

use anyhow::Result;

pub fn read_file_to_string(filename: &str) -> Result<String> {
    let mut file = File::open(filename)?;
    let mut text = String::new();

    file.read_to_string(&mut text).unwrap();

    Ok(text)
}

pub fn read_file_to_bytes(filename: &str) -> Result<Vec<u8>> {
    let data = fs::read(filename)?;

    Ok(data)
}

pub fn string_to_bytes(binary_string: String) -> Result<Vec<u8>> {
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
        current >>= 8 - bit_count;
        bytes.push(current);
    }

    Ok(bytes)
}

pub fn bytes_to_string(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:08b}", b))
        .collect::<Vec<String>>()
        .join("")
}

pub fn write_bytes_to_file(filename: &str, bytes: &[u8]) -> Result<()> {
    let mut file = File::create(filename)?;
    file.write_all(bytes)?;
    Ok(())
}

pub fn write_string_to_file(filename: &str, text: &str) -> std::io::Result<()> {
    fs::write(filename, text)
}
