//! Fai bitcode
//!
//! Each instruction is either one or two 32-bit words
//!
//! ```text
//! 0 - 15: Function
//! 16 - 17: Register
//! 18: Operand (0 = Constant, 1 = Register)
//! 19 - 20: IF REGISTER(#18): Register content of operand
//! 21: IF CONSTANT(#18): Constant should be interpreted as signed (i32), relative to this
//!     instruction's location in memory
//! 22: Single word instruction - constant will default to zero
//! 23 - 31: Reserved for future use (must be zero)
//! ```
//!
//! If double word instruction (#22 = 0):
//!
//! ```text
//! 32 - 63: IF CONSTANT(#18): Constant content of operand
//! ```
//!
//! See source code for function/register tables.

#![allow(unused_imports)] // For some reason Rust seems to think our globs are unused

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

        (0x001C, Halt),
        (0x001D, IntSw),
        (0x001E, IntHw),
        (0x001F, IntPause),
        (0x0020, IntCont),
        (0x0021, IntHGet),
        (0x0022, IntHSet),
        (0x0023, IntExit),

        (0x0024, Trace),
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

pub fn encode_instruction(inst: Instruction, out: &mut Vec<u32>) {
    let Instruction(fun, reg, op) = inst;

    let fi = encode_function(fun);
    let ri = encode_register(reg);

    match op {
        Reg(op_reg) => {
            out.push(
                fi |
                (ri << 16) |
                (1 << 18) |
                (encode_register(op_reg) << 19) |
                (1 << 22)
            );
        },
        Const(op_const) => {
            let n = fi | (ri << 16);

            if op_const == 0 {
                out.push(n | (1 << 22));
            } else {
                out.push(n);
                out.push(op_const);
            }
        },
        Relative(op_relative) => {
            // We could actually do the same as Const if we get Relative(0), but it's not very
            // common and the possibility makes the assembler's label handling more complicated.
            //
            // So we just let the assembler expect that Relative always results in two words.
            out.push(fi | (ri << 16) | (1 << 21));
            out.push(op_relative as u32);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    NeedMore,
}

pub fn decode_instruction(words: &[u32]) -> Result<Instruction, DecodeError> {
    // Handle compact (single word) instructions
    let words =
        if words[0] & (1 << 22) == 0 {
            (words[0], words.get(1).cloned().ok_or(DecodeError::NeedMore)?)
        } else {
            (words[0], 0)
        };


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

    Ok(Instruction(fun, reg, op))
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
        Instruction(Halt, A, Const(0)), // 0e
    ];

    static PROGRAM_BITS: &'static [u32] = &[
        0x0002 | (0b00000000 << 16), 0x00000001,
        0x0005 | (0b00000010 << 16), 0x00000002,
        0x0007 | (0b00100000 << 16), 0x00000008,
        0x0013 | (0b01010100 << 16),
        0x0012 | (0b00000010 << 16), 0x00000001,
        0x0006 | (0b00100000 << 16), 0xfffffff8,
        0x0010 | (0b01000000 << 16),
        0x001C | (0b01000000 << 16),
    ];

    static PROGRAM_BITS_ALT: &'static [u32] = &[
        0x0002 | (0b00000000 << 16), 0x00000001,
        0x0005 | (0b00000010 << 16), 0x00000002,
        0x0007 | (0b00100000 << 16), 0x00000008,
        0x0013 | (0b00010100 << 16), 0x00000000,
        0x0012 | (0b00000010 << 16), 0x00000001,
        0x0006 | (0b00100000 << 16), 0xfffffff8,
        0x0010 | (0b00000000 << 16), 0x00000000,
        0x001C | (0b00000000 << 16), 0x00000000,
    ];

    fn encode_all(instructions: &[Instruction]) -> Vec<u32> {
        let mut out = vec![];

        for &inst in instructions {
            encode_instruction(inst, &mut out);
        }

        out
    }

    fn decode_all(code: &[u32]) -> Vec<Instruction> {
        let mut ptr = 0;

        let mut decoded = vec![];

        while ptr < code.len() {
            let w0 = code[ptr];

            let inst = decode_instruction(&[w0])
                .or_else(|DecodeError::NeedMore| {
                    ptr += 1;

                    let w1 = code[ptr];

                    decode_instruction(&[w0, w1])
                })
                .unwrap();

            ptr += 1;

            decoded.push(inst);
        }

        decoded
    }

    #[test]
    fn encode() {
        let mem = encode_all(PROGRAM_INST);

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
        let decoded = decode_all(PROGRAM_BITS);

        assert_eq!(decoded, PROGRAM_INST);
    }

    #[test]
    fn symmetry() {
        let encoded = encode_all(PROGRAM_INST);
        let decoded = decode_all(&encoded);

        assert_eq!(decoded, PROGRAM_INST);
    }

    #[test]
    fn decode_dword_equivalents() {
        assert_eq!(decode_all(PROGRAM_BITS_ALT), decode_all(PROGRAM_BITS));
    }
}
