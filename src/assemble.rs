use std::str;

use nom::*;

use data::*;

type Label = String;

#[derive(Debug, Clone, PartialEq, Eq)]
enum AsmBlock {
    Instruction(AsmInstruction),
    Words(Vec<u32>),
    Bytes(AsmEndianness, Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AsmEndianness {
    Big,
    Little,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AsmInstruction(Function, Option<Register>, Option<AsmOperand>);

#[derive(Debug, Clone, PartialEq, Eq)]
enum AsmOperand {
    Reg(Register),
    Const(u32),
    Relative(i32),
    Label(Label)
}

named!(asm_blocks_file<&[u8], Vec<(Label, Vec<AsmBlock>)>>,
    do_parse!(
        blocks: asm_blocks >>
        opt!(complete!(multispace)) >>
        eof!() >>

        ( blocks )
    )
);

named!(asm_blocks<&[u8], Vec<(Label, Vec<AsmBlock>)>>,
    many0!(asm_block_pair)
);

named!(asm_block_pair<&[u8], (Label, Vec<AsmBlock>)>,
    do_parse!(
        label_opt: opt!(complete!(label_def)) >>
        blocks: many1!(complete!(asm_block)) >>

        ( label_opt.unwrap_or_else(|| String::new()), blocks )
    )
);

named!(label_initial_char<&[u8], char>,
    verify!(anychar, |c: char| c == '_' || c.is_alpha())
);

named!(label_final_char<&[u8], char>,
    verify!(anychar, |c: char| c == '_' || c == '.' || c.is_alphanum())
);

named!(label<&[u8], Label>,
    do_parse!(
        initial: label_initial_char >>
        full: fold_many0!(label_final_char, initial.to_string(), |mut o: String, b: char| {
            o.push(b);
            o
        }) >>

        ( full )
    )
);

named!(label_def<&[u8], Label>,
    do_parse!(
        opt!(multispace) >>
        lb: label >>
        one_of!(":") >>
        ( lb )
    )
);

named!(asm_block<&[u8], AsmBlock>,
    do_parse!(
        opt!(complete!(multispace)) >>
        inst: map!(asm_instruction, AsmBlock::Instruction) >>
        opt!(complete!(space)) >>
        alt_complete!(eof!() | line_ending) >>

        (inst)
    )
);

named!(asm_instruction<&[u8], AsmInstruction>,
    do_parse!(
        function: function >>

        register: opt!(complete!(preceded!(space, register))) >>

        operand: opt!(complete!(preceded!(opt!(space),
            delimited!(tag!("["), operand, tag!("]"))))) >>

        (AsmInstruction(function, register, operand))
    )
);

named!(function<&[u8], Function>,
    map_opt!(alpha, |code: &[u8]| {
        use data::Function::*;

        let lowercase = match str::from_utf8(code) {
            Ok(s) => s,
            Err(_) => return None
        }.to_lowercase();

        Some(match &lowercase[..] {
            "bad"      => Bad,

            "nop"      => Nop,
            "set"      => Set,
            "load"     => Load,
            "store"    => Store,

            "cmp"      => Cmp,
            "branch"   => Branch,
            "branchl"  => BranchL,
            "branchg"  => BranchG,
            "branche"  => BranchE,
            "branchne" => BranchNE,

            "getsp"    => GetSp,
            "setsp"    => SetSp,
            "push"     => Push,
            "pop"      => Pop,
            "call"     => Call,
            "ret"      => Ret,

            "add"      => Add,
            "sub"      => Sub,
            "mul"      => Mul,
            "div"      => Div,
            "divmod"   => DivMod,

            "not"      => Not,
            "and"      => And,
            "or"       => Or,
            "xor"      => Xor,
            "lsh"      => Lsh,
            "rsh"      => Rsh,

            _ => return None
        })
    })
);

named!(register<&[u8], Register>,
    map_opt!(anychar, |c: char| {
        match c {
            'a' | 'A' => Some(Register::A),
            'b' | 'B' => Some(Register::B),
            'c' | 'C' => Some(Register::C),
            'd' | 'D' => Some(Register::D),
            _ => None
        }
    })
);

named!(operand<&[u8], AsmOperand>,
    alt_complete!(
        map!(register, AsmOperand::Reg) |
        map!(relative, AsmOperand::Relative) |
        map!(constant, AsmOperand::Const) |
        map!(label, AsmOperand::Label)
    )
);

named!(relative<&[u8], i32>,
    do_parse!(
        one_of!("$") >>
        opt!(space) >>
        op: one_of!("+-") >>
        opt!(space) >>
        num: constant >>

        (match op {
            '+' => num as i32,
            '-' => -(num as i32),
            _ => unreachable!()
        })
    )
);

named!(constant<&[u8], u32>,
    do_parse!(
        sign: opt!(one_of!("-+")) >>
        num: alt_complete!(c_hex | c_binary | c_octal | c_decimal) >>

        (match sign {
            Some('+') | None => num,
            Some('-') => (-(num as i32)) as u32,
            _ => unimplemented!()
        })
    )
);

named!(c_hex<&[u8], u32>,
    preceded!(tag!("0x"), hex_u32)
);

fn is_binary(c: u8) -> bool {
    c == b'0' || c == b'1'
}

named!(binary_digit,
    take_while!(is_binary)
);

named!(c_binary<&[u8], u32>,
    map_opt!(preceded!(tag!("0b"), binary_digit), |digits| {
        str::from_utf8(digits).ok()
            .and_then(|s| u32::from_str_radix(s, 2).ok())
    })
);

named!(c_octal<&[u8], u32>,
    map_opt!(preceded!(tag!("0"), oct_digit), |digits| {
        str::from_utf8(digits).ok()
            .and_then(|s| u32::from_str_radix(s, 8).ok())
    })
);

named!(c_decimal<&[u8], u32>,
    map_opt!(digit, |digits| {
        str::from_utf8(digits).ok()
            .and_then(|s| u32::from_str_radix(s, 10).ok())
    })
);

#[test]
fn test_fibonacci() {
    let code = br#"
fibonacci:
    cmp a [0]
    branchl [fibonacci.bad]
    branche [fibonacci.ret]
    cmp a [1]
    branche [fibonaaci.ret]

    push a
    sub a [1]
    call [fibonacci]

    pop b
    push a
    sub b [2]
    set a [b]
    call [fibonacci]

    pop b
    add a [b]
fibonacci.ret:
    ret
fibonacci.bad:
    bad
"#;

    println!("{:?}", String::from_utf8_lossy(code));

    match asm_blocks_file( &code[..] ) {
        IResult::Done( b"", result ) => {
            println!("{:#?}", result);
        },
        e => panic!("{:?}", e)
    }
}

#[test]
fn test_asm_instruction_none() {
    assert_eq!(
        asm_instruction( &b"bad"[..] ),
        IResult::Done( &b""[..], AsmInstruction(
                Function::Bad, None, None))
    );
}

#[test]
fn test_asm_instruction_reg() {
    assert_eq!(
        asm_instruction( &b"bad a"[..] ),
        IResult::Done( &b""[..], AsmInstruction(
                Function::Bad, Some(Register::A), None))
    );
}

#[test]
fn test_asm_instruction_op() {
    assert_eq!(
        asm_instruction( &b"bad [b]"[..] ),
        IResult::Done( &b""[..], AsmInstruction(
                Function::Bad, None, Some(AsmOperand::Reg(Register::B))))
    );
}

#[test]
fn test_asm_instruction_reg_op() {
    assert_eq!(
        asm_instruction( &b"bad a [b]"[..] ),
        IResult::Done( &b""[..], AsmInstruction(
                Function::Bad, Some(Register::A), Some(AsmOperand::Reg(Register::B))))
    );
}

#[test]
fn test_operand_reg() {
    assert_eq!(
        operand( &b"a"[..] ),
        IResult::Done( &b""[..], AsmOperand::Reg(Register::A) )
    );
}

#[test]
fn test_operand_const_hex() {
    assert_eq!(
        operand( &b"0x20"[..] ),
        IResult::Done( &b""[..], AsmOperand::Const(0x20) )
    );
}

#[test]
fn test_operand_const_bin() {
    assert_eq!(
        operand( &b"0b11011"[..] ),
        IResult::Done( &b""[..], AsmOperand::Const(0b11011) )
    );
}

#[test]
fn test_operand_const_oct() {
    assert_eq!(
        operand( &b"0755"[..] ),
        IResult::Done( &b""[..], AsmOperand::Const(0b111101101) )
    );
}

#[test]
fn test_operand_const_dec() {
    assert_eq!(
        operand( &b"322"[..] ),
        IResult::Done( &b""[..], AsmOperand::Const(322) )
    );
}

#[test]
fn test_operand_const_dec_neg() {
    assert_eq!(
        operand( &b"-420"[..] ),
        IResult::Done( &b""[..], AsmOperand::Const(-420i32 as u32) )
    );
}

#[test]
fn test_operand_relative_pos() {
    assert_eq!(
        operand( &b"$ + 0x20"[..] ),
        IResult::Done( &b""[..], AsmOperand::Relative(0x20) )
    );
}

#[test]
fn test_operand_relative_neg() {
    assert_eq!(
        operand( &b"$ - 4"[..] ),
        IResult::Done( &b""[..], AsmOperand::Relative(-4) )
    );
}

#[test]
fn test_operand_label() {
    assert_eq!(
        operand( &b"FooFunction"[..] ),
        IResult::Done( &b""[..], AsmOperand::Label("FooFunction".into()) )
    );
}
