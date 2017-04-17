use std::sync::mpsc::Sender;

use hardware::{Hardware, Id, HardwareMessage, Route};
use event_pool::Dispatch;
use integrated_ram::{IntegratedRam, Updated};

pub struct Monitor {
    id: Option<Id>,
    machine: Option<Id>,

    update_tx: Sender<(u32, u32)>,

    vid_ram: IntegratedRam,

    on: bool,
    initialize: bool
}

impl Monitor {
    pub fn new(update_tx: Sender<(u32, u32)>) -> Monitor {
        Monitor {
            id: None,
            machine: None,

            update_tx: update_tx,

            vid_ram: IntegratedRam::new(0),

            on: false,
            initialize: false
        }
    }

    fn route(&self) -> Route {
        Route { from: self.id.unwrap(), to: self.machine.unwrap() }
    }
}

impl Hardware for Monitor {
    fn set_id(&mut self, id: Id) {
        self.id = Some(id);
    }

    fn receive(&mut self, message: HardwareMessage) {
        use hardware::HardwareMessage::*;

        self.vid_ram.receive(&message);

        match message {
            InitializeDevice(route) => {
                self.initialize = true;
                self.machine = Some(route.from);
            },
            _ => ()
        }
    }

    fn tick(&mut self, _ts: u64, mut dispatch: Dispatch) {
        use hardware::HardwareMessage::*;

        if self.initialize {
            self.vid_ram = IntegratedRam::new(200); // 40x20 = 200 words

            self.initialize = false;
            self.on = true;

            dispatch.send(DeviceReady(self.route()));

            return;
        }
        
        if !self.on { return; }

        let route = self.route();

        if let Some(Updated(addr)) = self.vid_ram.tick(route, &mut dispatch) {
            self.update_tx.send((addr, self.vid_ram.words[addr as usize])).unwrap();
        }
    }
}
