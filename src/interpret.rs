use std::cmp::Ordering;

use data::*;
use mem_backend::MemBackend;

pub fn interpret<M>(inst: Instruction, mem: &mut M, state: State) -> Result<State, M::Error>
    where M: MemBackend + ?Sized {

    use data::Function::*;

    let Instruction(f, reg, op) = inst;

    Ok(match f {
        Bad => panic!("bad instruction"),
        Nop => state,
        Set => state.register_modify(reg, |_| state.operand(op)),
        Load => {
            let val = mem.load(state.operand(op))?;
            state.register_modify(reg, |_| val)
        },
        Store => {
            mem.store(state.operand(op), state.register(reg))?;
            state
        },
        Cmp => State {
            flags: {
                let mut new_flags = Flags {
                    cmp_l: false, cmp_g: false, cmp_e: false, ..state.flags };

                match state.register(reg).cmp(&state.operand(op)) {
                    Ordering::Less    => { new_flags.cmp_l = true; },
                    Ordering::Greater => { new_flags.cmp_g = true; },
                    Ordering::Equal   => { new_flags.cmp_e = true; }
                }

                new_flags
            },
            ..state
        },
        Branch => state.branch(op),
        BranchL => if state.flags.cmp_l { state.branch(op) } else { state },
        BranchG => if state.flags.cmp_g { state.branch(op) } else { state },
        BranchE => if state.flags.cmp_e { state.branch(op) } else { state },
        BranchNE => if !state.flags.cmp_e { state.branch(op) } else { state },

        GetSp => state.register_modify(reg, |_| state.sp),
        SetSp => State { sp: state.operand(op), ..state },
        Push => {
            let new_sp = state.sp - 1;
            mem.store(new_sp, state.register(reg))?;
            State { sp: new_sp, ..state }
        },
        Pop => {
            let val = mem.load(state.sp)?;
            State { sp: state.sp + 1, ..state.register_modify(reg, |_| val) }
        },
        Call => {
            let new_sp = state.sp - 1;
            mem.store(new_sp, state.ip)?;
            State { sp: new_sp, ip: state.operand(op), ..state }
        },
        Ret => {
            let val = mem.load(state.sp)?;
            State { sp: state.sp + 1, ip: val, ..state }
        },

        Add => state.register_modify(reg, |x| x.wrapping_add(state.operand(op))),
        Sub => state.register_modify(reg, |x| x.wrapping_sub(state.operand(op))),
        Mul => state.register_modify(reg, |x| x * state.operand(op)),
        Div => state.register_modify(reg, |x| x / state.operand(op)),
        DivMod => {
            let x = state.register(reg);
            let y = state.operand(op);

            // Always puts the result in registers C, D

            State {
                c: x / y,
                d: x % y,
                ..state
            }
        },

        Not => state.register_modify(reg, |x| !x),
        And => state.register_modify(reg, |x| x & state.operand(op)),
        Or  => state.register_modify(reg, |x| x | state.operand(op)),
        Xor => state.register_modify(reg, |x| x ^ state.operand(op)),
        Lsh => state.register_modify(reg, |x| x << state.operand(op)),
        Rsh => state.register_modify(reg, |x| x >> state.operand(op)),

        Halt     => State { halt: true, ..state },
        IntSw    => handle_interrupt(state.operand(op), mem, state)?,
        IntHw    => State { int_outgoing: Some(state.operand(op)), ..state },
        IntPause => State { flags: Flags { int_pause: true, ..state.flags }, ..state },
        IntCont  => State { flags: Flags { int_pause: false, ..state.flags }, ..state },
        IntHGet  => state.register_modify(reg, |_| state.inth),
        IntHSet  => State { inth: state.operand(op), ..state },
        IntExit  => {
            let a     = mem.load(state.sp + 0)?;
            let ip    = mem.load(state.sp + 1)?;
            let flags = Flags::from(mem.load(state.sp + 2)?);

            let new_sp = state.sp + 3;

            State { sp: new_sp, ip: ip, a: a, flags: flags, ..state }
        },

        Trace => {
            if let Operand::Const(0) = op {
                info!("trace(ip={:#010x}) {:#?}", state.ip, state);
            } else {
                info!("trace(ip={:#010x}) {:?} = {:#010x}", state.ip, op, state.operand(op));
            }
            state
        },
    })
}

pub fn handle_interrupt<M>(code: u32, mem: &mut M, state: State) -> Result<State, M::Error>
    where M: MemBackend + ?Sized {

    if state.inth == 0 {
        Ok(state)
    } else {
        let new_sp = state.sp - 3;

        mem.store(new_sp + 0, state.a)?;
        mem.store(new_sp + 1, state.ip)?;
        mem.store(new_sp + 2, state.flags.into())?;

        Ok(State {
            a: code,
            sp: new_sp,
            ip: state.inth,
            halt: false,
            flags: Flags {
                int_pause: true,
                ..Flags::default()
            },
            ..state
        })
    }
}

#[cfg(test)]
mod tests {
    use data::*;
    use data::Function::*;
    use data::Operand::*;
    use data::Register::*;

    // Test-friendly boiler-plate free version
    fn interpret(instruction: Instruction, mem: &mut [u32], state: State) -> State {
        super::interpret(instruction, mem, state).unwrap_or_else(|addr| {
            panic!("out of bounds: {:x}", addr);
        })
    }

    fn interprets(instructions: &[Instruction], mem: &mut [u32], state: State) -> State {
        instructions.iter().cloned()
            .fold(state, |state, inst| interpret(inst, mem, state))
    }

    fn memless_test(state0: State, instructions: &[Instruction], state1: State) {
        let state1_actual = interprets(instructions, &mut vec![], state0);

        assert_eq!(state1_actual, state1);
    }

    #[test]
    #[should_panic]
    fn interpret_bad() {
        memless_test(
            State::default(),
            &[
                Instruction(Bad, A, Const(0))
            ],
            State::default()
        )
    }

    #[test]
    fn interpret_nop() {
        memless_test(
            State::default(),
            &[
                Instruction(Nop, A, Const(0))
            ],
            State::default()
        );
    }

    #[test]
    fn interpret_set() {
        let mut mem = vec![];

        let n = 0x8381adef;

        let state0 = State::default();
        let state1 = interpret(Instruction(Set, B, Const(n)), &mut mem, state0);
        let state2 = interpret(Instruction(Set, C, Reg(B)), &mut mem, state1);

        assert_eq!(state1, State { b: n, ..state0 });
        assert_eq!(state2, State { c: n, ..state1 });
    }

    #[test]
    fn interpret_load() {
        let mut mem = vec![0x0403_0201, 0x4030_2010];

        let mem_orig = mem.clone();

        let state0 =
            State { d: 0x0000_0001, ..State::default() };

        let state1 = interprets(
            &[
                Instruction(Load, A, Const(0x0000_0000)),
                Instruction(Load, B, Reg(D))
            ],
            &mut mem,
            state0
        );

        assert_eq!(state1, State {
            a: 0x0403_0201,
            b: 0x4030_2010,
            ..state0
        });

        assert_eq!(mem, mem_orig);
    }

    #[test]
    fn interpret_store() {
        let mut mem = vec![0; 2];

        let state0 =
            State {
                a: 0xaabb_ccdd,
                b: 0xeeff_1111,
                d: 0x0000_0001,
                ..State::default()
            };

        let state1 = interprets(
            &[
                Instruction(Store, A, Const(0x0000_0000)),
                Instruction(Store, B, Reg(D)),
            ],
            &mut mem,
            state0
        );

        assert_eq!(state1, state0);

        assert_eq!(&mem, &[0xaabb_ccdd, 0xeeff_1111]);
    }

    #[test]
    fn interpret_cmp() {
        let mut mem = vec![];

        let state0 =
            State {
                a: 1,
                b: 2,
                c: 3,
                d: 2,
                ..State::default()
            };

        let fd = Flags::default();

        let cases = &[
            (A, Reg(B), Flags { cmp_l: true,  cmp_g: false, cmp_e: false, ..fd }),
            (C, Reg(B), Flags { cmp_l: false, cmp_g: true,  cmp_e: false, ..fd }),
            (D, Reg(B), Flags { cmp_l: false, cmp_g: false, cmp_e: true,  ..fd }),

            (A, Const(2), Flags { cmp_l: true,  cmp_g: false, cmp_e: false, ..fd }),
            (B, Const(2), Flags { cmp_l: false, cmp_g: false, cmp_e: true,  ..fd }),
            (C, Const(2), Flags { cmp_l: false, cmp_g: true,  cmp_e: false, ..fd }),
        ];

        for &(reg, op, flags) in cases {
            let state1 = interpret(Instruction(Cmp, reg, op), &mut mem, state0);

            assert_eq!(state1, State { flags: flags, ..state0 });
        }
    }

    #[test]
    fn interpret_branch() {
        let mut mem = vec![];

        let state0 = State { b: 0xdead_beef, ..State::default() };

        // register parameter shouldn't matter
        let state1 = interpret(Instruction(Branch, A, Const(0xabab_0202)), &mut mem, state0);
        let state2 = interpret(Instruction(Branch, C, Reg(B)), &mut mem, state1);

        assert_eq!(state1, State { ip: 0xabab_0202, ..state0 });
        assert_eq!(state2, State { ip: 0xdead_beef, ..state1 });
    }

    fn test_branch_flag<F>(f: Function, mut should_branch: F)
        where F: FnMut(Flags) -> bool {

        let mut mem = vec![];

        for flag_bits in 0x0..0x8 {
            let flags = Flags {
                cmp_l: flag_bits & 0x1 != 0,
                cmp_g: flag_bits & 0x2 != 0,
                cmp_e: flag_bits & 0x4 != 0,
                ..Flags::default()
            };

            let state0 = State { b: 0xf3f3_1a1a, flags: flags, ..State::default() };

            // register parameter shouldn't matter
            let state1 = interpret(Instruction(f, D, Const(0xeaea_0000)), &mut mem, state0);
            let state2 = interpret(Instruction(f, B, Reg(B)), &mut mem, state1);

            if should_branch(flags) {
                assert_eq!(state1, State { ip: 0xeaea_0000, ..state0 });
                assert_eq!(state2, State { ip: 0xf3f3_1a1a, ..state1 });
            } else {
                assert_eq!(state1, state0);
                assert_eq!(state2, state1);
            }
        }
    }

    #[test]
    fn interpret_branchl() {
        test_branch_flag(BranchL, |flags| flags.cmp_l);
    }

    #[test]
    fn interpret_branchg() {
        test_branch_flag(BranchG, |flags| flags.cmp_g);
    }

    #[test]
    fn interpret_branche() {
        test_branch_flag(BranchE, |flags| flags.cmp_e);
    }

    #[test]
    fn interpret_branchne() {
        test_branch_flag(BranchNE, |flags| !flags.cmp_e);
    }

    #[test]
    fn interpret_getsp() {
        let state0 = State { sp: 0x30, ..State::default() };
        let state1 = State { b: 0x30, ..state0 };

        memless_test(
            state0,
            &[
                Instruction(GetSp, B, Const(0))
            ],
            state1
        );
    }

    #[test]
    fn interpret_setsp() {
        let state0 = State { b: 0x100, ..State::default() };
        let state1 = State { sp: 0x100, ..state0 };


        memless_test(
            state0,
            &[
                Instruction(SetSp, A, Reg(B))
            ],
            state1
        );

        memless_test(
            state0,
            &[
                Instruction(SetSp, A, Const(0x100))
            ],
            state1
        );
    }

    #[test]
    fn interpret_push() {
        let mut mem = vec![0; 0x04];

        let state0 = State { a: 0xaaa, b: 0xf000_baaa, sp: 0x02, ..State::default() };

        let state1 = interprets(
            &[
                Instruction(Push, A, Reg(C)),
                Instruction(Push, B, Const(0xdead_beef)),
            ],
            &mut mem,
            state0
        );

        assert_eq!(state1, State { sp: 0x00, ..state0 });

        assert_eq!(&mem, &[0xf000_baaa, 0xaaa, 0, 0]);
    }

    #[test]
    fn interpret_pop() {
        let mut mem = vec![0xdeadbeef, 0xaaa, 0, 0];

        let mem_orig = mem.clone();

        let state0 = State { sp: 0x00, ..State::default() };

        let state1 = interprets(
            &[
                Instruction(Pop, A, Const(0)),
                Instruction(Pop, B, Const(0)),
            ],
            &mut mem,
            state0
        );

        assert_eq!(state1, State {
            a: 0xdead_beef,
            b: 0x0000_0aaa,
            sp: 0x02,
            ..state0
        });

        assert_eq!(mem, mem_orig);
    }

    #[test]
    fn interpret_call() {
        let mut mem = vec![0; 0x04];

        let state0 = State { a: 0xaaa, ip: 0xbbb, sp: 0x02, ..State::default() };

        let state1 = interprets(
            &[
                Instruction(Call, C, Reg(A)),
                Instruction(Call, D, Const(0xdead_beef)),
            ],
            &mut mem,
            state0
        );

        assert_eq!(state1, State { sp: 0x00, ip: 0xdead_beef, ..state0 });

        assert_eq!(&mem, &[0xaaa, 0xbbb, 0x0, 0x0]);
    }

    #[test]
    fn interpret_ret() {
        let mut mem = vec![0xdeadbeef, 0xaaa, 0, 0];

        let mem_orig = mem.clone();

        let state0 = State { sp: 0x00, ..State::default() };

        let state1 = interprets(
            &[
                Instruction(Ret, A, Const(0)),
            ],
            &mut mem,
            state0
        );

        assert_eq!(state1, State {
            ip: 0xdead_beef,
            sp: 0x01,
            ..state0
        });

        assert_eq!(mem, mem_orig);
    }

    #[test]
    fn interpret_add() {
        let state0 = State::default();
        let state1 = State { a: 5, b: 10, ..state0 };

        memless_test(
            state0,
            &[
                Instruction(Add, A, Const(5)),
                Instruction(Add, B, Reg(A)),
                Instruction(Add, B, Reg(B)),
            ],
            state1
        );
    }

    #[test]
    fn interpret_sub() {
        let state0 = State { a: 5, b: 10, ..State::default() };
        let state1 = State { a: 3, b: 7, ..state0 };

        memless_test(
            state0,
            &[
                Instruction(Sub, A, Const(2)),
                Instruction(Sub, B, Reg(A)),
            ],
            state1
        );
    }

    #[test]
    fn interpret_mul() {
        let state0 = State { a: 6, ..State::default() };
        let state1 = State { a: 144, ..state0 };

        memless_test(
            state0,
            &[
                Instruction(Mul, A, Const(2)),
                Instruction(Mul, A, Reg(A)),
            ],
            state1
        );
    }

    #[test]
    fn interpret_div() {
        let state0 = State { a: 100, b: 100, ..State::default() };
        let state1 = State { a: 50, b: 2, ..state0 };

        memless_test(
            state0,
            &[
                Instruction(Div, A, Const(2)),
                Instruction(Div, B, Reg(A)),
            ],
            state1
        );
    }

    #[test]
    fn interpret_divmod() {
        let state0 = State { a: 28, b: 9, ..State::default() };
        let state1 = State { c: 3, d: 1, ..state0 };

        memless_test(
            state0,
            &[
                Instruction(DivMod, A, Reg(B))
            ],
            state1
        );
    }

    #[test]
    fn interpret_not() {
        let state0 = State { d: 0xff00_ff00, a: 0x0000_0000, ..State::default() };
        let state1 = State { d: 0x00ff_00ff, a: 0xffff_ffff, ..state0 };

        memless_test(
            state0,
            &[
                Instruction(Not, A, Reg(B)), // Reg(B) should be ignored
                Instruction(Not, D, Const(0)),
            ],
            state1
        );
    }

    #[test]
    fn interpret_and() {
        let state0 = State { a: 0x0f0f_0f0f, b: 0x3232_3232, c: 0x3232_3232, ..State::default() };
        let state1 = State {                 b: 0x0202_0202, c: 0x3030_3030, ..state0 };

        memless_test(
            state0,
            &[
                Instruction(And, B, Reg(A)),
                Instruction(And, C, Const(0xf0f0_f0f0)),
            ],
            state1
        );
    }

    #[test]
    fn interpret_or() {
        let state0 = State { a: 0x1010_0101, b: 0x0101_0000, ..State::default() };
        let state1 = State { a: 0x1111_1111, b: 0x0101_1010, ..state0 };

        memless_test(
            state0,
            &[
                Instruction(Or, B, Const(0x0000_1010)),
                Instruction(Or, A, Reg(B)),
            ],
            state1
        );
    }

    #[test]
    fn interpret_xor() {
        let state0 = State { a: 0x1010_0101, b: 0x1111_1111, ..State::default() };
        let state1 = State { a: 0x0101_1010, b: 0x1010_0101, ..state0 };

        memless_test(
            state0,
            &[
                Instruction(Xor, A, Reg(B)),
                Instruction(Xor, B, Const(0x0101_1010)),
            ],
            state1
        );
    }

    #[test]
    fn interpret_lsh() {
        let state0 = State { a: 0x0000_1111, b: 4, ..State::default() };
        let state1 = State { a: 0x1110_0000, ..state0 };

        memless_test(
            state0,
            &[
                Instruction(Lsh, A, Reg(B)),
                Instruction(Lsh, A, Const(16)),
            ],
            state1
        );
    }

    #[test]
    fn interpret_rsh() {
        let state0 = State { a: 0x1110_0000, b: 4, ..State::default() };
        let state1 = State { a: 0x0000_0111, ..state0 };

        memless_test(
            state0,
            &[
                Instruction(Rsh, A, Reg(B)),
                Instruction(Rsh, A, Const(16)),
            ],
            state1
        );
    }

    #[test]
    fn interpret_halt() {
        let state0 = State { halt: false, ..State::default() };
        let state1 = State { halt: true, ..state0 };

        memless_test(
            state0,
            &[
                Instruction(Halt, A, Const(0))
            ],
            state1
        );
    }
}
