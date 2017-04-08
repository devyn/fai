extern crate fai;

use std::io;
use std::io::prelude::*;

use fai::assemble::assemble;
use fai::bitcode;

fn main() {
    let mut buffer = vec![];

    io::stdin().read_to_end(&mut buffer).unwrap();

    let mut bitcode = vec![];

    assemble(&buffer, &mut bitcode).unwrap();

    let mut current_ptr = 0;

    while current_ptr < bitcode.len() {
        print!("{:08x}    {:08x}", current_ptr, bitcode[current_ptr]);

        if current_ptr + 1 < bitcode.len() {
            print!(" {:08x}", bitcode[current_ptr + 1]);

            let inst = bitcode::decode_instruction(
                (bitcode[current_ptr], bitcode[current_ptr + 1]));

            print!("    {:?}", inst);
        } else {
            print!(" {:8}", "");
        }

        println!();

        current_ptr += 2;
    }
}
