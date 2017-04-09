extern crate fai;

extern crate byteorder;
extern crate getopts;

use std::env;
use std::process::exit;
use std::fs::File;
use std::io;
use std::io::prelude::*;

use getopts::Options;
use byteorder::*;

use fai::assemble::assemble;
use fai::bitcode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Pretty,
    PlainText,
    Binary,
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [file.fai] [options]", program);
    print!("{}", opts.usage(&brief));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();

    opts.optflag("h", "help", "Show this message");

    opts.optopt("f", "format", "Choices: pretty, plaintext, binary. \
                                Default: pretty", "FORMAT");

    opts.optopt("o", "out", "File to write the result to, instead of stdout", "FILE");

    let matches = opts.parse(&args[1..]).unwrap();

    if matches.opt_present("help") {
        print_usage(&program, opts);
        return;
    }

    let format = matches.opt_str("format").map(|s| match &s[..] {
        "pretty"    => OutputFormat::Pretty,
        "plaintext" => OutputFormat::PlainText,
        "binary"    => OutputFormat::Binary,
        x           => panic!("Invalid value provided for --format: {}", x)
    }).unwrap_or(OutputFormat::Pretty);

    let mut buffer = vec![];

    match matches.free.len() {
        0 => {
            io::stdin().read_to_end(&mut buffer).unwrap();
        },
        1 => {
            let mut f = File::open(&matches.free[0]).unwrap();
            f.read_to_end(&mut buffer).unwrap();
        },
        _ => {
            print_usage(&program, opts);
            exit(1);
        }
    }

    let mut bitcode = vec![];

    assemble(&buffer, &mut bitcode).unwrap();

    let mut out_stream: Box<Write> = match matches.opt_str("out") {
        Some(s) => Box::new(File::create(&s).unwrap()),
        None    => Box::new(io::stdout())
    };

    match format {
        OutputFormat::Pretty    => output_pretty(&bitcode, &mut out_stream),
        OutputFormat::PlainText => output_plain_text(&bitcode, &mut out_stream),
        OutputFormat::Binary    => output_binary(&bitcode, &mut out_stream),
    }.unwrap()
}

fn output_pretty<W: Write>(bitcode: &[u32], mut out_stream: W) -> io::Result<()> {
    let mut current_ptr = 0;

    while current_ptr < bitcode.len() {
        write!(out_stream, "{:08x}    {:08x}", current_ptr, bitcode[current_ptr])?;

        if current_ptr + 1 < bitcode.len() {
            write!(out_stream, " {:08x}", bitcode[current_ptr + 1])?;

            let inst = bitcode::decode_instruction(
                (bitcode[current_ptr], bitcode[current_ptr + 1]));

            write!(out_stream, "    {:?}", inst)?;
        } else {
            write!(out_stream, " {:8}", "")?;
        }

        writeln!(out_stream)?;

        current_ptr += 2;
    }

    Ok(())
}

fn output_plain_text<W: Write>(bitcode: &[u32], mut out_stream: W) -> io::Result<()> {
    for &word in bitcode {
        writeln!(out_stream, "{:08x}", word)?;
    }

    Ok(())
}

fn output_binary<W: Write>(bitcode: &[u32], mut out_stream: W) -> io::Result<()> {
    let mut buffer = [0u8; 4];

    for &word in bitcode {
        LittleEndian::write_u32(&mut buffer, word);
        out_stream.write_all(&buffer)?;
    }

    Ok(())
}
