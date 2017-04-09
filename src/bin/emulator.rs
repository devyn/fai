extern crate fai;

extern crate getopts;
extern crate byteorder;

use std::env;
use std::io::prelude::*;
use std::fs::File;
use std::process::exit;

use getopts::Options;
use byteorder::*;

use fai::machine::Machine;

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} <file.bin> [options]", program);
    print!("{}", opts.usage(&brief));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();

    opts.optflag("h", "help", "Show this message");

    opts.optflag("", "trace", "Print the state of the machine after each instruction");

    opts.optopt("", "load-address", "Address (in hex) to write the program to in memory.
                                     Default: 1000", "ADDR");

    opts.optopt("", "stack-pointer", "Address (in hex) to start the stack at.
                                      Default: 900", "ADDR");

    opts.optopt("m", "ram-size", "Number of words (in hex) of RAM to make available to the machine. \
                                  Default: 2000", "WORDS");

    let matches = opts.parse(&args[1..]).unwrap();

    if matches.opt_present("help") {
        print_usage(&program, opts);
        return;
    }

    let bin_path = match matches.free.len() {
        1 => matches.free[0].clone(),
        _ => {
            print_usage(&program, opts);
            exit(1);
        }
    };

    fn u32_hex_option(opt: Option<String>, default: u32) -> u32 {
        opt.as_ref().map(|s| u32::from_str_radix(&s, 16)).unwrap_or(Ok(default)).unwrap()
    }

    let load_address = u32_hex_option(matches.opt_str("load-address"), 0x1000);

    let stack_pointer = u32_hex_option(matches.opt_str("stack-pointer"), 0x900);

    let ram_size = u32_hex_option(matches.opt_str("ram-size"), 0x2000);

    let mut machine = Machine::new(ram_size);

    {
        let mut input = File::open(bin_path).unwrap();

        let mut ptr = load_address;

        let mut buffer = [0; 4];

        while input.read_exact(&mut buffer).is_ok() {
            machine.store_words(ptr, &[LittleEndian::read_u32(&buffer)]);
            ptr += 1;
        }
    }

    machine.state.sp = stack_pointer;
    machine.state.ip = load_address;

    if matches.opt_present("trace") {
        machine.trace_until_halt();
    } else {
        machine.run_until_halt();
    }
}
