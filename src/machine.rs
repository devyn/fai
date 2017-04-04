use data::*;
use interpret::*;
use bitcode::*;

pub struct Machine {
    pub state: State,
    pub mem: Vec<u8>,
}

impl Machine {
    pub fn new(memory_size: u32) -> Machine {
        Machine {
            state: State::default(),
            mem: vec![0; memory_size as usize]
        }
    }

    pub fn store_data(&mut self, base_addr: u32, data: &[u8]) {
        for (addr, &byte) in ((base_addr as usize)..).zip(data) {
            self.mem[addr] = byte;
        }
    }

    pub fn load_data(&mut self, base_addr: u32, out: &mut [u8]) {
        for (out_byte, &byte) in out.iter_mut().zip(&self.mem[(base_addr as usize)..]) {
            *out_byte = byte;
        }
    }

    pub fn store_words(&mut self, base_addr: u32, data: &[u32]) {
        let mut addr = base_addr;

        for &word in data {
            store(&mut self.mem, addr, word);
            addr += 4;
        }
    }

    pub fn load_words(&mut self, base_addr: u32, out: &mut [u32]) {
        let mut addr = base_addr;

        for out_word in out.iter_mut() {
            *out_word = load(&mut self.mem, addr);
            addr += 4;
        }
    }

    pub fn store_instructions(&mut self, base_addr: u32, insts: &[Instruction]) {
        let mut addr = base_addr;

        for &inst in insts {
            let words = encode_instruction(inst);

            store(&mut self.mem, addr + 0, words.0);
            store(&mut self.mem, addr + 4, words.1);

            addr += 8;
        }
    }

    pub fn load_instructions(&mut self, base_addr: u32, out: &mut [Instruction]) {
        let mut addr = base_addr;

        for out_inst in out.iter_mut() {
            let word0 = load(&self.mem, addr + 0);
            let word1 = load(&self.mem, addr + 4);

            *out_inst = decode_instruction((word0, word1));

            addr += 8;
        }
    }

    pub fn decode_next(&mut self) -> Instruction {
        let words = &mut [0, 0];

        let ip = self.state.ip;

        self.load_words(ip, words);

        self.state.ip += 8;

        decode_instruction((words[0], words[1]))
    }

    pub fn interpret(&mut self, inst: Instruction) {
        self.state = interpret(inst, &mut self.mem, self.state);
    }

    pub fn run_until_zero(&mut self) {
        while self.state.ip != 0 {
            let inst = self.decode_next();
            self.interpret(inst);
        }
    }

    pub fn trace_until_zero(&mut self) {
        while self.state.ip != 0 {
            println!("{:#?}", self.state);
            let inst = self.decode_next();

            println!("{:?}", inst);
            self.interpret(inst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use data::*;
    use data::Function::*;
    use data::Register::*;
    use data::Operand::*;

    static FACTORIAL: &'static [Instruction] = &[
        Instruction(Set, A, Const(1)), // 00
        Instruction(Cmp, C, Const(2)), // 08
        Instruction(BranchL, A, Const(0x0)), // 10
        Instruction(Mul, A, Reg(C)), // 18
        Instruction(Sub, C, Const(1)), // 20
        Instruction(Branch, A, Relative(-0x20)), // 28
    ];

    #[test]
    fn factorial() {
        let mut machine = Machine::new(0x200);

        machine.state.c = 10;
        machine.state.sp = 0x80;
        machine.state.ip = 0x100;

        machine.store_instructions(0x100, FACTORIAL);

        machine.trace_until_zero();

        assert_eq!(machine.state.a, 3628800);
    }
}
