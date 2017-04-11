use std::thread;
use std::time::Duration;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use hardware::{Hardware, HardwareMessage, Route, Id};
use device::DeviceConfig;

pub struct EventPool {
    ts: u64,
    id_counter: Id,
    hardware: BTreeMap<Id, Box<Hardware>>,
    mailboxes: BTreeMap<Id, VecDeque<HardwareMessage>>,
    routes: BTreeSet<Route>,
}

impl EventPool {
    pub fn new() -> EventPool {
        EventPool {
            ts: 0,
            id_counter: 1,
            hardware: BTreeMap::new(),
            mailboxes: BTreeMap::new(),
            routes: BTreeSet::new(),
        }
    }

    pub fn dispatch<'a>(&'a mut self) -> Dispatch<'a> {
        Dispatch {
            ensure_from: None,
            routes: &mut self.routes,
            mailboxes: &mut self.mailboxes
        }
    }

    pub fn tick(&mut self) {
        for (owner, mbox) in &mut self.mailboxes {
            let hw = self.hardware.get_mut(owner).unwrap();
            for message in mbox.drain(..) {
                debug!("<- {:?}", message);
                hw.receive(message);
            }
        }

        for (id, hw) in &mut self.hardware {
            debug!("tick {}, {}", id, self.ts);
            hw.tick(self.ts, Dispatch {
                ensure_from: Some(*id),
                routes: &mut self.routes,
                mailboxes: &mut self.mailboxes
            })
        }

        self.ts += 1;
    }

    pub fn tick_real_clock(&mut self, delay: Duration) {
        loop {
            self.tick();
            thread::sleep(delay);
        }
    }

    pub fn add_hardware<H>(&mut self, mut hw: H) -> u32 where H: Hardware, H: 'static {
        let id = self.id_counter;
        self.id_counter += 1;

        hw.set_id(id);

        self.hardware.insert(id, Box::new(hw));
        self.mailboxes.insert(id, VecDeque::new());

        id
    }

    pub fn connect(&mut self, a: Id, b: Id) {
        self.routes.insert(Route { from: a, to: b });
        self.routes.insert(Route { from: b, to: a });
    }

    pub fn disconnect(&mut self, a: Id, b: Id) {
        self.routes.remove(&Route { from: a, to: b });
        self.routes.remove(&Route { from: b, to: a });
    }

    pub fn initialize_machine(&mut self, machine_id: Id, devices: &[DeviceConfig]) {
        self.dispatch().send(HardwareMessage::InitializeMachine(machine_id, devices.to_vec()));
    }
}

// Just protects hardware from doing anything other than sending messages
pub struct Dispatch<'a> {
    ensure_from: Option<Id>,
    mailboxes: &'a mut BTreeMap<Id, VecDeque<HardwareMessage>>,
    routes: &'a mut BTreeSet<Route>,
}

impl<'a> Dispatch<'a> {
    pub fn send(&mut self, message: HardwareMessage) {
        if let Some(route) = message.route() {
            if self.routes.contains(&route) {
                if let Some(id) = self.ensure_from {
                    assert_eq!(route.from, id);
                }
                self.mailboxes.get_mut(&route.to).unwrap().push_back(message);
            }
        } else if self.ensure_from.is_none() {
            let to = message.to();

            if let Some(mbox) = self.mailboxes.get_mut(&to) {
                mbox.push_back(message);
            }
        }
    }
}
