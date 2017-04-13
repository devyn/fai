use std::sync::mpsc::{Receiver, TryRecvError};

use hardware::{Hardware, Id, HardwareMessage, Cacheable, Route};
use event_pool::Dispatch;

pub struct Keyboard {
    id: Option<Id>,
    machine: Option<Id>,
    input_rx: Receiver<u32>,

    incoming: u32,

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

impl Keyboard {
    pub fn new(input_rx: Receiver<u32>) -> Keyboard {
        Keyboard {
            id: None,
            machine: None,
            input_rx: input_rx,

            incoming: 0,

            on: false,
            initialize: false,
            interrupt: false,
            acknowledged: false,
            request: None
        }
    }

    fn route(&self) -> Route {
        Route { from: self.id.unwrap(), to: self.machine.unwrap() }
    }
}

impl Hardware for Keyboard {
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
            self.incoming = 0;

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
            debug!("ACK");
            self.acknowledged = true;
            self.interrupt = false;
            return;
        }

        if let Some(request) = self.request.take() {
            debug!("request: {:?}", request);

            match request {
                Request::Get(addr) => {
                    let result = match addr {
                        0 => self.incoming,
                        _ => 0
                    };

                    dispatch.send(MemGetResponse(self.route(), addr, result, Cacheable::No));
                },
                Request::Set(addr, _value) => {
                    // Not writable
                    let result = match addr {
                        0 => self.incoming,
                        _ => 0
                    };

                    dispatch.send(MemSetResponse(self.route(), addr, result, Cacheable::No));
                },
            }
            return;
        }

        if self.acknowledged {
            let result = self.input_rx.try_recv();

            match result {
                Ok(word) => {
                    self.incoming = word;
                    self.acknowledged = false;

                    debug!("incoming updated: {:#x}", self.incoming);

                    dispatch.send(IntDeviceToMachine(self.route()));
                },
                Err(TryRecvError::Disconnected) => {
                    warn!("The keyboard's input source seems to have been disconnected");
                    self.on = false;
                },
                Err(TryRecvError::Empty) => (),
            }
            return;
        }
    }
}
