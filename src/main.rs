#[macro_use]
extern crate lazy_static;

pub mod data;
pub mod interpret;
pub mod bitcode;
pub mod machine;

use data::*;
use machine::Machine;

fn main() {
    use data::Function::*;
    use data::Register::*;
    use data::Operand::*;

    let instructions = &[
        Instruction(Set, A, Const(5)),
        Instruction(Mul, A, Const(5)),
        Instruction(Set, B, Const(16)),
        Instruction(Store, A, Reg(B)),
        Instruction(Load, C, Reg(B)),
        Instruction(Branch, A, Const(0)),
    ];

    let mut machine = Machine::new(0x200);

    machine.state.sp = 0x80;
    machine.state.ip = 0x100;

    machine.store_instructions(0x100, instructions);

    machine.trace_until_zero();

    println!("{:?}", machine.mem);
    println!("{:#?}", machine.state);

    assert_eq!(machine.state.c, 25);
}
