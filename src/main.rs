pub mod data;
pub mod interpret;

use data::*;
use interpret::*;

fn main() {
    use data::Function::*;
    use data::Register::*;
    use data::Operand::*;

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
