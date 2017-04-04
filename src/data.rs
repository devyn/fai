#[derive(Debug, Clone, Copy)]
pub struct Instruction(pub Function, pub Register, pub Operand);

#[derive(Debug, Clone, Copy)]
pub enum Function {
    Nop,
    Set,
    Load,
    Store,

    Cmp,
    Branch,
    BranchL,
    BranchG,
    BranchE,
    BranchNE,

    GetSp,
    SetSp,
    Push,
    Pop,
    Call,
    Ret,

    Add,
    Sub,
    Mul,
    Div,
    DivMod,

    Not,
    And,
    Or,
    Xor,
    Lsh,
    Rsh,
}

#[derive(Debug, Clone, Copy)]
pub enum Register {
    A,
    B,
    C,
    D,
}

#[derive(Debug, Clone, Copy)]
pub enum Operand {
    Reg(Register),
    Const(u32),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    pub ip: u32,
    pub sp: u32,
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
    pub flags: Flags,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    pub l: bool,
    pub g: bool,
    pub e: bool,
}

impl State {
    pub fn register(&self, register: Register) -> u32 {
        match register {
            Register::A => self.a,
            Register::B => self.b,
            Register::C => self.c,
            Register::D => self.d
        }
    }

    pub fn register_modify<F>(self, register: Register, fun: F) -> State
        where F: FnOnce(u32) -> u32 {

        match register {
            Register::A => State { a: fun(self.a), ..self },
            Register::B => State { b: fun(self.b), ..self },
            Register::C => State { c: fun(self.c), ..self },
            Register::D => State { d: fun(self.d), ..self },
        }
    }

    pub fn operand(&self, operand: Operand) -> u32 {
        match operand {
            Operand::Reg(reg) => self.register(reg),
            Operand::Const(c) => c
        }
    }

    pub fn branch(self, op: Operand) -> State {
        State { ip: self.operand(op), ..self }
    }
}

pub fn load(mem: &[u8], addr: u32) -> u32 {
    let addr = addr as usize;
    mem[addr] as u32 |
        ((mem[addr + 1] as u32) << 8) |
        ((mem[addr + 2] as u32) << 16) |
        ((mem[addr + 3] as u32) << 24)
}

pub fn store(mem: &mut [u8], addr: u32, val: u32) {
    let addr = addr as usize;

    mem[addr] = (val & 0xff) as u8;
    mem[addr + 1] = ((val >> 8) & 0xff) as u8;
    mem[addr + 2] = ((val >> 16) & 0xff) as u8;
    mem[addr + 3] = ((val >> 24) & 0xff) as u8;
}
