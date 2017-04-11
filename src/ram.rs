use hardware::{Hardware, Id, Route, HardwareMessage, Cacheable};
use event_pool::Dispatch;

#[derive(Debug, Clone, Copy)]
enum Request { Get(u32), Set(u32, u32) }

pub struct Ram {
    id: Option<Id>,
    machine: Option<Id>,
    words: Vec<u32>,
    on: bool,
    initialize: bool,
    request: Option<Request>,
}

impl Ram {
    pub fn new(size: u32) -> Ram {
        Ram {
            id: None,
            machine: None,
            words: vec![0; size as usize],
            on: false,
            initialize: false,
            request: None,
        }
    }

    pub fn words(&self) -> &[u32] {
        &self.words
    }

    pub fn words_mut(&mut self) -> &mut [u32] {
        &mut self.words
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

        match message {
            InitializeDevice(route) => {
                self.initialize = true;
                self.machine = Some(route.from);
            },
            MemGetRequest(_, addr) => {
                self.request = Some(Request::Get(addr));
            },
            MemSetRequest(_, addr, value) => {
                self.request = Some(Request::Set(addr, value));
            },
            _ => ()
        }
    }

    fn tick(&mut self, _ts: u64, mut dispatch: Dispatch) {
        use hardware::HardwareMessage::*;

        if self.initialize {
            self.request = None;

            /* TODO: Decide if mem should be cleared on init
            for word in self.words.iter_mut() {
                *word = 0;
            }
            */

            self.initialize = false;
            self.on = true;

            dispatch.send(DeviceReady(self.route()));
        } else if self.on {
            if let Some(request) = self.request.take() {
                debug!("request: {:?}", request);

                match request {
                    Request::Get(addr) => {
                        let result = self.words.get(addr as usize).cloned().unwrap_or(0);

                        dispatch.send(MemGetResponse(self.route(), addr, result, Cacheable::Yes));
                    },
                    Request::Set(addr, val) => {
                        let route = self.route();

                        if let Some(pos) = self.words.get_mut(addr as usize) {
                            *pos = val;
                            dispatch.send(MemSetResponse(route, addr, val, Cacheable::Yes));
                        } else {
                            // Fail to set
                            dispatch.send(MemSetResponse(route, addr, 0, Cacheable::Yes));
                        }
                    },
                }
            }
        }
    }
}
