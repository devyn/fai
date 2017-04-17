use hardware::{Hardware, Id, Route, HardwareMessage};
use event_pool::Dispatch;
use integrated_ram::IntegratedRam;

pub struct Ram {
    id: Option<Id>,
    machine: Option<Id>,
    ram: IntegratedRam,
    on: bool,
    initialize: bool,
}

impl Ram {
    pub fn new(size: u32) -> Ram {
        Ram {
            id: None,
            machine: None,
            ram: IntegratedRam::new_cacheable(size),
            on: false,
            initialize: false,
        }
    }

    pub fn words(&self) -> &[u32] {
        &self.ram.words
    }

    pub fn words_mut(&mut self) -> &mut [u32] {
        &mut self.ram.words
    }

    fn route(&self) -> Route {
        Route { from: self.id.unwrap(), to: self.machine.unwrap() }
    }
}

impl Hardware for Ram {
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
            _ => ()
        }
    }

    fn tick(&mut self, _ts: u64, mut dispatch: Dispatch) {
        use hardware::HardwareMessage::*;

        if self.initialize {
            self.ram.reinitialize();

            self.initialize = false;
            self.on = true;

            dispatch.send(DeviceReady(self.route()));
        } else if self.on {
            let route = self.route();

            self.ram.tick(route, &mut dispatch);
        }
    }
}
