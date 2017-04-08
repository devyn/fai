//! Fai bitcode
//!
//! Each instruction is two 32-bit words
//!
//! ```
//! 0 - 15: Function
//! 16 - 17: Register
//! 18: Operand (0 = Constant, 1 = Register)
//! 19 - 20: IF REGISTER(#18): Register content of operand
//! 21: IF CONSTANT(#18): Constant should be interpreted as signed (i32), relative to this
//!     instruction's location in memory
//! 22 - 31: Reserved for future use (must be zero)
//! 32 - 63: IF CONSTANT(#18): Constant content of operand
//! ```
//!
//! See source code for function/register tables.

use std::collections::BTreeMap;

use data::*;
use data::Function::*;
use data::Register::*;
use data::Operand::*;

lazy_static! {
    pub static ref FUNCTIONS: BTreeMap<u32, Function> = [
        (0x0000, Bad),

        (0x0001, Nop),
        (0x0002, Set),
        (0x0003, Load),
        (0x0004, Store),

        (0x0005, Cmp),
        (0x0006, Branch),
        (0x0007, BranchL),
        (0x0008, BranchG),
        (0x0009, BranchE),
        (0x000A, BranchNE),

        (0x000B, GetSp),
        (0x000C, SetSp),
        (0x000D, Push),
        (0x000E, Pop),
        (0x000F, Call),
        (0x0010, Ret),

        (0x0011, Add),
        (0x0012, Sub),
        (0x0013, Mul),
        (0x0014, Div),
        (0x0015, DivMod),

        (0x0016, Not),
        (0x0017, And),
        (0x0018, Or),
        (0x0019, Xor),
        (0x001A, Lsh),
        (0x001B, Rsh),
    ].iter().cloned().collect();

    pub static ref REGISTERS: BTreeMap<u32, Register> = [
        (0b00, A),
        (0b01, B),
        (0b10, C),
        (0b11, D),
    ].iter().cloned().collect();

    pub static ref FUNCTIONS_INV: BTreeMap<Function, u32> =
        FUNCTIONS.iter().map(|(k, v)| (*v, *k)).collect();

    pub static ref REGISTERS_INV: BTreeMap<Register, u32> =
        REGISTERS.iter().map(|(k, v)| (*v, *k)).collect();
}

pub fn encode_function(f: Function) -> u32 {
    *FUNCTIONS_INV.get(&f).unwrap()
}

pub fn decode_function(f: u32) -> Function {
    FUNCTIONS.get(&(f & 0xFFFF)).cloned().unwrap_or(Bad)
}

pub fn encode_register(r: Register) -> u32 {
    *REGISTERS_INV.get(&r).unwrap()
}

pub fn decode_register(r: u32) -> Register {
    *REGISTERS.get(&(r & 0x3)).unwrap()
}

pub fn encode_instruction(inst: Instruction) -> (u32, u32) {
    let Instruction(fun, reg, op) = inst;

    let fi = encode_function(fun);
    let ri = encode_register(reg);

    match op {
        Reg(op_reg) => (fi | (ri << 16) | (1 << 18) | (encode_register(op_reg) << 19), 0),
        Const(op_const) => (fi | (ri << 16), op_const),
        Relative(op_relative) => (fi | (ri << 16) | (1 << 21), op_relative as u32),
    }
}

pub fn decode_instruction(words: (u32, u32)) -> Instruction {
    let fi = words.0 & 0xFFFF;
    let ri = (words.0 >> 16) & 0x3;

    let fun = decode_function(fi);
    let reg = decode_register(ri);

    let op = match (words.0 >> 18) & 1 {
        0 => {
            match (words.0 >> 21) & 1 {
                0 => Const(words.1),
                1 => Relative(words.1 as i32),
                _ => unimplemented!()
            }
        },
        1 => {
            let op_ri = (words.0 >> 19) & 0x3;
            Reg(decode_register(op_ri))
        },
        _ => unreachable!()
    };

    Instruction(fun, reg, op)
}

#[cfg(test)]
mod tests {
    use super::*;

    use data::*;
    use data::Function::*;
    use data::Register::*;
    use data::Operand::*;

    static PROGRAM_INST: &'static [Instruction] = &[
        Instruction(Set, A, Const(1)), // 00
        Instruction(Cmp, C, Const(2)), // 02
        Instruction(BranchL, A, Relative(0x08)), // 04
        Instruction(Mul, A, Reg(C)), // 06
        Instruction(Sub, C, Const(1)), // 08
        Instruction(Branch, A, Relative(-0x08)), // 0a
        Instruction(Ret, A, Const(0)), // 0c
        Instruction(Bad, A, Const(0)), // 0e
    ];

    static PROGRAM_BITS: &'static [u32] = &[
        0x0002 | (0b00000000 << 16), 0x00000001,
        0x0005 | (0b00000010 << 16), 0x00000002,
        0x0007 | (0b00100000 << 16), 0x00000008,
        0x0013 | (0b00010100 << 16), 0x00000000,
        0x0012 | (0b00000010 << 16), 0x00000001,
        0x0006 | (0b00100000 << 16), 0xfffffff8,
        0x0010 | (0b00000000 << 16), 0x00000000,
        0x0000 | (0b00000000 << 16), 0x00000000,
    ];

    #[test]
    fn encode() {
        let mut mem = vec![0; 0x10];

        let instructions = PROGRAM_INST;

        let mut ptr = 0;

        for &inst in instructions {
            let words = encode_instruction(inst);

            store(&mut mem, ptr, words.0);
            store(&mut mem, ptr + 1, words.1);

            ptr += 2;
        }

        let bits = PROGRAM_BITS;

        assert_eq!(mem.len(), bits.len());

        for (idx, (&mem_word, &bits_word)) in mem.iter().zip(bits).enumerate() {
            assert!(mem_word == bits_word,
                "index {:#x} ({}.{}): actual word {:#x} differs from expectation {:#x}",
                idx, idx/8, idx%8, mem_word, bits_word);
        }
    }

    #[test]
    fn decode() {
        let instructions = PROGRAM_INST;
        let mem = PROGRAM_BITS;

        let mut ptr = 0;

        let mut decoded = vec![];

        while ptr < 0x10 {
            let w0 = load(&mem, ptr);
            let w1 = load(&mem, ptr + 1);

            decoded.push(decode_instruction((w0, w1)));

            ptr += 2;
        }

        assert_eq!(decoded, instructions);
    }

    #[test]
    fn symmetry() {
        let mut mem = vec![0; 0x10];

        let instructions = PROGRAM_INST;

        let mut ptr = 0;

        for &inst in instructions {
            let words = encode_instruction(inst);

            store(&mut mem, ptr, words.0);
            store(&mut mem, ptr + 1, words.1);

            ptr += 2;
        }

        ptr = 0;

        let mut decoded = vec![];

        while ptr < 0x10 {
            let w0 = load(&mem, ptr);
            let w1 = load(&mem, ptr + 1);

            decoded.push(decode_instruction((w0, w1)));

            ptr += 2;
        }

        assert_eq!(decoded, instructions);
    }
}
