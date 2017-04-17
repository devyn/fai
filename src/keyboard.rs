use std::sync::mpsc::{Receiver, TryRecvError};

use hardware::{Hardware, Id, HardwareMessage, Route};
use event_pool::Dispatch;
use integrated_ram::IntegratedRam;

pub struct Keyboard {
    id: Option<Id>,
    machine: Option<Id>,
    input_rx: Receiver<u32>,

    ram: IntegratedRam,

    on: bool,
    initialize: bool,
    interrupt: bool,
    acknowledged: bool
}

impl Keyboard {
    pub fn new(input_rx: Receiver<u32>) -> Keyboard {
        Keyboard {
            id: None,
            machine: None,
            input_rx: input_rx,

            ram: IntegratedRam::new(1),

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

impl Hardware for Keyboard {
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
            debug!("ACK");
            self.acknowledged = true;
            self.interrupt = false;
            return;
        }

        if self.ram.has_pending_request() {
            let route = self.route();
            self.ram.tick(route, &mut dispatch);
            return;
        }

        if self.acknowledged {
            let result = self.input_rx.try_recv();

            match result {
                Ok(word) => {
                    self.ram.words[0] = word;
                    self.acknowledged = false;

                    debug!("incoming updated: {:#x}", word);

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
