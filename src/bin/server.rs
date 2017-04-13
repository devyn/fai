#[macro_use]
extern crate log;

extern crate env_logger;
extern crate websocket;
extern crate byteorder;

extern crate fai;

use std::thread;
use std::sync::mpsc::channel;
use std::time::Duration;
use std::fs::File;
use std::io::prelude::*;

use byteorder::*;

use websocket::{Server, Message, Stream, Client};

use fai::data::State;
use fai::machine::Machine;
use fai::event_pool::EventPool;
use fai::ram::Ram;
use fai::monitor::Monitor;
use fai::hardware::HardwareMessage;
use fai::device::{DeviceConfig, DeviceModel};

static PROTOCOL: &'static str = "v1.fai.devyn.me";

fn main() {
    env_logger::init().unwrap();

    let server = Server::bind("[::]:2391").unwrap();

    info!("Server listening on [::]:2391");

    for request in server.filter_map(Result::ok) {
        thread::spawn(move || {
            if !request.protocols().contains(&PROTOCOL.into()) {
                request.reject().unwrap();
                return;
            }

            let client = request.use_protocol(PROTOCOL).accept().unwrap();

            info!("Connection from {}", client.peer_addr().unwrap());

            handle_session(client);
        });
    }
}

fn handle_session<S: Stream>(mut client: Client<S>) {
    let load_address  = 0x11000;
    let stack_pointer = 0x10e00;
    let ram_size      = 0x2000;
    let bin_path      = "debug.bin";
    let tick_dur      = Duration::new(0, 100_000);

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

    let (monitor_tx, monitor_rx) = channel::<(u32, u32)>();

    let monitor = Monitor::new(monitor_tx);

    let mut pool = EventPool::new();

    let machine_id = pool.add_hardware(machine);
    let ram_id     = pool.add_hardware(ram);
    let monitor_id = pool.add_hardware(monitor);

    pool.connect(machine_id, ram_id);
    pool.connect(machine_id, monitor_id);

    let configs = vec![
        DeviceConfig {
            id: ram_id,
            model: DeviceModel::Ram.number(),
            interrupt: 0xffff_0001,
            memmap_base: 0x10000,
            memmap_size: ram_size
        },
        DeviceConfig {
            id: monitor_id,
            model: DeviceModel::Monitor.number(),
            interrupt: 0xffff_0002,
            memmap_base: 0x80000,
            memmap_size: DeviceModel::Monitor.memory_size().unwrap()
        },
    ];

    pool.dispatch().send(HardwareMessage::InitializeMachine(machine_id, configs));

    loop {
        match monitor_rx.try_recv() {
            Ok((offset, word)) => {
                client.send_message(&Message::text(format!("{},{}", offset, word))).unwrap();
            },
            Err(_) => ()
        }

        pool.tick();
        thread::sleep(tick_dur);
    }
}
