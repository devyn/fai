use std::sync::mpsc::Sender;

use hardware::{Hardware, Id, HardwareMessage, Cacheable, Route};
use event_pool::Dispatch;

pub struct Monitor {
    id: Option<Id>,
    machine: Option<Id>,

    update_tx: Sender<(u32, u32)>,

    vid_mem: Vec<u32>,

    on: bool,
    initialize: bool,
    request: Option<Request>
}

#[derive(Debug, Clone, Copy)]
enum Request {
    Get(u32),
    Set(u32, u32)
}

impl Monitor {
    pub fn new(update_tx: Sender<(u32, u32)>) -> Monitor {
        Monitor {
            id: None,
            machine: None,

            update_tx: update_tx,

            vid_mem: vec![],

            on: false,
            initialize: false,
            request: None
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
            _ => ()
        }
    }

    fn tick(&mut self, _ts: u64, mut dispatch: Dispatch) {
        use hardware::HardwareMessage::*;

        if self.initialize {
            self.vid_mem = vec![0; 200]; // 40x20 = 200 words

            self.initialize = false;
            self.on = true;
            self.request = None;

            dispatch.send(DeviceReady(self.route()));

            return;
        }
        
        if !self.on { return; }

        if let Some(request) = self.request.take() {
            debug!("request: {:?}", request);

            match request {
                Request::Get(addr) => {
                    let result = self.vid_mem.get(addr as usize).cloned().unwrap_or(0);

                    dispatch.send(MemGetResponse(self.route(), addr, result, Cacheable::Yes));
                },
                Request::Set(addr, value) => {
                    let result = match self.vid_mem.get_mut(addr as usize) {
                        Some(ptr) => {
                            *ptr = value;

                            self.update_tx.send((addr, value)).unwrap();

                            value
                        },
                        None => 0
                    };

                    dispatch.send(MemSetResponse(self.route(), addr, result, Cacheable::Yes));
                },
            }
            return;
        }
    }
}
