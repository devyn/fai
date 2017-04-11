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

    Trace,
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
    pub inth: u32,
    pub int_outgoing: Option<u32>,
    pub halt: bool,
    pub flags: Flags,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    pub cmp_l: bool,
    pub cmp_g: bool,
    pub cmp_e: bool,
    pub int_pause: bool,
}

impl From<u32> for Flags {
    fn from(word: u32) -> Flags {
        Flags {
            cmp_l:     word & (1 << 0) != 0,
            cmp_g:     word & (1 << 1) != 0,
            cmp_e:     word & (1 << 2) != 0,
            int_pause: word & (1 << 9) != 0,
        }
    }
}

impl From<Flags> for u32 {
    fn from(flags: Flags) -> u32 {
        let mut word = 0;

        if flags.cmp_l {
            word |= 1 << 0;
        }
        if flags.cmp_g {
            word |= 1 << 1;
        }
        if flags.cmp_e {
            word |= 1 << 2;
        }
        if flags.int_pause {
            word |= 1 << 9;
        }

        word
    }
}
