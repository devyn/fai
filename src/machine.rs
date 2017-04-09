use data::*;
use interpret::*;
use bitcode::*;

pub struct Machine {
    pub state: State,
    pub mem: Vec<u32>,
}

impl Machine {
    pub fn new(memory_size: u32) -> Machine {
        Machine {
            state: State::default(),
            mem: vec![0; memory_size as usize]
        }
    }

    pub fn store_words(&mut self, base_addr: u32, data: &[u32]) {
        let start = base_addr as usize;
        let end   = start + data.len();
        self.mem[start..end].copy_from_slice(data);
    }

    pub fn load_words(&self, base_addr: u32, out: &mut [u32]) {
        let start = base_addr as usize;
        let end   = start + out.len();
        out.copy_from_slice(&self.mem[start..end]);
    }

    pub fn store_instructions(&mut self, base_addr: u32, insts: &[Instruction]) {
        let mut addr = base_addr;

        for &inst in insts {
            let words = encode_instruction(inst);

            store(&mut self.mem, addr + 0, words.0);
            store(&mut self.mem, addr + 1, words.1);

            addr += 2;
        }
    }

    pub fn load_instructions(&mut self, base_addr: u32, out: &mut [Instruction]) {
        let mut addr = base_addr;

        for out_inst in out.iter_mut() {
            let word0 = load(&self.mem, addr + 0);
            let word1 = load(&self.mem, addr + 1);

            *out_inst = decode_instruction((word0, word1));

            addr += 2;
        }
    }

    pub fn decode_next(&mut self) -> Instruction {
        let words = &mut [0, 0];

        let ip = self.state.ip;

        self.load_words(ip, words);

        self.state.ip += 2;

        decode_instruction((words[0], words[1]))
    }

    pub fn interpret(&mut self, inst: Instruction) {
        self.state = interpret(inst, &mut self.mem, self.state);
    }

    pub fn run_until_halt(&mut self) {
        while !self.state.halt {
            let inst = self.decode_next();
            self.interpret(inst);
        }
    }

    pub fn trace_until_halt(&mut self) {
        while !self.state.halt {
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

        machine.trace_until_halt();

        assert_eq!(machine.state.a, 3628800);
    }
}
