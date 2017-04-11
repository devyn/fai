use std::thread;
use std::io;
use std::io::prelude::*;
use std::sync::mpsc::{SyncSender, Receiver, TryRecvError, sync_channel};

use termion::raw::{RawTerminal, IntoRawMode};

use hardware::{Hardware, Id, HardwareMessage, Cacheable, Route};
use event_pool::Dispatch;

#[derive(Default)]
pub struct StdioConsole {
    id: Option<Id>,
    machine: Option<Id>,
    terminal: Option<RawTerminal<io::Stdout>>,
    stdin_rx: Option<Receiver<u8>>,

    int_message: u32,
    incoming: u32,
    outgoing: u32,

    on: bool,
    initialize: bool,
    interrupt: bool,
    acknowledged: bool,
    request: Option<Request>
}

#[derive(Debug, Clone, Copy)]
enum Request {
    Get(u32),
    Set(u32, u32)
}

impl StdioConsole {
    pub fn new() -> StdioConsole {
        StdioConsole::default()
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

        match message {
            InitializeDevice(route) => {
                self.initialize = true;
                self.machine = Some(route.from);
            },
            MemGetRequest(_, addr) => {
                self.request = Some(Request::Get(addr));
            },
            MemSetRequest(_, addr, val) => {
                self.request = Some(Request::Set(addr, val));
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

            self.int_message = 0;
            self.incoming = 0;
            self.outgoing = 0;

            self.initialize = false;
            self.on = true;
            self.interrupt = false;
            self.acknowledged = false;
            self.request = None;

            dispatch.send(DeviceReady(self.route()));

            return;
        }
        
        if !self.on { return; }

        if self.interrupt {
            match self.int_message {
                0 => /* Ack */ {
                    debug!("ACK");
                    self.acknowledged = true;
                },
                1 => /* Send */ {
                    debug!("SEND {:#x}", self.outgoing);

                    let terminal = self.terminal.as_mut().unwrap();

                    terminal.write_all(&[self.outgoing as u8]).unwrap();
                    terminal.flush().unwrap();
                },
                _ => {
                    debug!("Bad int_message = {:#010x}", self.int_message);
                }
            }
            self.interrupt = false;
            return;
        }

        if let Some(request) = self.request.take() {
            debug!("request: {:?}", request);

            match request {
                Request::Get(addr) => {
                    let result = match addr {
                        0 => self.int_message,
                        1 => self.incoming,
                        2 => self.outgoing,
                        _ => 0
                    };

                    dispatch.send(MemGetResponse(self.route(), addr, result, Cacheable::No));
                },
                Request::Set(addr, value) => {
                    let result = match addr {
                        0 => { self.int_message = value; value },
                        2 => { self.outgoing = value; value },

                        _ => 0
                    };

                    dispatch.send(MemSetResponse(self.route(), addr, result, Cacheable::No));
                },
            }
            return;
        }

        if self.acknowledged {
            let result = self.stdin_rx.as_mut().unwrap().try_recv();

            match result {
                Ok(byte) => {
                    self.incoming = byte as u32;
                    self.acknowledged = false;

                    debug!("incoming updated: {:#x}", self.incoming);

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
