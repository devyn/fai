#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction(pub Function, pub Register, pub Operand);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Function {
    Bad,

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

    Halt,
    IntSw,
    IntHw,
    IntPause,
    IntCont,
    IntHGet,
    IntHSet,
    IntExit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Register {
    A,
    B,
    C,
    D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    Reg(Register),
    Const(u32),
    Relative(i32),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    pub ip: u32,
    pub sp: u32,
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
    pub halt: bool,
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
            Operand::Const(c) => c,
            Operand::Relative(rel) => (self.ip - 2).wrapping_add(rel as u32),
        }
    }

    pub fn branch(self, op: Operand) -> State {
        State { ip: self.operand(op), ..self }
    }
}

#[inline]
pub fn load(mem: &[u32], addr: u32) -> u32 {
    mem[addr as usize]
}

#[inline]
pub fn store(mem: &mut [u32], addr: u32, val: u32) {
    mem[addr as usize] = val;
}
