mod frequency;
mod huffman;
mod bitio;
mod format;
mod compress;
mod decompress;

use std::path::Path;

use argh::FromArgs;

#[derive(FromArgs)]
/// A canonical Huffman compressor.
///
/// Compress:  cmprsr <file>
/// Decompress:  cmprsr -d <input.cmpr> <output>
struct Args {
    /// decompress a .cmpr file into the specified output
    #[argh(switch, short = 'd')]
    decompress: bool,

    /// input file path
    #[argh(positional)]
    input: String,

    /// output file path (required when decompressing, ignored otherwise)
    #[argh(positional)]
    output: Option<String>,
}

fn main() {
    // Handle --version before argh so it works without a positional argument.
    if std::env::args().any(|a| a == "--version") {
        println!("cmprsr v{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let args: Args = argh::from_env();
    let input_path = Path::new(&args.input);

    if args.decompress {
        // --- Decompress mode ---
        let output = match args.output {
            Some(path) => path,
            None => {
                eprintln!("error: output file is required when decompressing");
                std::process::exit(1);
            }
        };
        let output_path = Path::new(&output);

        if let Err(e) = decompress::decompress(input_path, output_path) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    } else {
        // --- Compress mode ---
        let mut output_path = input_path.to_path_buf();
        // Replace or append the .cmpr extension.
        output_path.set_extension("cmpr");

        if let Err(e) = compress::compress(input_path, &output_path) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
