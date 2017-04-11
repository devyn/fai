extern crate fai;

extern crate env_logger;

extern crate getopts;
extern crate byteorder;

use std::env;
use std::io::prelude::*;
use std::fs::File;
use std::process::exit;
use std::time::Duration;
use std::str::FromStr;

use getopts::Options;
use byteorder::*;

use fai::data::State;
use fai::machine::Machine;
use fai::event_pool::EventPool;
use fai::ram::Ram;
use fai::stdio_console::StdioConsole;
use fai::hardware::HardwareMessage;
use fai::device::{DeviceConfig, DeviceModel};

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} <file.bin> [options]", program);
    print!("{}", opts.usage(&brief));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    env_logger::init().unwrap();

    let mut opts = Options::new();

    opts.optflag("h", "help", "Show this message");

    opts.optopt("", "tick-rate", "How frequently to tick the event pool. Default: 10000 Hz",
                "HERTZ");

    opts.optopt("", "load-address", "Address (in hex) to write the program to in memory.
                                     Default: 11000", "ADDR");

    opts.optopt("", "stack-pointer", "Address (in hex) to start the stack at.
                                      Default: 10e00", "ADDR");

    opts.optopt("m", "ram-size", "Number of words (in hex) of RAM to make available to the machine. \
                                  RAM will be mounted at 10000. \
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

    fn f64_option(opt: Option<String>, default: f64) -> f64 {
        opt.as_ref().map(|s| f64::from_str(&s)).unwrap_or(Ok(default)).unwrap()
    }

    fn u32_hex_option(opt: Option<String>, default: u32) -> u32 {
        opt.as_ref().map(|s| u32::from_str_radix(&s, 16)).unwrap_or(Ok(default)).unwrap()
    }

    let tick_rate = f64_option(matches.opt_str("tick-rate"), 10_000.0);

    let tick_dur = {
        let tick_s = 1. / tick_rate;
        let seconds = tick_s as u64;
        let ns = ((tick_s % 1.) * 1_000_000_000.) as u32;
        Duration::new(seconds, ns)
    };

    let load_address = u32_hex_option(matches.opt_str("load-address"), 0x11000);
    assert!(load_address >= 0x10000);

    let stack_pointer = u32_hex_option(matches.opt_str("stack-pointer"), 0x10e00);

    let ram_size = u32_hex_option(matches.opt_str("ram-size"), 0x2000);

    let mut ram = Ram::new(ram_size);

    {
        let mut input = File::open(bin_path).unwrap();

        let mut ptr = load_address as usize;

        let mut buffer = [0; 4];

        while input.read_exact(&mut buffer).is_ok() {
            ram.words_mut()[ptr - 0x10000] = LittleEndian::read_u32(&buffer);
            ptr += 1;
        }
    }

    let mut default_state = State::default();

    default_state.sp = stack_pointer;
    default_state.ip = load_address;

    let machine = Machine::new(default_state);

    let stdio_console = StdioConsole::new();

    let mut pool = EventPool::new();

    let machine_id = pool.add_hardware(machine);
    let ram_id     = pool.add_hardware(ram);
    let console_id = pool.add_hardware(stdio_console);

    pool.connect(machine_id, ram_id);
    pool.connect(machine_id, console_id);

    let configs = vec![
        DeviceConfig {
            id: console_id,
            model: DeviceModel::DebugConsole.number(),
            interrupt: 0xffff_0001,
            memmap_base: 0x8c00,
            memmap_size: DeviceModel::DebugConsole.memory_size().unwrap()
        },
        DeviceConfig {
            id: ram_id,
            model: DeviceModel::Ram.number(),
            interrupt: 0xffff_0002,
            memmap_base: 0x10000,
            memmap_size: ram_size
        },
    ];

    pool.dispatch().send(HardwareMessage::InitializeMachine(machine_id, configs));

    pool.tick_real_clock(tick_dur);
}
