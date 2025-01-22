use crate::messages::{COINBASE_OUTPOINT_HASH, COINBASE_OUTPOINT_INDEX};
use crate::util::{sha256d, var_int, Hash256, Result, Serializable};
use bitcoin::witness;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fmt;
use std::io;
use std::io::{Read, Write};

use super::tx_in::TxIn;
use super::tx_out::TxOut;
use super::witness::TxWitness;
use super::Payload;

/// Maximum number of satoshis possible
pub const MAX_SATOSHIS: i64 = 21_000_000 * 100_000_000;

/// Bitcoin transaction
#[derive(Default, PartialEq, Eq, Hash, Clone)]
pub struct Tx {
    /// Transaction version
    pub version: u32,
    /// Flags
    pub flag: Option<u8>,
    /// Transaction inputs
    pub inputs: Vec<TxIn>,
    /// Transaction outputs
    pub outputs: Vec<TxOut>,
    /// Transaction Witnesses
    pub witnesses: Option<Vec<TxWitness>>,
    /// The block number or timestamp at which this transaction is unlocked
    pub lock_time: u32,
}

impl Tx {
    /// Calculates the hash of the transaction also known as the txid
    pub fn hash(&self) -> Hash256 {
        let mut b = Vec::with_capacity(self.size());
        self.write(&mut b).unwrap();
        sha256d(&b)
    }

    // /// Validates a non-coinbase transaction
    // pub fn validate(
    //     &self,
    //     require_sighash_forkid: bool,
    //     use_genesis_rules: bool,
    //     utxos: &LinkedHashMap<OutPoint, TxOut>,
    //     pregenesis_outputs: &HashSet<OutPoint>,
    // ) -> Result<()> {
    //     // Make sure neither in or out lists are empty
    //     if self.inputs.len() == 0 {
    //         return Err(Error::BadData("inputs empty".to_string()));
    //     }
    //     if self.outputs.len() == 0 {
    //         return Err(Error::BadData("outputs empty".to_string()));
    //     }

    //     // Each output value, as well as the total, must be in legal money range
    //     let mut total_out = 0;
    //     for tx_out in self.outputs.iter() {
    //         if tx_out.satoshis < 0 {
    //             return Err(Error::BadData("tx_out satoshis negative".to_string()));
    //         }
    //         total_out += tx_out.satoshis;
    //     }
    //     if total_out > MAX_SATOSHIS {
    //         return Err(Error::BadData("Total out exceeds max satoshis".to_string()));
    //     }

    //     // Make sure none of the inputs are coinbase transactions
    //     for tx_in in self.inputs.iter() {
    //         if tx_in.prev_output.hash == COINBASE_OUTPOINT_HASH
    //             && tx_in.prev_output.index == COINBASE_OUTPOINT_INDEX
    //         {
    //             return Err(Error::BadData("Unexpected coinbase".to_string()));
    //         }
    //     }

    //     // Check that lock_time <= INT_MAX because some clients interpret this differently
    //     if self.lock_time > 2_147_483_647 {
    //         return Err(Error::BadData("Lock time too large".to_string()));
    //     }

    //     // Check that all inputs are in the utxo set and are in legal money range
    //     let mut total_in = 0;
    //     for tx_in in self.inputs.iter() {
    //         let utxo = utxos.get(&tx_in.prev_output);
    //         if let Some(tx_out) = utxo {
    //             if tx_out.satoshis < 0 {
    //                 return Err(Error::BadData("tx_out satoshis negative".to_string()));
    //             }
    //             total_in += tx_out.satoshis;
    //         } else {
    //             return Err(Error::BadData("utxo not found".to_string()));
    //         }
    //     }
    //     if total_in > MAX_SATOSHIS {
    //         return Err(Error::BadData("Total in exceeds max satoshis".to_string()));
    //     }

    //     // Check inputs spent > outputs received
    //     if total_in < total_out {
    //         return Err(Error::BadData("Output total exceeds input".to_string()));
    //     }

    //     // Verify each script
    //     let mut sighash_cache = SigHashCache::new();
    //     for input in 0..self.inputs.len() {
    //         let tx_in = &self.inputs[input];
    //         let tx_out = utxos.get(&tx_in.prev_output).unwrap();

    //         let mut script = Script::new();
    //         script.append_slice(&tx_in.unlock_script.0);
    //         script.append(op_codes::OP_CODESEPARATOR);
    //         script.append_slice(&tx_out.lock_script.0);

    //         let mut tx_checker = TransactionChecker {
    //             tx: self,
    //             sig_hash_cache: &mut sighash_cache,
    //             input: input,
    //             satoshis: tx_out.satoshis,
    //             require_sighash_forkid,
    //         };

    //         let is_pregenesis_input = pregenesis_outputs.contains(&tx_in.prev_output);
    //         let flags = if !use_genesis_rules || is_pregenesis_input {
    //             PREGENESIS_RULES
    //         } else {
    //             NO_FLAGS
    //         };

    //         script.eval(&mut tx_checker, flags)?;
    //     }

    //     if use_genesis_rules {
    //         for tx_out in self.outputs.iter() {
    //             if tx_out.lock_script.0.len() == 22
    //                 && tx_out.lock_script.0[0] == OP_HASH160
    //                 && tx_out.lock_script.0[21] == OP_EQUAL
    //             {
    //                 return Err(Error::BadData("P2SH sunsetted".to_string()));
    //             }
    //         }
    //     }

    //     Ok(())
    // }

    /// Returns whether the transaction is the block reward
    pub fn coinbase(&self) -> bool {
        self.inputs.len() == 1
            && self.inputs[0].prev_output.hash == COINBASE_OUTPOINT_HASH
            && self.inputs[0].prev_output.index == COINBASE_OUTPOINT_INDEX
    }
}

impl Serializable<Tx> for Tx {
    fn read(reader: &mut dyn Read) -> Result<Tx> {
        let version = reader.read_i32::<LittleEndian>()?;
        let version = version as u32;
        let n_inputs = var_int::read(reader)?;
        if n_inputs == 0 {
            let segwit_flag = reader.read_u8()?;
            match segwit_flag {
                1 => {
                    let mut inputs = Vec::with_capacity(n_inputs as usize);
                    let n_inputs = var_int::read(reader)?;
                    for _i in 0..n_inputs {
                        inputs.push(TxIn::read(reader)?);
                    }
                    let n_outputs = var_int::read(reader)?;
                    let mut outputs = Vec::with_capacity(n_outputs as usize);
                    for _i in 0..n_outputs {
                        outputs.push(TxOut::read(reader)?);
                    }
                    let mut witnesses = Vec::new();
                    for _i in 0..n_inputs {
                        witnesses.push(TxWitness::read(reader)?);
                    }
                    let lock_time = reader.read_u32::<LittleEndian>()?;
                    println!("These are witnesses {:?}", witnesses);
                    return Ok(Tx {
                        version,
                        inputs,
                        flag: Some(segwit_flag),
                        outputs,
                        lock_time,
                        witnesses: Some(witnesses)
                    });

                }
                _ => {
                    panic!("cant flag this")
                }
            }
        }
        let mut inputs = Vec::with_capacity(n_inputs as usize);
        for _i in 0..n_inputs {
            inputs.push(TxIn::read(reader)?);
        }
        let n_outputs = var_int::read(reader)?;
        let mut outputs = Vec::with_capacity(n_outputs as usize);
        for _i in 0..n_outputs {
            outputs.push(TxOut::read(reader)?);
        }

        let lock_time = reader.read_u32::<LittleEndian>()?;
        Ok(Tx {
            version,
            inputs,
            outputs,
            lock_time,
            witnesses: None,
            flag: None
        })
    }

    fn write(&self, writer: &mut dyn Write) -> io::Result<()> {
        writer.write_u32::<LittleEndian>(self.version)?;

        if let Some(witnesses) = self.witnesses.clone() {
            writer.write_u8(0)?;
            writer.write_u8(1)?;

            var_int::write(self.inputs.len() as u64, writer)?;
            for tx_in in self.inputs.iter() {
                tx_in.write(writer)?;
            }
            var_int::write(self.outputs.len() as u64, writer)?;
            for tx_out in self.outputs.iter() {
                tx_out.write(writer)?;
            }

            for witness in witnesses {
                witness.write(writer).unwrap();
            }

            writer.write_u32::<LittleEndian>(self.lock_time)?;

            return Ok(())
        }

        var_int::write(self.inputs.len() as u64, writer)?;
        for tx_in in self.inputs.iter() {
            tx_in.write(writer)?;
        }
        var_int::write(self.outputs.len() as u64, writer)?;
        for tx_out in self.outputs.iter() {
            tx_out.write(writer)?;
        }
        writer.write_u32::<LittleEndian>(self.lock_time)?;
        Ok(())
    }
}

impl Payload<Tx> for Tx {
    fn size(&self) -> usize {
        let mut size = 8;

        if let Some(witnesses) = self.witnesses.clone() {
            size += 2;

            for witness in witnesses {
                size += witness.size();
            }
        }

        size += var_int::size(self.inputs.len() as u64);
        for tx_in in self.inputs.iter() {
            size += tx_in.size();
        }
        size += var_int::size(self.outputs.len() as u64);
        for tx_out in self.outputs.iter() {
            size += tx_out.size();
        }
        size
    }
}

impl fmt::Debug for Tx {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let inputs_str = format!("[<{} inputs>]", self.inputs.len());
        let outputs_str = format!("[<{} outputs>]", self.outputs.len());

        f.debug_struct("Tx")
            .field("version", &self.version)
            .field(
                "inputs",
                if self.inputs.len() <= 3 {
                    &self.inputs
                } else {
                    &inputs_str
                },
            )
            .field(
                "outputs",
                if self.outputs.len() <= 3 {
                    &self.outputs
                } else {
                    &outputs_str
                },
            )
            .field("lock_time", &self.lock_time)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::OutPoint;
    use crate::util::Hash256;
    use std::io::Cursor;

    // #[test]
    // fn write_read() {
    //     let mut v = Vec::new();
    //     let t = Tx {
    //         version: 1,
    //         inputs: vec![
    //             TxIn {
    //                 prev_output: OutPoint {
    //                     hash: Hash256([9; 32]),
    //                     index: 9,
    //                 },
    //                 unlock_script: Script(vec![1, 3, 5, 7, 9]),
    //                 sequence: 100,
    //             },
    //             TxIn {
    //                 prev_output: OutPoint {
    //                     hash: Hash256([0; 32]),
    //                     index: 8,
    //                 },
    //                 unlock_script: Script(vec![3; 333]),
    //                 sequence: 22,
    //             },
    //         ],
    //         outputs: vec![
    //             TxOut {
    //                 satoshis: 99,
    //                 lock_script: Script(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 100, 99, 98, 97, 96]),
    //             },
    //             TxOut {
    //                 satoshis: 199,
    //                 lock_script: Script(vec![56, 78, 90, 90, 78, 56]),
    //             },
    //         ],
    //         lock_time: 1000,
    //     };
    //     t.write(&mut v).unwrap();
    //     assert!(v.len() == t.size());
    //     assert!(Tx::read(&mut Cursor::new(&v)).unwrap() == t);
    // }

    #[test]
    fn hash() {
        // The coinbase from block 2
        let tx = Tx {
            version: 1,
            flag: None,
            witnesses: None,
            inputs: vec![TxIn {
                prev_output: OutPoint {
                    hash: Hash256([0; 32]),
                    index: 4294967295,
                },
                unlock_script: vec![4, 255, 255, 0, 29, 1, 11],
                sequence: 4294967295,
            }],
            outputs: vec![TxOut {
                satoshis: 5000000000,
                lock_script: vec![
                    65, 4, 114, 17, 168, 36, 245, 91, 80, 82, 40, 228, 195, 213, 25, 76, 31, 207,
                    170, 21, 164, 86, 171, 223, 55, 249, 185, 217, 122, 64, 64, 175, 192, 115, 222,
                    230, 200, 144, 100, 152, 79, 3, 56, 82, 55, 217, 33, 103, 193, 62, 35, 100, 70,
                    180, 23, 171, 121, 160, 252, 174, 65, 42, 227, 49, 107, 119, 172,
                ],
            }],
            lock_time: 0,
        };
        let h = "9b0fc92260312ce44e74ef369f5c66bbb85848f2eddd5a7a1cde251e54ccfdd5";
        assert!(tx.hash() == Hash256::decode(h).unwrap());
        assert!(tx.coinbase());
    }

    // #[test]
    // fn validate() {
    //     let utxo = (
    //         OutPoint {
    //             hash: Hash256([5; 32]),
    //             index: 3,
    //         },
    //         TxOut {
    //             satoshis: 100,
    //             lock_script: Script(vec![]),
    //         },
    //     );
    //     let mut utxos = LinkedHashMap::new();
    //     utxos.insert(utxo.0.clone(), utxo.1.clone());

    //     let tx = Tx {
    //         version: 2,
    //         inputs: vec![TxIn {
    //             prev_output: utxo.0.clone(),
    //             unlock_script: Script(vec![op_codes::OP_1]),
    //             sequence: 0,
    //         }],
    //         outputs: vec![
    //             TxOut {
    //                 satoshis: 10,
    //                 lock_script: Script(vec![]),
    //             },
    //             TxOut {
    //                 satoshis: 20,
    //                 lock_script: Script(vec![]),
    //             },
    //         ],
    //         lock_time: 0,
    //     };
    //     assert!(tx.validate(true, true, &utxos, &HashSet::new()).is_ok());

    //     let mut tx_test = tx.clone();
    //     tx_test.inputs = vec![];
    //     assert!(tx_test
    //         .validate(true, true, &utxos, &HashSet::new())
    //         .is_err());

    //     let mut tx_test = tx.clone();
    //     tx_test.outputs = vec![];
    //     assert!(tx_test
    //         .validate(true, true, &utxos, &HashSet::new())
    //         .is_err());

    //     let mut tx_test = tx.clone();
    //     tx_test.outputs[0].satoshis = -1;
    //     assert!(tx_test
    //         .validate(true, true, &utxos, &HashSet::new())
    //         .is_err());

    //     let mut tx_test = tx.clone();
    //     tx_test.outputs[0].satoshis = 0;
    //     tx_test.outputs[0].satoshis = 0;
    //     assert!(tx_test
    //         .validate(true, true, &utxos, &HashSet::new())
    //         .is_ok());

    //     let mut tx_test = tx.clone();
    //     tx_test.outputs[0].satoshis = MAX_SATOSHIS;
    //     tx_test.outputs[1].satoshis = MAX_SATOSHIS;
    //     assert!(tx_test
    //         .validate(true, true, &utxos, &HashSet::new())
    //         .is_err());

    //     let mut tx_test = tx.clone();
    //     tx_test.outputs[1].satoshis = MAX_SATOSHIS + 1;
    //     assert!(tx_test
    //         .validate(true, true, &utxos, &HashSet::new())
    //         .is_err());

    //     let mut tx_test = tx.clone();
    //     tx_test.inputs[0].prev_output.hash = COINBASE_OUTPOINT_HASH;
    //     tx_test.inputs[0].prev_output.index = COINBASE_OUTPOINT_INDEX;
    //     assert!(tx_test
    //         .validate(true, true, &utxos, &HashSet::new())
    //         .is_err());

    //     let mut tx_test = tx.clone();
    //     tx_test.lock_time = 4294967295;
    //     assert!(tx_test
    //         .validate(true, true, &utxos, &HashSet::new())
    //         .is_err());

    //     let mut tx_test = tx.clone();
    //     tx_test.inputs[0].prev_output.hash = Hash256([8; 32]);
    //     assert!(tx_test
    //         .validate(true, true, &utxos, &HashSet::new())
    //         .is_err());

    //     let mut utxos_clone = utxos.clone();
    //     let prev_output = &tx.inputs[0].prev_output;
    //     utxos_clone.get_mut(prev_output).unwrap().satoshis = -1;
    //     assert!(tx
    //         .validate(true, true, &utxos_clone, &HashSet::new())
    //         .is_err());

    //     let mut utxos_clone = utxos.clone();
    //     let prev_output = &tx.inputs[0].prev_output;
    //     utxos_clone.get_mut(prev_output).unwrap().satoshis = MAX_SATOSHIS + 1;
    //     assert!(tx
    //         .validate(true, true, &utxos_clone, &HashSet::new())
    //         .is_err());

    //     let mut tx_test = tx.clone();
    //     tx_test.outputs[0].satoshis = 100;
    //     assert!(tx_test
    //         .validate(true, true, &utxos, &HashSet::new())
    //         .is_err());

    //     let mut utxos_clone = utxos.clone();
    //     let prev_output = &tx.inputs[0].prev_output;
    //     utxos_clone.get_mut(prev_output).unwrap().lock_script = Script(vec![op_codes::OP_0]);
    //     assert!(tx
    //         .validate(true, true, &utxos_clone, &HashSet::new())
    //         .is_err());

    //     let mut tx_test = tx.clone();
    //     tx_test.outputs[0].lock_script = Script(vec![
    //         OP_HASH160, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, OP_EQUAL,
    //     ]);
    //     assert!(tx_test
    //         .validate(true, false, &utxos, &HashSet::new())
    //         .is_ok());
    //     assert!(tx_test
    //         .validate(true, true, &utxos, &HashSet::new())
    //         .is_err());
    // }
}
