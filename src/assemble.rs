use std::str;
use std::collections::BTreeMap;

use nom::*;
use byteorder::*;

use data::*;
use bitcode;

type Label = String;

#[derive(Debug, Clone, PartialEq, Eq)]
enum AsmBlock {
    Instruction(AsmInstruction),
    Words(Vec<u32>),
    Bytes(AsmEndianness, Vec<u8>),
}

impl AsmBlock {
    fn size(&self) -> u32 {
        match *self {
            AsmBlock::Instruction(_)    => 2,
            AsmBlock::Words(ref vec)    => vec.len() as u32,
            AsmBlock::Bytes(_, ref vec) => {
                let len = vec.len() as u32;
                len / 4 + if len % 4 > 0 { 1 } else { 0 }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub fn assemble(code: &[u8], out: &mut Vec<u32>) -> Result<u32, String> {
    match asm_blocks_file( &code[..] ) {
        IResult::Done( b"", result ) =>
            blocks_to_instructions(&result, out),

        IResult::Error(verbose_errors::Err::Position(ekind, pos)) => {
            let s = String::from_utf8_lossy(pos);
            Err(format!("parser error near {:?}, unconsumed input: {}", ekind, s))
        },

        IResult::Error(e) => Err(format!("parser error: {:?}", e)),
        other_e => Err(format!("parser unexpected condition: {:?}", other_e))
    }
}

fn blocks_to_instructions(sections: &[(Label, Vec<AsmBlock>)], out: &mut Vec<u32>)
    -> Result<u32, String> {

    let mut current_ptr = 0_u32;

    let label_offsets: BTreeMap<Label, u32> =
        sections.iter().map(|&(ref label, ref blocks)| {
            let start = current_ptr;
            current_ptr += blocks.iter().map(|b| b.size()).sum();
            (label.clone(), start)
        }).collect();

    current_ptr = 0;

    let resolve = |current_ptr: u32, label: &str| -> Result<i32, String> {
        let off = *label_offsets.get(label)
            .ok_or_else(|| format!("Label not found: {}", label))?;

        Ok(off as i32 - current_ptr as i32)
    };

    for &(_, ref blocks) in sections {
        for block in blocks {
            match *block {
                AsmBlock::Instruction(AsmInstruction(f, r, ref op)) => {
                    let words = bitcode::encode_instruction(
                        Instruction(f, r.unwrap_or(Register::A), match *op {
                            Some(AsmOperand::Reg(r))           => Operand::Reg(r),
                            Some(AsmOperand::Const(c))         => Operand::Const(c),
                            Some(AsmOperand::Relative(cr))     => Operand::Relative(cr),
                            Some(AsmOperand::Label(ref label)) =>
                                Operand::Relative(resolve(current_ptr, label)?),
                            None => Operand::Const(0)
                        })
                    );

                    if current_ptr % 2 != 0 {
                        // Align to double-words.
                        //
                        // Have not decided whether this will be mandatory for instructions or not,
                        // but it should make it easier for debug tools.
                        out.push(0);
                        current_ptr += 1;
                    }

                    out.push(words.0);
                    out.push(words.1);

                    current_ptr += 2;
                },
                AsmBlock::Words(ref words) => {
                    out.extend(words.iter().cloned());

                    current_ptr += words.len() as u32;
                },
                AsmBlock::Bytes(endianness, ref bytes) => {
                    let mut iter = bytes.iter().cloned().fuse();

                    let mut ended = false;

                    while !ended {
                        let mut word_bytes = [0u32; 4];

                        for idx in 0..4 {
                            word_bytes[idx] = iter.next().unwrap_or_else(|| {
                                ended = true;
                                0
                            }) as u32;
                        }

                        if let AsmEndianness::Big = endianness {
                            out.push((word_bytes[3] <<  0) |
                                     (word_bytes[2] <<  8) |
                                     (word_bytes[1] << 16) |
                                     (word_bytes[0] << 24));
                        } else {
                            out.push((word_bytes[0] <<  0) |
                                     (word_bytes[1] <<  8) |
                                     (word_bytes[2] << 16) |
                                     (word_bytes[3] << 24));
                        }

                        current_ptr += 1;
                    }
                }
            }
        }
    }

    Ok(current_ptr)
}

named!(asm_blocks_file<&[u8], Vec<(Label, Vec<AsmBlock>)>>,
    do_parse!(
        opt!(complete!(asm_multispace)) >>
        blocks: asm_blocks >>
        eof!() >>

        ( blocks )
    )
);

named!(asm_blocks<&[u8], Vec<(Label, Vec<AsmBlock>)>>,
    many0!(asm_block_pair)
);

named!(asm_block_pair<&[u8], (Label, Vec<AsmBlock>)>,
    do_parse!(
        not!(peek!(preceded!(asm_multispace, eof!()))) >>
        label_opt: opt!(complete!(label_def)) >>
        blocks: many0!(asm_block) >>

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
        lb: label >>
        one_of!(":") >>
        opt!(complete!(asm_multispace)) >>
        ( lb )
    )
);

named!(asm_block<&[u8], AsmBlock>,
    do_parse!(
        block: alt_complete!(
            asm_block_directive |
            map!(asm_instruction, AsmBlock::Instruction) 
        ) >>
        opt!(complete!(space)) >>
        opt!(complete!(comment)) >>
        alt_complete!(eof!() | line_ending) >>
        opt!(complete!(asm_multispace)) >>

        (block)
    )
);

fn is_alphanumeric_underscore(c: u8) -> bool {
    (c >= b'0' && c <= b'9') ||
    (c >= b'A' && c <= b'Z') ||
    (c >= b'a' && c <= b'z') ||
    (c == b'_')
}

named!(alphanumeric_underscore,
    take_while1!(is_alphanumeric_underscore)
);

named!(asm_block_directive<&[u8], AsmBlock>,
    preceded!(tag!("."),
        switch!(terminated!(alphanumeric_underscore, space),
            b"len_words" => call!(dir_len_words) |
            b"words"     => call!(dir_words) |
            b"len_bytes" => call!(dir_len_bytes) |
            b"bytes"     => call!(dir_bytes)
        )
    )
);

named!(dir_words<&[u8], AsmBlock>,
    map!(delimited!(one_of!("{"),
                    separated_list!(one_of!(","), ws!(constant)),
                    one_of!("}")),
         AsmBlock::Words)
);

named!(dir_len_words<&[u8], AsmBlock>,
    map!(dir_words, |block| {
        let mut words = match block {
            AsmBlock::Words(words) => words,
            _ => unreachable!()
        };

        let len = words.len() as u32;

        words.insert(0, len);
        AsmBlock::Words(words)
    })
);

named!(dir_bytes<&[u8], AsmBlock>,
    do_parse!(
        endianness: alt_complete!(
            tag_no_case!("be") => { |_| AsmEndianness::Big    } |
            tag_no_case!("le") => { |_| AsmEndianness::Little }
        ) >>
        s: string >>
        
        ( AsmBlock::Bytes(endianness, s) )
    )
);

named!(dir_len_bytes<&[u8], AsmBlock>,
    map!(dir_bytes, |block| {
        let (endianness, bytes) = match block {
            AsmBlock::Bytes(endianness, bytes) => (endianness, bytes),
            _ => unreachable!()
        };

        let len = bytes.len() as u32;

        let mut buf = vec![0; 4];

        match endianness {
            AsmEndianness::Big    => BigEndian::write_u32(   &mut buf[..], len),
            AsmEndianness::Little => LittleEndian::write_u32(&mut buf[..], len),
        }

        buf.extend(bytes);
        AsmBlock::Bytes(endianness, buf)
    })
);

fn is_string_safe(c: u8) -> bool {
    c != b'\"' && c != b'\\'
}

named!(string_safe_bytes, take_while1!(is_string_safe));

named!(string<&[u8], Vec<u8>>,
    delimited!(one_of!("\""), escaped_transform!(string_safe_bytes, b'\\',
        alt_complete!(
            tag!("\\") => { |_| &b"\\"[..] } |
            tag!("\"") => { |_| &b"\""[..] } |
            tag!("n")  => { |_| &b"\n"[..] } |
            tag!("r")  => { |_| &b"\r"[..] }
        )
    ), one_of!("\""))
);

named!(asm_instruction<&[u8], AsmInstruction>,
    do_parse!(
        function: function >>

        register: opt!(complete!(preceded!(space, register))) >>

        operand: opt!(complete!(preceded!(opt!(space),
            delimited!(tag!("["), ws!(operand), tag!("]"))))) >>

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

            "halt"     => Halt,
            "intsw"    => IntSw,
            "inthw"    => IntHw,
            "intpause" => IntPause,
            "intcont"  => IntCont,
            "inthget"  => IntHGet,
            "inthset"  => IntHSet,
            "intexit"  => IntExit,

            "trace"    => Trace,

            _ => return None
        })
    })
);

named!(register<&[u8], Register>,
    do_parse!(
        ch: map_opt!(anychar, |c: char| {
            match c {
                'a' | 'A' => Some(Register::A),
                'b' | 'B' => Some(Register::B),
                'c' | 'C' => Some(Register::C),
                'd' | 'D' => Some(Register::D),
                _ => None
            }
        }) >>
        not!(peek!(alphanumeric)) >>
        ( ch )
    )
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
    take_while1!(is_binary)
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

fn is_not_newline(c: u8) -> bool { c != b'\n' }

named!(comment,
    preceded!(one_of!(";"), take_while!(is_not_newline))
);

static EMPTY_BYTES: &'static [u8] = &[];

named!(asm_multispace,
    fold_many0!(alt_complete!(multispace | comment), EMPTY_BYTES, |_, _| EMPTY_BYTES)
);

#[cfg(test)]
static FIBONACCI_CODE: &'static [u8] = br#"
; This is a comment
fibonacci:
    cmp a [0]
    branchl [fibonacci.bad] ; another test comment
    branche [fibonacci.ret]
    cmp a [1]
    branche [fibonacci.ret]

    push a
    sub a [1]
    call [fibonacci]

    ; comments should work anywhere, basically

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

; including here
"#;

#[test]
fn test_fibonacci() {
    let code = FIBONACCI_CODE;

    println!("{:?}", String::from_utf8_lossy(code));

    match asm_blocks_file( &code[..] ) {
        IResult::Done( b"", result ) => {
            println!("{:#?}", result);

            let mut out = vec![];

            blocks_to_instructions(&result, &mut out).unwrap();

            println!("{:?}", out);

            assert_eq!(out.len(), 17 * 2);
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
