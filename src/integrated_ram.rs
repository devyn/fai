use hardware::{HardwareMessage, Route, Cacheable};
use event_pool::Dispatch;

pub struct IntegratedRam {
    pub words: Vec<u32>,
    cacheable: Cacheable,
    request: Option<Request>,
}

#[derive(Debug, Clone, Copy)]
enum Request {
    Get(u32),
    Set(u32, u32)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Updated(pub u32);

impl Updated {
    pub fn unwrap(self) -> u32 {
        self.0
    }
}

impl IntegratedRam {
    pub fn new(size: u32) -> IntegratedRam {
        IntegratedRam {
            words: vec![0; size as usize],
            cacheable: Cacheable::No,
            request: None
        }
    }

    pub fn new_cacheable(size: u32) -> IntegratedRam {
        IntegratedRam {
            cacheable: Cacheable::Yes,
            ..IntegratedRam::new(size)
        }
    }

    pub fn reinitialize(&mut self) {
        self.request = None;
    }

    pub fn clear(&mut self) {
        for ptr in self.words.iter_mut() {
            *ptr = 0;
        }
    }

    pub fn has_pending_request(&self) -> bool {
        self.request.is_some()
    }

    pub fn receive(&mut self, message: &HardwareMessage) {
        use hardware::HardwareMessage::*;

        match *message {
            MemGetRequest(_, addr) => {
                self.request = Some(Request::Get(addr));
            },
            MemSetRequest(_, addr, value) => {
                self.request = Some(Request::Set(addr, value));
            },
            _ => ()
        }
    }

    pub fn tick(&mut self, route: Route, dispatch: &mut Dispatch) -> Option<Updated> {
        use hardware::HardwareMessage::*;

        if let Some(request) = self.request.take() {
            match request {
                Request::Get(addr) => {
                    let result = self.words.get(addr as usize).cloned().unwrap_or(0);

                    dispatch.send(MemGetResponse(route, addr, result, self.cacheable));

                    None
                },
                Request::Set(addr, val) => {
                    if let Some(pos) = self.words.get_mut(addr as usize) {
                        *pos = val;
                        dispatch.send(MemSetResponse(route, addr, val, self.cacheable));

                        Some(Updated(addr))
                    } else {
                        // Fail to set
                        dispatch.send(MemSetResponse(route, addr, 0, self.cacheable));

                        None
                    }
                },
            }
        } else {
            None
        }
    }
}
