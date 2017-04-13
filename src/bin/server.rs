#[macro_use]
extern crate log;

extern crate env_logger;
extern crate websocket;
extern crate byteorder;

extern crate fai;

use std::str;
use std::thread;
use std::sync::mpsc::{Sender, TryRecvError, channel};
use std::time::Duration;
use std::fs::File;
use std::io::prelude::*;

use byteorder::*;

use websocket::{Server, Message};
use websocket::message::Type;
use websocket::sender::Writer;
use websocket::receiver::Reader;

use fai::data::State;
use fai::machine::Machine;
use fai::event_pool::EventPool;
use fai::ram::Ram;
use fai::monitor::Monitor;
use fai::keyboard::Keyboard;
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

            let ip = client.peer_addr().unwrap().to_string();

            let (client_rx, client_tx) = client.split().unwrap();

            handle_session(ip, client_rx, client_tx);
        });
    }
}

enum ClientMsg {
    KeyboardInput(u32),
    WsPing(Vec<u8>),
}

fn handle_session<R, W>(ip: String, client_rx: Reader<R>, mut client_tx: Writer<W>)
    where R: Read + Send + 'static, W: Write {

    let load_address  = 0x11000;
    let stack_pointer = 0x10e00;
    let ram_size      = 0x2000;
    let bin_path      = "debug.bin";
    let tick_dur      = Duration::new(0, 10_000);

    info!("Connection from {}", ip);

    let (client_msg_tx, client_msg_rx) = channel::<ClientMsg>();

    thread::spawn(move || {
        handle_client_messages(client_rx, client_msg_tx);
    });

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
    let monitor                  = Monitor::new(monitor_tx);

    let (keyboard_tx, keyboard_rx) = channel::<u32>();
    let keyboard                   = Keyboard::new(keyboard_rx);

    let mut pool = EventPool::new();

    let machine_id  = pool.add_hardware(machine);
    let ram_id      = pool.add_hardware(ram);
    let monitor_id  = pool.add_hardware(monitor);
    let keyboard_id = pool.add_hardware(keyboard);

    pool.connect(machine_id, ram_id);
    pool.connect(machine_id, monitor_id);
    pool.connect(machine_id, keyboard_id);

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
        DeviceConfig {
            id: keyboard_id,
            model: DeviceModel::Keyboard.number(),
            interrupt: 0xffff_0003,
            memmap_base: 0x8a00,
            memmap_size: DeviceModel::Keyboard.memory_size().unwrap()
        },
    ];

    pool.dispatch().send(HardwareMessage::InitializeMachine(machine_id, configs));

    loop {
        match client_msg_rx.try_recv() {
            Ok(ClientMsg::KeyboardInput(word)) => {
                keyboard_tx.send(word).unwrap();
            },

            Ok(ClientMsg::WsPing(buf)) => {
                client_tx.send_message(&Message::pong(buf)).unwrap();
            },

            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => {
                // Client has left.
                info!("Client disconnected: {}", ip);
                break;
            }
        }

        match monitor_rx.try_recv() {
            Ok((offset, word)) => {
                client_tx.send_message(
                    &Message::text(format!("{},{}", offset, word))).unwrap();
            },
            Err(_) => ()
        }

        pool.tick();

        if pool.ts() % 10 == 0 {
            thread::sleep(tick_dur);
        }
    }
}

fn handle_client_messages<R: Read>(mut client_rx: Reader<R>, client_msg_tx: Sender<ClientMsg>) {
    for msg in client_rx.incoming_messages().map(|m| m.unwrap()) {
        let msg: Message = msg;

        match msg.opcode {
            Type::Text => {
                let ch = str::from_utf8(&msg.payload).unwrap()
                    .chars().nth(0).unwrap();

                if ch <= '\u{ff}' {
                    client_msg_tx.send(ClientMsg::KeyboardInput(ch as u32)).unwrap();
                }
            },
            Type::Ping => {
                client_msg_tx.send(ClientMsg::WsPing(msg.payload.into_owned())).unwrap();
            },
            Type::Close => {
                break;
            },
            _ => ()
        }
    }
}
