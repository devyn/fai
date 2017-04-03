use std::cmp::Ordering;

#[derive(Debug, Clone, Copy)]
struct Instruction(Function, Register, Operand);

#[derive(Debug, Clone, Copy)]
enum Function {
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
    Add,
    Sub,
    Mul,
    Div,
    Not,
    And,
    Or,
    Xor,
    Lsh,
    Rsh,
}

#[derive(Debug, Clone, Copy)]
enum Register {
    A,
    B,
    C,
    D,
}

#[derive(Debug, Clone, Copy)]
enum Operand {
    Reg(Register),
    Const(u32),
}

#[derive(Debug, Clone, Copy, Default)]
struct State {
    ip: u32,
    a: u32,
    b: u32,
    c: u32,
    d: u32,
    flags: Flags,
}

#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    l: bool,
    g: bool,
    e: bool,
}

impl State {
    fn register(&self, register: Register) -> u32 {
        match register {
            Register::A => self.a,
            Register::B => self.b,
            Register::C => self.c,
            Register::D => self.d
        }
    }

    fn register_modify<F>(self, register: Register, fun: F) -> State
        where F: FnOnce(u32) -> u32 {

        match register {
            Register::A => State { a: fun(self.a), ..self },
            Register::B => State { b: fun(self.b), ..self },
            Register::C => State { c: fun(self.c), ..self },
            Register::D => State { d: fun(self.d), ..self },
        }
    }

    fn operand(&self, operand: Operand) -> u32 {
        match operand {
            Operand::Reg(reg) => self.register(reg),
            Operand::Const(c) => c
        }
    }

    fn branch(self, op: Operand) -> State {
        State { ip: self.operand(op), ..self }
    }
}

fn interpret(inst: Instruction, mem: &mut [u8], state: State) -> State {
    use self::Function::*;

    let Instruction(f, reg, op) = inst;

    match f {
        Nop => state,
        Set => state.register_modify(reg, |_| state.operand(op)),
        Load => {
            let addr = state.operand(op) as usize;
            let val = mem[addr] as u32 |
                ((mem[addr + 1] as u32) << 8) |
                ((mem[addr + 2] as u32) << 16) |
                ((mem[addr + 3] as u32) << 24);
            state.register_modify(reg, |_| val)
        },
        Store => {
            let addr = state.operand(op) as usize;
            let val = state.register(reg);

            mem[addr] = (val & 0xff) as u8;
            mem[addr + 1] = ((val >> 8) & 0xff) as u8;
            mem[addr + 2] = ((val >> 16) & 0xff) as u8;
            mem[addr + 3] = ((val >> 24) & 0xff) as u8;

            state
        },
        Cmp => State {
            flags: match state.register(reg).cmp(&state.operand(op)) {
                Ordering::Less    => Flags { l: true, g: false, e: false },
                Ordering::Greater => Flags { l: false, g: true, e: false },
                Ordering::Equal   => Flags { l: false, g: false, e: true }
            },
            ..state
        },
        Branch => state.branch(op),
        BranchL => if state.flags.l { state.branch(op) } else { state },
        BranchG => if state.flags.g { state.branch(op) } else { state },
        BranchE => if state.flags.e { state.branch(op) } else { state },
        BranchNE => if !state.flags.e { state.branch(op) } else { state },
        Add => state.register_modify(reg, |x| x + state.operand(op)),
        Sub => state.register_modify(reg, |x| x - state.operand(op)),
        Mul => state.register_modify(reg, |x| x * state.operand(op)),
        Div => state.register_modify(reg, |x| x / state.operand(op)),
        Not => state.register_modify(reg, |x| !x),
        And => state.register_modify(reg, |x| x & state.operand(op)),
        Or  => state.register_modify(reg, |x| x | state.operand(op)),
        Xor => state.register_modify(reg, |x| x ^ state.operand(op)),
        Lsh => state.register_modify(reg, |x| x << state.operand(op)),
        Rsh => state.register_modify(reg, |x| x >> state.operand(op)),
    }
}

fn main() {
    use self::Function::*;
    use self::Register::*;
    use self::Operand::*;

    let mut mem = vec![0; 64];

    let instructions = &[
        Instruction(Set, A, Const(5)),
        Instruction(Mul, A, Const(5)),
        Instruction(Set, B, Const(16)),
        Instruction(Store, A, Reg(B)),
        Instruction(Load, C, Reg(B)),
    ];

    let state =
        instructions.iter().cloned()
            .fold(State::default(), |state, i| interpret(i, &mut mem, state));

    println!("{:?}", mem);
    println!("{:#?}", state);

    assert_eq!(state.c, 25);
}
