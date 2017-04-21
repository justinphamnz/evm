use utils::bigint::{M256, MI256};
use utils::gas::Gas;
use utils::address::Address;
use super::Opcode;
use vm::{Machine, Memory, Stack, PC};
use transaction::Transaction;
use blockchain::Block;

use std::ops::{Add, Sub, Not, Mul, Div, Shr, Shl, BitAnd, BitOr, BitXor, Rem};
use crypto::sha3::Sha3;
use crypto::digest::Digest;

fn signed_abs(v: M256) -> M256 {
    let negative: M256 = M256::one() << 256;

    if v >= negative {
        !v + 1.into()
    } else {
        v
    }
}

macro_rules! op2 {
    ( $machine:expr, $op:ident ) => ({
        let op1 = $machine.stack_mut().pop();
        let op2 = $machine.stack_mut().pop();
        $machine.stack_mut().push(op1.$op(op2));
    })
}

macro_rules! op2_ref {
    ( $machine:expr, $op:ident ) => ({
        let op1 = $machine.stack_mut().pop();
        let op2 = $machine.stack_mut().pop();
        $machine.stack_mut().push(op1.$op(&op2).into());
    })
}

impl Opcode {
    pub fn run<M: Machine>(&self, machine: &mut M) {
        let opcode = self.clone();

        match opcode {
            Opcode::STOP => {
                machine.pc_mut().stop();
            },

            Opcode::ADD => op2!(machine, add),
            Opcode::MUL => op2!(machine, mul),
            Opcode::SUB => op2!(machine, sub),
            Opcode::DIV => op2!(machine, div),

            Opcode::SDIV => {
                let op1: MI256 = machine.stack_mut().pop().into();
                let op2: MI256 = machine.stack_mut().pop().into();
                let r = op1 / op2;
                machine.stack_mut().push(r.into());
            },

            Opcode::MOD => op2!(machine, rem),

            Opcode::SMOD => {
                let op1: MI256 = machine.stack_mut().pop().into();
                let op2: MI256 = machine.stack_mut().pop().into();
                let r = op1 % op2;
                machine.stack_mut().push(r.into());
            },

            Opcode::ADDMOD => {
                let op1 = machine.stack_mut().pop();
                let op2 = machine.stack_mut().pop();
                let op3 = machine.stack_mut().pop();

                if op3 == 0.into() {
                    machine.stack_mut().push(0.into());
                } else {
                    let v = op1 + op2;
                    machine.stack_mut().push(v - (v / op3) * op3);
                }
            },

            Opcode::MULMOD => {
                let op1 = machine.stack_mut().pop();
                let op2 = machine.stack_mut().pop();
                let op3 = machine.stack_mut().pop();

                if op3 == 0.into() {
                    machine.stack_mut().push(0.into());
                } else {
                    let v = op1 * op2;
                    machine.stack_mut().push(v - (v / op3) * op3);
                }
            },

            Opcode::EXP => {
                let op1 = machine.stack_mut().pop();
                let mut op2 = machine.stack_mut().pop();
                let mut r: M256 = 1.into();

                while op2 != 0.into() {
                    r = r * op1;
                    op2 = op2 - 1.into();
                }
                machine.stack_mut().push(r);
            },

            Opcode::SIGNEXTEND => {
                // TODO: Check this confines with the yello paper

                let mut op1 = machine.stack_mut().pop();
                let mut op2 = machine.stack_mut().pop();

                let mut negative: M256 = 1.into();
                let mut s = 0;
                while op2 != 0.into() {
                    negative = M256::one() << s;
                    s = s + 1;
                    op2 = op2 - 1.into();
                }

                if op1 >= negative {
                    while s <= 256 {
                        op1 = op1 + (M256::one() << s);
                        s = s + 1;
                    }
                    machine.stack_mut().push(op1);
                } else {
                    machine.stack_mut().push(op1);
                }
            },

            Opcode::LT => op2_ref!(machine, lt),
            Opcode::GT => op2_ref!(machine, gt),

            Opcode::SLT => {
                let negative = M256::one() << 256;

                let op1 = machine.stack_mut().pop();
                let op2 = machine.stack_mut().pop();

                if op1 < negative && op2 < negative {
                    if op1 < op2 {
                        machine.stack_mut().push(1.into());
                    } else {
                        machine.stack_mut().push(0.into());
                    }
                } else if op2 >= negative && op2 >= negative {
                    if op1 < op2 {
                        machine.stack_mut().push(0.into());
                    } else {
                        machine.stack_mut().push(1.into());
                    }
                } else if op1 < negative && op2 >= negative {
                    machine.stack_mut().push(0.into());
                } else {
                    machine.stack_mut().push(1.into());
                }
            },

            Opcode::SGT => {
                let negative = M256::one() << 256;

                let op1 = machine.stack_mut().pop();
                let op2 = machine.stack_mut().pop();

                if op1 < negative && op2 < negative {
                    if op1 < op2 {
                        machine.stack_mut().push(0.into());
                    } else {
                        machine.stack_mut().push(1.into());
                    }
                } else if op2 >= negative && op2 >= negative {
                    if op1 < op2 {
                        machine.stack_mut().push(1.into());
                    } else {
                        machine.stack_mut().push(0.into());
                    }
                } else if op1 < negative && op2 >= negative {
                    machine.stack_mut().push(1.into());
                } else {
                    machine.stack_mut().push(0.into());
                }
            },

            Opcode::EQ => op2_ref!(machine, eq),

            Opcode::ISZERO => {
                let op1 = machine.stack_mut().pop();

                if op1 == 0.into() {
                    machine.stack_mut().push(1.into());
                } else {
                    machine.stack_mut().push(0.into());
                }
            },

            Opcode::AND => op2!(machine, bitand),
            Opcode::OR => op2!(machine, bitor),
            Opcode::XOR => op2!(machine, bitxor),

            Opcode::NOT => {
                let op1 = machine.stack_mut().pop();

                machine.stack_mut().push(!op1);
            },

            Opcode::BYTE => {
                let op1 = machine.stack_mut().pop();
                let op2: usize = machine.stack_mut().pop().into(); // 256 / 8
                let mark: M256 = 0xff.into();

                if op2 >= 256 / 8 {
                    machine.stack_mut().push(0.into());
                } else {
                    machine.stack_mut().push((op1 >> (op2 * 8)) & mark);
                }
            },

            Opcode::SHA3 => {
                let mut op1 = machine.stack_mut().pop();
                let op2 = machine.stack_mut().pop();

                let mut r: [u8; 32] = [0u8; 32];
                let mut sha3 = Sha3::keccak256();

                while op1 != op2 - 1.into() {
                    let val = machine.memory_mut().read(op1);
                    let a: [u8; 32] = val.into();
                    sha3.input(a.as_ref());
                    op1 = op1 + 1.into();
                }
                sha3.result(&mut r);
                machine.stack_mut().push(M256::from(r.as_ref()))
            },

            Opcode::ADDRESS => {
                let address = machine.transaction().callee();
                machine.stack_mut().push(address.into());
            },

            Opcode::BALANCE => {
                let address: Option<Address> = machine.stack_mut().pop().into();
                let balance = address.map_or(None, |address| {
                    machine.block().balance(address)
                }).map_or(M256::zero(), |balance| balance.into());
                machine.stack_mut().push(balance);
            },

            Opcode::ORIGIN => {
                let address = machine.transaction().originator();
                machine.stack_mut().push(address.into());
            },

            Opcode::CALLER => {
                let address = machine.transaction().sender();
                machine.stack_mut().push(address.into());
            },

            Opcode::CALLVALUE => {
                let value = machine.transaction().value();
                machine.stack_mut().push(value);
            },

            Opcode::CALLDATALOAD => {
                let start_index: usize = machine.stack_mut().pop().into();
                let load = M256::from(&machine.transaction().data()
                                      .unwrap()[start_index..start_index+32]);
                machine.stack_mut().push(load);
            },

            Opcode::CALLDATASIZE => {
                let len = machine.transaction().data().map_or(0, |s| s.len());
                machine.stack_mut().push(len.into());
            },

            Opcode::CALLDATACOPY => {
                let memory_index = machine.stack_mut().pop();
                let data_index: usize = machine.stack_mut().pop().into();
                let len: usize = machine.stack_mut().pop().into();

                for i in 0..len {
                    let val = machine.transaction().data().unwrap()[data_index + i];
                    machine.memory_mut().write_raw(memory_index + i.into(), val);
                }
            },

            Opcode::CODESIZE => {
                let len = machine.pc().code().len();
                machine.stack_mut().push(len.into());
            },

            Opcode::CODECOPY => {
                let memory_index = machine.stack_mut().pop();
                let code_index: usize = machine.stack_mut().pop().into();
                let len: usize = machine.stack_mut().pop().into();

                for i in 0..len {
                    let val = machine.pc().code()[code_index + i];
                    machine.memory_mut().write_raw(memory_index + i.into(), val);
                }
            },

            Opcode::GASPRICE => {
                let price: M256 = machine.transaction().gas_price().into();
                machine.stack_mut().push(price);
            },

            Opcode::EXTCODESIZE => {
                let account: Option<Address> = machine.stack_mut().pop().into();
                let account = account.unwrap();
                let len = machine.block().account_code(account).map_or(0, |s| s.len());
                machine.stack_mut().push(len.into());
            },

            Opcode::EXTCODECOPY => {
                let account: Option<Address> = machine.stack_mut().pop().into();
                let account = account.unwrap();
                let memory_index = machine.stack_mut().pop();
                let code_index: usize = machine.stack_mut().pop().into();
                let len: usize = machine.stack_mut().pop().into();

                for i in 0..len {
                    let val = machine.block().account_code(account).unwrap()[code_index + i];
                    machine.memory_mut().write_raw(memory_index + i.into(), val);
                }
            },

            Opcode::BLOCKHASH => {
                let target = machine.stack_mut().pop();
                let val = machine.block().blockhash(target);
                machine.stack_mut().push(val.into());
            },

            Opcode::COINBASE => {
                let val = machine.block().coinbase();
                machine.stack_mut().push(val.into());
            },

            Opcode::TIMESTAMP => {
                let val = machine.block().timestamp();
                machine.stack_mut().push(val.into());
            },

            Opcode::NUMBER => {
                let val = machine.block().number();
                machine.stack_mut().push(val.into());
            },

            Opcode::DIFFICULTY => {
                let val = machine.block().difficulty();
                machine.stack_mut().push(val.into());
            },

            Opcode::GASLIMIT => {
                let val = machine.block().gas_limit();
                machine.stack_mut().push(val.into());
            },

            Opcode::POP => {
                machine.stack_mut().pop();
            },

            Opcode::MLOAD => {
                let op1 = machine.stack_mut().pop();
                let val = machine.memory_mut().read(op1);
                // u_i update is automatically handled by Memory.
                machine.stack_mut().push(val);
            },

            Opcode::MSTORE => {
                let op1 = machine.stack_mut().pop(); // Index
                let op2 = machine.stack_mut().pop(); // Data
                // u_i update is automatically handled by Memory.
                machine.memory_mut().write(op1, op2);
            },

            Opcode::MSTORE8 => {
                let op1 = machine.stack_mut().pop(); // Index
                let op2 = machine.stack_mut().pop(); // Data
                let a: [u8; 32] = op2.into();
                let val = a[31];
                machine.memory_mut().write_raw(op1, val);
            },

            Opcode::SLOAD => {
                let op1 = machine.stack_mut().pop();
                let from = machine.transaction().callee();
                let val = machine.block().account_storage(from, op1);
                machine.stack_mut().push(val);
            },

            Opcode::SSTORE => {
                let op1 = machine.stack_mut().pop(); // Index
                let op2 = machine.stack_mut().pop(); // Data
                let from = machine.transaction().callee();
                machine.block_mut().set_account_storage(from, op1, op2);
            }

            Opcode::JUMP => {
                let op1_u: u64 = machine.stack_mut().pop().into();
                machine.pc_mut().jump(op1_u as usize);
            },

            Opcode::JUMPI => {
                let op1_u: u64 = machine.stack_mut().pop().into();
                let op2 = machine.stack_mut().pop();

                if op2 != 0.into() {
                    machine.pc_mut().jump(op1_u as usize);
                }
            },

            Opcode::PC => {
                let position = machine.pc().position();
                machine.stack_mut().push((position - 1).into()); // PC increment for opcode is always an u8.
            },

            Opcode::MSIZE => {
                let active_len = machine.memory().active_len();
                machine.stack_mut().push(active_len.into());
            },

            Opcode::GAS => {
                let gas: M256 = machine.transaction().gas_limit().into();
                machine.stack_mut().push(gas);
            },

            Opcode::JUMPDEST => (), // This operation has no effect on machine state during execution.

            Opcode::PUSH(v) => {
                let val = machine.pc_mut().read(v);
                machine.stack_mut().push(val);
            },

            Opcode::DUP(v) => {
                let val = machine.stack().peek(v - 1);
                machine.stack_mut().push(val);
            },

            Opcode::SWAP(v) => {
                let val1 = machine.stack().peek(0);
                let val2 = machine.stack().peek(v);
                machine.stack_mut().set(0, val2);
                machine.stack_mut().set(v, val1);
            },

            Opcode::LOG(v) => {
                let address = machine.transaction().callee();
                let mut data: Vec<u8> = Vec::new();
                let start = machine.stack_mut().pop();
                let len: usize = machine.stack_mut().pop().into();

                for i in 0..len {
                    data.push(machine.memory_mut().read_raw(start + i.into()));
                }

                let mut topics: Vec<M256> = Vec::new();

                for i in 0..v {
                    topics.push(machine.stack_mut().pop());
                }

                machine.block_mut().log(address, data.as_ref(), topics.as_ref());
            },

            Opcode::CREATE => {
                // TODO: Register the transaction for its value.
                let value = machine.stack_mut().pop();
                let start: usize = machine.stack_mut().pop().into();
                let len: usize = machine.stack_mut().pop().into();
                let code: Vec<u8> = machine.pc().code()[start..(start + len)].into();
                let address = machine.block_mut().create_account(code.as_ref());
                machine.stack_mut().push(address.unwrap().into());
            },

            Opcode::CALL => {
                let gas: Gas = machine.stack_mut().pop().into();
                let from = machine.transaction().callee();
                let to: Option<Address> = machine.stack_mut().pop().into();
                let to = to.unwrap();
                let value = machine.stack_mut().pop().into();
                let memory_in_start = machine.stack_mut().pop();
                let memory_in_len = machine.stack_mut().pop();
                let memory_out_start = machine.stack_mut().pop();
                let memory_out_len = machine.stack_mut().pop();

                machine.fork(gas, from, to, value, memory_in_start, memory_in_len,
                             memory_out_start, memory_out_len, |machine| {
                                 machine.fire();
                             });

                machine.stack_mut().push(M256::zero());
            },

            Opcode::CALLCODE => {
                let gas: Gas = machine.stack_mut().pop().into();
                machine.stack_mut().pop();
                let from = machine.transaction().callee();
                let to = machine.transaction().callee();
                let value = machine.stack_mut().pop().into();
                let memory_in_start = machine.stack_mut().pop();
                let memory_in_len = machine.stack_mut().pop();
                let memory_out_start = machine.stack_mut().pop();
                let memory_out_len = machine.stack_mut().pop();

                machine.fork(gas, from, to, value, memory_in_start, memory_in_len,
                             memory_out_start, memory_out_len, |machine| {
                                 machine.fire();
                             });

                machine.stack_mut().push(M256::zero());
            },

            Opcode::RETURN => {
                let start = machine.stack_mut().pop();
                let len: usize = machine.stack_mut().pop().into();
                let mut vec: Vec<u8> = Vec::new();

                for i in 0..len {
                    vec.push(machine.memory_mut().read_raw(start + i.into()));
                }

                machine.set_return_values(vec.as_ref());
                machine.pc_mut().stop();
            },

            Opcode::DELEGATECALL => {
                let gas: Gas = machine.stack_mut().pop().into();
                let from = machine.transaction().sender();
                let to: Option<Address> = machine.stack_mut().pop().into();
                let to = to.unwrap();
                let value = machine.transaction().value();
                let memory_in_start = machine.stack_mut().pop();
                let memory_in_len = machine.stack_mut().pop();
                let memory_out_start = machine.stack_mut().pop();
                let memory_out_len = machine.stack_mut().pop();

                machine.fork(gas, from, to, value, memory_in_start, memory_in_len,
                             memory_out_start, memory_out_len, |machine| {
                                 machine.fire();
                             });

                machine.stack_mut().push(M256::zero());
            },

            Opcode::SUICIDE => {
                machine.stack_mut().pop();
                machine.pc_mut().stop();
            },

            Opcode::INVALID => {
                machine.pc_mut().stop();
            }
        }
    }
}
