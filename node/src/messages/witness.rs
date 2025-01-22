use crate::util::{var_int, Result, Serializable};
use byteorder::{ReadBytesExt, WriteBytesExt};
use std::{io};
use std::io::{Read, Write};

use super::Payload;

/// Transaction input
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub struct TxWitness {
    pub witness: Vec<TxWitnessData>
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub struct TxWitnessData {
    pub witness_data: Vec<u8>
}

impl Payload<TxWitnessData> for TxWitnessData {
    fn size(&self) -> usize {
        let mut size = var_int::size(self.witness_data.len() as u64);

        size += self.witness_data.len();

        size

    }
}


impl Serializable<TxWitnessData> for TxWitnessData {
    fn read(reader: &mut dyn Read) -> Result<TxWitnessData> {
        let witness_len = var_int::read(reader)?;
        let mut witness_data = Vec::new();
        for _i in 0..witness_len {
            witness_data.push(reader.read_u8()?);
        };
        Ok(TxWitnessData {
            witness_data
        })
    }

    fn write(&self, writer: &mut dyn Write) -> io::Result<()> {
        var_int::write(self.witness_data.len() as u64, writer).unwrap();
        for byte in self.witness_data.clone().into_iter() {
            writer.write_u8(byte).unwrap();
        };
        Ok(())
    }
}

impl Payload<TxWitness> for TxWitness {
    fn size(&self) -> usize {
        let mut size = var_int::size(self.witness.len() as u64);

        for witnessdata in &self.witness {
            size +=  witnessdata.size();
        }
        size
    }
}


impl Serializable<TxWitness> for TxWitness {
    fn read(reader: &mut dyn Read) -> Result<TxWitness> {
        let witness_len = var_int::read(reader)?;
        let mut witness = Vec::new();
        for _i in 0..witness_len {
            witness.push(TxWitnessData::read(reader)?);
        };
        Ok(TxWitness {
            witness
        })
    }

    fn write(&self, writer: &mut dyn Write) -> io::Result<()> {
        var_int::write(self.witness.len() as u64, writer).unwrap();
        for witness_data in self.witness.iter() {
            witness_data.write(writer).unwrap();
        };
        Ok(())
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::util::Hash256;
//     use std::io::Cursor;

//     #[test]
//     fn write_read() {
//         let mut v = Vec::new();
//         let t = TxIn {
//             prev_output: OutPoint {
//                 hash: Hash256([6; 32]),
//                 index: 8,
//             },
//             unlock_script: Script(vec![255; 254]),
//             sequence: 100,
//         };
//         t.write(&mut v).unwrap();
//         assert!(v.len() == t.size());
//         assert!(TxIn::read(&mut Cursor::new(&v)).unwrap() == t);
//     }
// }
