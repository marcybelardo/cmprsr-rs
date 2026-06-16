use std::fs::File;
use std::path::Path;

use argh::FromArgs;

use cmprsr_rs::compress;
use cmprsr_rs::decompress;
use cmprsr_rs::format;

#[derive(FromArgs)]
/// A fast canonical Huffman compressor.
///
/// Compress:    cmprsr <file>
/// Decompress:  cmprsr -d <input.cmpr> <output>
/// List info:   cmprsr -l <file.cmpr>
#[derive(PartialEq, Debug)]
struct Args {
    /// decompress a .cmpr file into the specified output
    #[argh(switch, short = 'd')]
    decompress: bool,

    /// list information about a .cmpr file (reads header only)
    #[argh(switch, short = 'l')]
    list: bool,

    /// write to stdout (compressed data or decompressed output)
    #[argh(switch, short = 'c')]
    stdout: bool,

    /// force overwrite of output file without warning
    #[argh(switch, short = 'f')]
    force: bool,

    /// keep the input file after compression (default, no-op)
    #[argh(switch, short = 'k')]
    keep: bool,

    /// input file path
    #[argh(positional)]
    input: String,

    /// output file path (required when decompressing unless --stdout)
    #[argh(positional)]
    output: Option<String>,
}

fn main() {
    // Handle --version before argh so it works without any arguments.
    if std::env::args().any(|a| a == "--version") {
        println!("cmprsr v{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let args: Args = argh::from_env();

    // --list mode: inspect a .cmpr file without decompressing
    if args.list {
        if let Err(e) = list_file(Path::new(&args.input)) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return;
    }

    let input_path = Path::new(&args.input).to_path_buf();

    if args.decompress {
        decompress_cmd(&input_path, &args);
    } else {
        compress_cmd(&input_path, &args);
    }
}

// ---------------------------------------------------------------------------
// Compression command
// ---------------------------------------------------------------------------

fn compress_cmd(input_path: &Path, args: &Args) {
    let output_path = if args.stdout {
        None
    } else {
        let mut p = input_path.to_path_buf();
        p.set_extension("cmpr");
        Some(p)
    };

    if let Some(ref out) = output_path {
        if out.exists() && !args.force {
            eprintln!(
                "Warning: overwriting existing file `{}`",
                out.display()
            );
        }
    }

    let result = if let Some(ref out) = output_path {
        compress::compress(input_path, out)
    } else {
        compress::compress_to_stdout(input_path)
    };

    match result {
        Ok((original_size, compressed_size)) => {
            // Print compression statistics to stderr
            let ratio = if original_size > 0 {
                (compressed_size as f64 / original_size as f64) * 100.0
            } else {
                0.0
            };
            let display_name = output_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<stdout>".to_string());
            eprintln!(
                "original: {original_size:>12}   compressed: {compressed_size:>12}   ratio: {ratio:.1}%   {}",
                display_name
            );
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// Decompression command
// ---------------------------------------------------------------------------

fn decompress_cmd(input_path: &Path, args: &Args) {
    let output_path = if args.stdout {
        None
    } else {
        match args.output.as_deref() {
            Some(p) => Some(Path::new(p).to_path_buf()),
            None => {
                eprintln!("error: output file is required when decompressing (use -c for stdout)");
                std::process::exit(2);
            }
        }
    };

    if let Some(ref out) = output_path {
        if out.exists() && !args.force {
            eprintln!(
                "Warning: overwriting existing file `{}`",
                out.display()
            );
        }
    }

    let result = if let Some(ref out) = output_path {
        decompress::decompress(input_path, out)
    } else {
        decompress::decompress_to_stdout(input_path)
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// List command
// ---------------------------------------------------------------------------

fn list_file(input_path: &Path) -> std::io::Result<()> {
    let mut file = File::open(input_path)?;
    let header = format::read_header(&mut file)?;

    // Get the total file size
    let file_len = file.metadata()?.len();

    // Compressed data size (excludes header and CRC)
    let header_size = format::FIXED_HEADER_SIZE as u64 + header.symbol_count as u64 * 2;
    let compressed_data_size = if header.version >= 0x02 {
        file_len - header_size - format::CRC_SIZE
    } else {
        file_len - header_size
    };

    let ratio = if header.original_size > 0 {
        (compressed_data_size as f64 / header.original_size as f64) * 100.0
    } else {
        0.0
    };

    // Use tab-aligned output like gzip -l
    println!(
        "{:>12} {:>12} {:>7}  {}",
        compressed_data_size,
        header.original_size,
        format!("{:.1}%", ratio),
        input_path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default()
    );

    Ok(())
}
