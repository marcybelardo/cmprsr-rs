use anyhow::Result;
use clap::{Parser, Subcommand};
use std::process::exit;

use cmprsr_rs::{
    canonical::Canonical,
    decoder::Decoder,
    encoder::Encoder,
    huffman::{generate_base_codes, HuffTree},
    utils::{read_file_to_bytes, read_file_to_string, write_bytes_to_file, write_string_to_file},
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// The file to be encoded/decoded
    file: Option<String>,

    /// Name for output file
    #[arg(short, long)]
    out: Option<String>,

    #[command(subcommand)]
    mode: Mode,
}

#[derive(Debug, Subcommand)]
enum Mode {
    /// Encode files to CMPR format
    Encode,

    /// Decode files from CMPR format
    Decode,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.file.as_deref().is_none() {
        eprintln!("Please provide a file");
        exit(1);
    }

    match &cli.mode {
        Mode::Encode => {
            let out_file = match cli.out.as_deref() {
                Some(file) => format!("{}.cmpr", file),
                None => String::from("out.cmpr"),
            };

            let filename = cli.file.as_deref().expect("Could not find the file");
            let text = read_file_to_string(filename).expect("Could not read to string");

            let mut encoder = Encoder::new(text);
            encoder.count_chars();

            let mut canon = Canonical::default();

            if let Some(tree) = HuffTree::build_huffman_tree(&mut encoder.char_map) {
                HuffTree::generate_base_codes(*tree.root, &mut canon.base_codes, String::new())?;
            }

            canon.generate_canon_codes();

            let mut bin_data = Vec::new();
            let header_data = encoder.write_header_data();
            let encoded_data = encoder.encode()?;

            bin_data.extend(header_data);
            bin_data.extend(encoded_data);

            write_bytes_to_file(&out_file, &bin_data)?;
        }
        Mode::Decode => {
            let out_file = match cli.out.as_deref() {
                Some(file) => format!("{}.txt", file),
                None => String::from("out.txt"),
            };

            let filename = cli.file.as_deref().expect("Could not find the file");
            let mut decoder = Decoder::new();
            if let Ok(data) = read_file_to_bytes(filename) {
                decoder.parse_header_and_data(&data)?;
            }

            let lookup_table = decoder.header_to_lookup_table();
            let decoded = decoder.decode(lookup_table)?;

            write_string_to_file(&out_file, &decoded)?;
        }
    }

    Ok(())
}
