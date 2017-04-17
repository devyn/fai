use std::thread;
use std::io;
use std::io::prelude::*;
use std::sync::mpsc::{SyncSender, Receiver, TryRecvError, sync_channel};

use termion::raw::{RawTerminal, IntoRawMode};

use hardware::{Hardware, Id, HardwareMessage, Route};
use event_pool::Dispatch;
use integrated_ram::IntegratedRam;

static INT_MESSAGE: usize = 0;
static INCOMING: usize = 1;
static OUTGOING: usize = 2;

static ACK: u32 = 0;
static SEND: u32 = 1;

pub struct StdioConsole {
    id: Option<Id>,
    machine: Option<Id>,
    terminal: Option<RawTerminal<io::Stdout>>,
    stdin_rx: Option<Receiver<u8>>,

    ram: IntegratedRam,

    on: bool,
    initialize: bool,
    interrupt: bool,
    acknowledged: bool
}

impl StdioConsole {
    pub fn new() -> StdioConsole {
        StdioConsole {
            id: None,
            machine: None,
            terminal: None,
            stdin_rx: None,

            ram: IntegratedRam::new(3),

            on: false,
            initialize: false,
            interrupt: false,
            acknowledged: false
        }
    }

    fn route(&self) -> Route {
        Route { from: self.id.unwrap(), to: self.machine.unwrap() }
    }
}

impl Hardware for StdioConsole {
    fn set_id(&mut self, id: Id) {
        self.id = Some(id);
    }

    fn receive(&mut self, message: HardwareMessage) {
        use hardware::HardwareMessage::*;

        self.ram.receive(&message);

        match message {
            InitializeDevice(route) => {
                self.initialize = true;
                self.machine = Some(route.from);
            },
            IntMachineToDevice(_) => {
                self.interrupt = true;
            },
            _ => ()
        }
    }

    fn tick(&mut self, _ts: u64, mut dispatch: Dispatch) {
        use hardware::HardwareMessage::*;

        if self.initialize {
            let (tx, rx) = sync_channel(0);

            self.terminal = Some(io::stdout().into_raw_mode().unwrap());

            self.stdin_rx = Some(rx);

            thread::spawn(move || stdin_worker(tx));

            self.ram.reinitialize();
            self.ram.clear();

            self.initialize = false;
            self.on = true;
            self.interrupt = false;
            self.acknowledged = false;

            dispatch.send(DeviceReady(self.route()));

            return;
        }
        
        if !self.on { return; }

        if self.interrupt {
            match self.ram.words[INT_MESSAGE] {
                cmd if cmd == ACK => {
                    debug!("ACK");
                    self.acknowledged = true;
                },
                cmd if cmd == SEND => {
                    debug!("SEND {:#x}", self.ram.words[OUTGOING]);

                    let terminal = self.terminal.as_mut().unwrap();

                    terminal.write_all(&[self.ram.words[OUTGOING] as u8]).unwrap();
                    terminal.flush().unwrap();
                },
                other => {
                    debug!("Bad int_message = {:#010x}", other);
                }
            }
            self.interrupt = false;
            return;
        }

        if self.ram.has_pending_request() {
            let route = self.route();
            self.ram.tick(route, &mut dispatch);
            return;
        }

        if self.acknowledged {
            let result = self.stdin_rx.as_mut().unwrap().try_recv();

            match result {
                Ok(byte) => {
                    self.ram.words[INCOMING] = byte as u32;
                    self.acknowledged = false;

                    debug!("incoming updated: {:#x}", byte);

                    dispatch.send(IntDeviceToMachine(self.route()));
                },
                Err(TryRecvError::Disconnected) => {
                    panic!("stdin_worker() exited, for some reason. EOF on stdin?");
                },
                Err(TryRecvError::Empty) => (),
            }
            return;
        }
    }
}

fn stdin_worker(tx: SyncSender<u8>) {
    let mut stdin = io::stdin();

    let mut buffer = [0];

    loop {
        if stdin.read_exact(&mut buffer).is_err() {
            debug!("Read error");
            break;
        }

        if tx.send(buffer[0]).is_err() {
            debug!("Send error");
            break;
        }

        debug!("Sent successfully! {:?}", buffer);
    }
}
