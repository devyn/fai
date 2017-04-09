use event_pool::Dispatch;

pub trait Hardware {
    fn set_id(&mut self, id: Id);
    fn receive(&mut self, message: HardwareMessage);
    fn tick<'a>(&mut self, ts: u64, dispatch: Dispatch<'a>);
}

pub type Id = u32;
pub type LocalAddr = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Route {
    pub to: Id,
    pub from: Id,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HardwareMessage {
    InitializeMachine(Id, Vec<DeviceConfig>),
    InitializeDevice(Route),
    DeviceReady(Route),
    IntMachineToDevice(Route),
    IntDeviceToMachine(Route),
    MemGetRequest(Route, LocalAddr),
    MemGetResponse(Route, LocalAddr, u32, Cacheable),
    MemSetRequest(Route, LocalAddr),
    MemSetResponse(Route, LocalAddr, u32, Cacheable),
}

impl HardwareMessage {
    pub fn route(&self) -> Option<Route> {
        use self::HardwareMessage::*;

        match *self {
            InitializeDevice(route) |
            DeviceReady(route) |
            IntMachineToDevice(route) |
            IntDeviceToMachine(route) |
            MemGetRequest(route, ..) |
            MemGetResponse(route, ..) |
            MemSetRequest(route, ..) |
            MemSetResponse(route, ..) => Some(route),
            _ => None
        }
    }

    pub fn to(&self) -> Id {
        use self::HardwareMessage::*;

        if let Some(route) = self.route() {
            route.to
        } else {
            match *self {
                InitializeMachine(to, _) => to,
                _ => unreachable!()
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceConfig {
    pub id: Id,
    pub model: u32,
    pub interrupt: u32,
    pub memmap_base: u32,
    pub memmap_size: u32, // 0 = no memmap
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cacheable {
    No,
    Yes
}

impl Default for Cacheable {
    fn default() -> Cacheable {
        Cacheable::No
    }
}
