use std::collections::VecDeque;

use data::*;
use interpret::*;
use mem_backend::*;
use bitcode::*;
use hardware::{Hardware, HardwareMessage, Id, Route};
use device::DeviceConfig;
use event_pool::Dispatch;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PipelineStage {
    Fetch,
    Execute(Instruction),
    Interrupt(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PowerState {
    Off,
    ReadyForInit,
    WaitingForDevices(Vec<Id>),
    On
}

pub type MemoryError = TransactionalMemError;

pub struct Machine {
    id: Option<Id>,
    default_state: State,
    state: State,
    power_state: PowerState,
    pipeline_stage: PipelineStage,
    interrupt_queue: VecDeque<u32>,
    device_configs: Vec<DeviceConfig>,
    device_config_rom: Vec<u32>,
    mem_backend: TransactionalMemBackend,
    fake_mem: Option<(u32, Vec<u32>)>,
}

impl Machine {
    pub fn new(default_state: State) -> Machine {
        Machine {
            id: None,
            default_state: default_state,
            state: default_state,
            power_state: PowerState::Off,
            pipeline_stage: PipelineStage::Fetch,
            interrupt_queue: VecDeque::new(),
            device_configs: vec![],
            device_config_rom: vec![],
            mem_backend: TransactionalMemBackend::new(),
            fake_mem: None,
        }
    }

    pub fn transition(&mut self, pipeline_stage: PipelineStage) {
        debug!("transition() from = {:?}, to = {:?}", self.pipeline_stage, pipeline_stage);

        self.pipeline_stage = pipeline_stage;
        self.mem_backend.reset();
    }

    fn p_fetch(&mut self) -> Result<(), MemoryError> {
        let ip = self.state.ip;

        debug!("p_fetch() ip = {:#010x}", ip);

        let word0 = self.load(ip + 0)?; debug!("p_fetch() word0 = {:#010x}", word0);
        let word1 = self.load(ip + 1)?; debug!("p_fetch() word1 = {:#010x}", word1);

        let inst = decode_instruction((word0, word1));

        self.state.ip += 2;

        self.transition(PipelineStage::Execute(inst));

        Ok(())
    }

    fn p_execute(&mut self, inst: Instruction) -> Result<(), MemoryError> {
        let old_state = self.state;

        debug!("p_execute() inst = {:?}", inst);

        self.state = interpret(inst, self, old_state)?;

        self.transition(PipelineStage::Fetch);

        Ok(())
    }

    fn p_interrupt(&mut self, code: u32) -> Result<(), MemoryError> {
        let old_state = self.state;

        debug!("p_interrupt() code = {:#010x}", code);

        self.state = handle_interrupt(code, self, old_state)?;

        self.transition(PipelineStage::Fetch);

        Ok(())
    }

    pub fn advance(&mut self) -> Result<(), MemoryError> {
        match self.pipeline_stage {
            PipelineStage::Fetch           => self.p_fetch(),
            PipelineStage::Execute(inst)   => self.p_execute(inst),
            PipelineStage::Interrupt(code) => self.p_interrupt(code),
        }
    }

    pub fn run_until_halt(&mut self) -> Result<(), MemoryError> {
        while !self.state.halt {
            self.advance()?;
        }
        Ok(())
    }

    fn create_device_config_rom(&mut self) {
        let mut rom = vec![];

        for device_config in &self.device_configs {
            rom.push(device_config.model);
            rom.push(device_config.interrupt);
            rom.push(device_config.memmap_base);
            rom.push(device_config.memmap_size);
        }

        self.device_config_rom = rom;
    }

    pub fn initialize(&mut self, device_configs: &[DeviceConfig]) {
        self.state = self.default_state;
        self.interrupt_queue = VecDeque::new();

        self.device_configs = device_configs.to_owned();
        self.create_device_config_rom();

        self.pipeline_stage = PipelineStage::Fetch;
        self.mem_backend.reset();

        self.power_state = PowerState::ReadyForInit;
    }

    pub fn addr_to_device(&self, addr: u32) -> Option<(Id, u32)> {
        for config in &self.device_configs {
            if addr >= config.memmap_base && (addr - config.memmap_base) < config.memmap_size {
                return Some((config.id, addr - config.memmap_base));
            }
        }
        None
    }

    pub fn device_to_addr(&self, device: Id, addr: u32) -> Option<u32> {
        if let Some(config) = self.device_configs.iter().find(|d| d.id == device) {
            if addr < config.memmap_size {
                Some(config.memmap_base + addr)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn service_mem_request(&mut self, req: TransactionalMemRequest, dispatch: &mut Dispatch) {
        use hardware::HardwareMessage::*;

        match req {
            TransactionalMemRequest::Get(addr) => {
                if let Some((id, d_addr)) = self.addr_to_device(addr) {
                    dispatch.send(MemGetRequest(self.route(id), d_addr));
                } else {
                    self.mem_backend.respond_get(addr, 0);
                }
            },
            TransactionalMemRequest::Set(addr, val) => {
                if let Some((id, d_addr)) = self.addr_to_device(addr) {
                    dispatch.send(MemSetRequest(self.route(id), d_addr, val));
                } else {
                    self.mem_backend.respond_set(addr, 0);
                }
            }
        }
    }

    fn service_mem_response(&mut self, msg: HardwareMessage) {
        use hardware::HardwareMessage::*;

        match msg {
            MemGetResponse(route, d_addr, val, _cacheable) => {
                if let Some(addr) = self.device_to_addr(route.from, d_addr) {
                    self.mem_backend.respond_get(addr, val);
                }
            },
            MemSetResponse(route, d_addr, val, _cacheable) => {
                if let Some(addr) = self.device_to_addr(route.from, d_addr) {
                    self.mem_backend.respond_set(addr, val);
                }
            },
            other => panic!("service_mem_response() got {:?}", other)
        }
    }

    fn send_interrupt(&self, code: u32, dispatch: &mut Dispatch) {
        debug!("send_interrupt() code = {:#010x}", code);

        let config = self.device_configs.iter().find(|c| c.interrupt == code);

        if let Some(config) = config {
            debug!("send_interrupt() resolved {:#010x} to device {}", code, config.id);

            dispatch.send(HardwareMessage::IntMachineToDevice(self.route(config.id)));
        }
    }

    fn route(&self, to: Id) -> Route {
        Route { from: self.id.unwrap(), to: to }
    }
}

impl MemBackend for Machine {
    type Error = MemoryError;

    fn load(&mut self, addr: u32) -> Result<u32, MemoryError> {
        if addr >= 0x1000 && ((addr - 0x1000) as usize) < self.device_config_rom.len() {
            Ok(self.device_config_rom[(addr - 0x1000) as usize])

        } else if let Some((mount_point, ref fake_mem)) = self.fake_mem {
            if addr >= mount_point && ((addr - mount_point) as usize) < fake_mem.len() {
                Ok(fake_mem[(addr - mount_point) as usize])
            } else {
                panic!("load out of bounds of fake mem: {:#010x}", addr);
            }

        } else {
            self.mem_backend.load(addr)
        }
    }

    fn store(&mut self, addr: u32, val: u32) -> Result<(), MemoryError> {
        if addr >= 0x1000 && ((addr - 0x1000) as usize) < self.device_config_rom.len() {
            // Read only memory. Do nothing.
            Ok(())

        } else if let Some((mount_point, ref mut fake_mem)) = self.fake_mem {
            if addr >= mount_point && ((addr - mount_point) as usize) < fake_mem.len() {
                fake_mem[(addr - mount_point) as usize] = val;
                Ok(())
            } else {
                panic!("store out of bounds of fake mem: {:#010x} <- {:#010x}", addr, val);
            }

        } else {
            self.mem_backend.store(addr, val)
        }
    }
}

impl Hardware for Machine {
    fn set_id(&mut self, id: Id) {
        self.id = Some(id);
    }

    fn receive(&mut self, message: HardwareMessage) {
        use hardware::HardwareMessage::*;

        match message {
            InitializeMachine(_, configs) => {
                self.initialize(&configs);
            },
            DeviceReady(route) => {
                if let PowerState::WaitingForDevices(ref mut devices) = self.power_state {
                    devices.retain(|&d| d != route.from);
                }
            },
            IntDeviceToMachine(route) => {
                let config = self.device_configs.iter().find(|c| c.id == route.from);

                if let Some(config) = config {
                    self.interrupt_queue.push_back(config.interrupt);

                    debug!("interrupt_queue = {:?}, int_pause = {:?}",
                           self.interrupt_queue, self.state.flags.int_pause);
                }
            },
            MemGetResponse(..) | MemSetResponse(..) => {
                self.service_mem_response(message);
            },
            _ => ()
        }
    }

    fn tick(&mut self, _ts: u64, mut dispatch: Dispatch) {
        match self.power_state {
            PowerState::Off => {
                return;
            },
            PowerState::ReadyForInit => {
                for config in &self.device_configs {
                    dispatch.send(HardwareMessage::InitializeDevice(self.route(config.id)));
                }

                self.power_state = PowerState::WaitingForDevices(
                    self.device_configs.iter().map(|d| d.id).collect());

                return;
            },
            PowerState::WaitingForDevices(_) => {
                let is_empty = match self.power_state {
                    PowerState::WaitingForDevices(ref devices) => devices.is_empty(),
                    _ => unreachable!()
                };

                if is_empty {
                    self.power_state = PowerState::On;
                } else {
                    return;
                }
            },
            PowerState::On => ()
        }

        if self.mem_backend.pending().is_some() {
            return;
        }

        if !self.interrupt_queue.is_empty() &&
            self.pipeline_stage == PipelineStage::Fetch &&
            !self.state.flags.int_pause {

            let code = self.interrupt_queue.pop_front().unwrap();

            self.transition(PipelineStage::Interrupt(code));

            self.state.halt = false;
        }

        if self.state.halt {
            return;
        }

        match self.advance() {
            Ok(_) => {
                if let Some(code) = self.state.int_outgoing.take() {
                    self.send_interrupt(code, &mut dispatch);
                }
            },

            Err(TransactionalMemError::Need(req)) => {
                self.mem_backend.retry();

                self.service_mem_request(req, &mut dispatch);
            }
        }
    }
}

/* TODO: fix these tests
#[cfg(test)]
mod tests {
    use super::*;

    use data::*;
    use data::Function::*;
    use data::Register::*;
    use data::Operand::*;

    static FACTORIAL: &'static [Instruction] = &[
        Instruction(Set, A, Const(1)), // 00
        Instruction(Cmp, C, Const(2)), // 02
        Instruction(BranchL, A, Relative(0x08)), // 04
        Instruction(Mul, A, Reg(C)), // 06
        Instruction(Sub, C, Const(1)), // 08
        Instruction(Branch, A, Relative(-0x08)), // 0A
        Instruction(Halt, A, Const(0)), // 0C
    ];

    #[test]
    fn factorial() {
        let mut machine = Machine::new(0x80);

        machine.state.c = 10;
        machine.state.sp = 0x20;
        machine.state.ip = 0x40;

        machine.store_instructions(0x40, FACTORIAL);

        machine.run_until_halt();

        assert_eq!(machine.state.a, 3628800);
    }
}
*/
