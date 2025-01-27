//!
//! # BIP158 Compact Block Filters for Light Clients
//!
//! Implements a structure for compact filters on block data, for use in the BIP 157 light client protocol.
//! The filter construction proposed is an alternative to Bloom filters, as used in BIP 37,
//! that minimizes filter size by using Golomb-Rice coding for compression.
//!

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::io::Read;
use std::{cmp, io};
use bitcoin::hashes::siphash24;

use super::{var_int, Hash256};

const P: u8 = 19;
const M: u64 = 784931;

/// A block filter, as described by BIP 158.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockFilter {
    /// Golomb encoded filter
    pub content: Vec<u8>,
}

impl BlockFilter {
    /// Creates a new filter from pre-computed data.
    pub fn new(content: &[u8]) -> BlockFilter { BlockFilter { content: content.to_vec() } }


    /// Returns true if any query matches against this [`BlockFilter`].
    pub fn match_any<I>(&self, block_hash: &Hash256, query: I) -> Result<bool, io::Error>
    where
        I: Iterator,
        I::Item: Borrow<[u8]>,
    {
        let filter_reader = BlockFilterReader::new(block_hash);
        filter_reader.match_any(&mut self.content.as_slice(), query)
    }

    /// Returns true if all queries match against this [`BlockFilter`].
    pub fn match_all<I>(&self, block_hash: &Hash256, query: I) -> Result<bool, io::Error>
    where
        I: Iterator,
        I::Item: Borrow<[u8]>,
    {
        let filter_reader = BlockFilterReader::new(block_hash);
        filter_reader.match_all(&mut self.content.as_slice(), query)
    }
}


/// Reads and interpret a block filter
pub struct BlockFilterReader {
    reader: GcsFilterReader
}

impl BlockFilterReader {
    /// Create a block filter reader

    pub fn new(block_hash: &Hash256) -> BlockFilterReader {
        let block_hash_as_int = block_hash.0;
        let k0 = u64::from_le_bytes(block_hash_as_int[0..8].try_into().expect("8 byte slice"));
        let k1 = u64::from_le_bytes(block_hash_as_int[8..16].try_into().expect("8 byte slice"));
        BlockFilterReader { reader: GcsFilterReader::new(k0, k1,M,P) }
    }

    /// Returns true if any query matches against this [`BlockFilterReader`].
    pub fn match_any<I, R>(&self, reader: &mut R, query: I) -> Result<bool, io::Error>
    where
        I: Iterator,
        I::Item: Borrow<[u8]>,
        R: Read ,
    {
        self.reader.match_any(reader, query)
    }

    /// Returns true if all queries match against this [`BlockFilterReader`].
    pub fn match_all<I, R>(&self, reader: &mut R, query: I) -> Result<bool, io::Error>
    where
        I: Iterator,
        I::Item: Borrow<[u8]>,
        R: Read,
    {
        self.reader.match_all(reader, query)
    }
}


pub struct GcsFilterReader {
    filter: GcsFilter,
    m: u64,
}

impl GcsFilterReader {
    /// Creates a new [`GcsFilterReader`] with specific seed to siphash.
    pub fn new(k0: u64, k1: u64, m: u64, p: u8) -> GcsFilterReader {
        GcsFilterReader { filter: GcsFilter::new(k0, k1, p), m }
    }

    /// Returns true if any query matches against this [`GcsFilterReader`].
    pub fn match_any<I, R>(&self, reader: &mut R, query: I) -> Result<bool, io::Error>
    where
        I: Iterator,
        I::Item: Borrow<[u8]>,
        R: Read ,
    {
        let n_elements = var_int::read(reader).unwrap_or(0);
        // map hashes to [0, n_elements << grp]
        let nm = n_elements * self.m;
        let mut mapped =
            query.map(|e| map_to_range(self.filter.hash(e.borrow()), nm)).collect::<Vec<_>>();
        // sort
        mapped.sort_unstable();
        if mapped.is_empty() {
            return Ok(true);
        }
        if n_elements == 0 {
            return Ok(false);
        }

        // find first match in two sorted arrays in one read pass
        let mut reader = BitStreamReader::new(reader);
        let mut data = self.filter.golomb_rice_decode(&mut reader).unwrap();
        let mut remaining = n_elements - 1;
        for p in mapped {
            loop {
                match data.cmp(&p) {
                    Ordering::Equal => return Ok(true),
                    Ordering::Less =>
                        if remaining > 0 {
                            data += self.filter.golomb_rice_decode(&mut reader).unwrap();
                            remaining -= 1;
                        } else {
                            return Ok(false);
                        },
                    Ordering::Greater => break,
                }
            }
        }
        Ok(false)
    }

    /// Returns true if all queries match against this [`GcsFilterReader`].
    pub fn match_all<I, R>(&self, reader: &mut R, query: I) -> Result<bool, io::Error>
    where
        I: Iterator,
        I::Item: Borrow<[u8]>,
        R: Read,
    {
        let n_elements  = var_int::read(reader).unwrap_or(0);
        // map hashes to [0, n_elements << grp]
        let nm = n_elements * self.m;
        let mut mapped =
            query.map(|e| map_to_range(self.filter.hash(e.borrow()), nm)).collect::<Vec<_>>();
        // sort
        mapped.sort_unstable();
        mapped.dedup();
        if mapped.is_empty() {
            return Ok(true);
        }
        if n_elements == 0 {
            return Ok(false);
        }

        // figure if all mapped are there in one read pass
        let mut reader = BitStreamReader::new(reader);
        let mut data = self.filter.golomb_rice_decode(&mut reader).unwrap();
        let mut remaining = n_elements - 1;
        for p in mapped {
            loop {
                match data.cmp(&p) {
                    Ordering::Equal => break,
                    Ordering::Less =>
                        if remaining > 0 {
                            data += self.filter.golomb_rice_decode(&mut reader).unwrap();
                            remaining -= 1;
                        } else {
                            return Ok(false);
                        },
                    Ordering::Greater => return Ok(false),
                }
            }
        }
        Ok(true)
    }
}

// fast reduction of hash to [0, nm) range
fn map_to_range (hash: u64, nm: u64) -> u64 {
    ((hash as u128 * nm as u128) >> 64) as u64
}


/// Golomb Coded Set Filter
struct GcsFilter {
    k0: u64, // sip hash key
    k1: u64, // sip hash key
    p: u8,
}

impl GcsFilter {
    /// Creates a new [`GcsFilter`].
    fn new(k0: u64, k1: u64, p: u8) -> GcsFilter { GcsFilter { k0, k1, p } }


    /// Golomb-Rice decodes a number from a bit stream (parameter 2^k).
    fn golomb_rice_decode<R>(&self, reader: &mut BitStreamReader<R>) -> Result<u64, io::Error>
    where
        R: Read + ?Sized,
    {
        let mut q = 0u64;
        while reader.read(1).unwrap() == 1 {
            q += 1;
        }
        let r = reader.read(self.p).unwrap();
        Ok((q << self.p) + r)
    }

    /// Hashes an arbitrary slice with siphash using parameters of this filter.
    fn hash(&self, element: &[u8]) -> u64 {
        siphash24::Hash::hash_to_u64_with_keys(self.k0, self.k1, element)
    }
}


/// Bitwise stream reader
/// Bitwise stream reader.
pub struct BitStreamReader<'a, R: ?Sized> {
    buffer: [u8; 1],
    offset: u8,
    reader: &'a mut R,
}

impl<'a, R: Read + ?Sized> BitStreamReader<'a, R> {
    /// Creates a new [`BitStreamReader`] that reads bitwise from a given `reader`.
    pub fn new(reader: &'a mut R) -> BitStreamReader<'a, R> {
        BitStreamReader { buffer: [0u8], reader, offset: 8 }
    }

    /// Reads nbit bits, returning the bits in a `u64` starting with the rightmost bit.
    ///
    /// # Examples
    /// ```
    /// # use bitcoin::bip158::BitStreamReader;
    /// # let data = vec![0xff];
    /// # let mut input = data.as_slice();
    /// let mut reader = BitStreamReader::new(&mut input); // input contains all 1's
    /// let res = reader.read(1).expect("read failed");
    /// assert_eq!(res, 1_u64);
    /// ```
    pub fn read(&mut self, mut nbits: u8) -> Result<u64, io::Error> {
        if nbits > 64 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "can not read more than 64 bits at once",
            ));
        }
        let mut data = 0u64;
        while nbits > 0 {
            if self.offset == 8 {
                self.reader.read_exact(&mut self.buffer).unwrap();
                self.offset = 0;
            }
            let bits = cmp::min(8 - self.offset, nbits);
            data <<= bits;
            data |= ((self.buffer[0] << self.offset) >> (8 - bits)) as u64;
            self.offset += bits;
            nbits -= bits;
        }
        Ok(data)
    }
}



// #[cfg(test)]
// mod test {
//     use blockfilter::test::rustc_serialize::json::Json;
//     use rand;
//     use rand::Rng;
//     use rand::RngCore;
//     use std::fs::File;
//     use std::io::Cursor;
//     use std::io::Read;
//     use std::path::PathBuf;
//     use std::collections::HashMap;
//     use super::*;

//     extern crate rustc_serialize;

//     extern crate hex;

//     fn decode<T: ? Sized>(data: Vec<u8>) -> Result<T, io::Error>
//         where T: Decodable<Cursor<Vec<u8>>> {
//         let mut decoder = Cursor::new(data);
//         Ok(Decodable::consensus_decode(&mut decoder)
//             .map_err(|_| { io::Error::new(io::ErrorKind::InvalidData, "serialization error") })?)
//     }

//     impl UTXOAccessor for HashMap<(Sha256dHash, u32), (Script, u64)> {
//         fn get_utxo(&mut self, txid: &Sha256dHash, ix: u32) -> Result<(Script, u64), io::Error> {
//             if let Some (ux) = self.get(&(*txid, ix)) {
//                 Ok(ux.clone())
//             }
//             else {
//                 println!("missing {}", txid);
//                 Err(io::Error::from(io::ErrorKind::NotFound))
//             }
//         }
//     }

//     #[test]
//     fn test_blockfilters () {
//         let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
//         d.push("tests/blockfilters.json");
//         let mut file = File::open(d).unwrap();
//         let mut data = String::new();
//         file.read_to_string(&mut data).unwrap();

//         let json = Json::from_str(&data).unwrap();
//         let blocks = json[0].as_array().unwrap();
//         let txs = json[1].as_array().unwrap();
//         for t in 1..8 {
//             let mut txmap = HashMap::new();
//             let test_case = blocks [t].as_array().unwrap();
//             let block_hash = Sha256dHash::from_hex(test_case [1].as_string().unwrap()).unwrap();
//             let previous_header_hash = Sha256dHash::from_hex(test_case [3].as_string().unwrap()).unwrap();
//             let header_hash = Sha256dHash::from_hex(test_case[5].as_string().unwrap()).unwrap();
//             let block :Block = decode (hex::decode(test_case[2].as_string().unwrap()).unwrap()).unwrap();
//             assert_eq!(block.bitcoin_hash(), block_hash);

//             for tx in &block.txdata {
//                 for (ix, out) in tx.output.iter().enumerate() {
//                     txmap.insert((tx.txid(), ix as u32), (out.script_pubkey.clone(), out.value));
//                 }
//             }
//             for i in 1 .. 9 {
//                 let line = txs[i].as_array().unwrap();
//                 let tx: bitcoin::blockdata::transaction::Transaction = decode(hex::decode(line[1].as_string().unwrap()).unwrap()).unwrap();
//                 assert_eq!(tx.txid().to_string(), line[0].as_string().unwrap());
//                 for (ix, out) in tx.output.iter().enumerate() {
//                     txmap.insert((tx.txid(), ix as u32), (out.script_pubkey.clone(), out.value));
//                 }
//             }

//             let test_filter = hex::decode(test_case[4].as_string().unwrap()).unwrap();
//             let mut constructed_filter = Cursor::new(Vec::new());
//             {
//                 let mut writer = BlockFilterWriter::new(&mut constructed_filter, &block);
//                 writer.wallet_filter(txmap).unwrap();
//                 writer.finish().unwrap();
//             }

//             let filter = constructed_filter.into_inner();
//             assert_eq!(test_filter, filter);
//             let filter_hash = Sha256dHash::from_data(filter.as_slice());
//             let mut header_data = [0u8; 64];
//             header_data[0..32].copy_from_slice(&filter_hash.as_bytes()[..]);
//             header_data[32..64].copy_from_slice(&previous_header_hash.as_bytes()[..]);
//             let filter_header_hash = Sha256dHash::from_data(&header_data);
//             assert_eq!(filter_header_hash, header_hash);
//         }
//     }

//     #[test]
//     fn test_filter () {
//         let mut bytes = Vec::new();
//         let mut rng = rand::thread_rng();
//         let mut patterns = HashSet::new();
//         for _ in 0..1000 {
//             let mut bytes = [0u8; 8];
//             rng.fill_bytes(&mut bytes);
//             patterns.insert(bytes);
//         }
//         {
//             let mut out = Cursor::new(&mut bytes);
//             let mut writer = GCSFilterWriter::new(&mut out, 0, 0);
//             for p in &patterns {
//                 writer.add_element(p);
//             }
//             writer.finish().unwrap();
//         }
//         {
//             let ref mut reader = GCSFilterReader::new(0, 0).unwrap();
//             let mut it = patterns.iter();
//             for _ in 0..5 {
//                 reader.add_query_pattern(it.next().unwrap());
//             }
//             for _ in 0..100 {
//                 let mut p = it.next().unwrap().to_vec();
//                 p [0] = !p[0];
//                 reader.add_query_pattern(p.as_slice());
//             }
//             let mut input = Cursor::new(&bytes);
//             assert!(reader.match_any(&mut input).unwrap());
//         }
//         {
//             let mut reader = GCSFilterReader::new(0, 0).unwrap();
//             let mut it = patterns.iter();
//             for _ in 0..100 {
//                 let mut p = it.next().unwrap().to_vec();
//                 p [0] = !p[0];
//                 reader.add_query_pattern(p.as_slice());
//             }
//             let mut input = Cursor::new(&bytes);
//             assert!(!reader.match_any(&mut input).unwrap());
//         }
//     }

//     #[test]
//     fn test_bit_stream () {
//         let mut bytes = Vec::new();
//         {
//             let mut out = Cursor::new(&mut bytes);
//             let mut writer = BitStreamWriter::new(&mut out);
//             writer.write(0, 1).unwrap(); // 0
//             writer.write(2, 2).unwrap(); // 10
//             writer.write(6, 3).unwrap(); // 110
//             writer.write(11, 4).unwrap(); // 1011
//             writer.write(1, 5).unwrap(); // 00001
//             writer.write(32, 6).unwrap(); // 100000
//             writer.write(7, 7).unwrap(); // 0000111
//             writer.flush().unwrap();
//         }
//         assert_eq!("01011010110000110000000001110000", format!("{:08b}{:08b}{:08b}{:08b}",bytes[0],bytes[1],bytes[2],bytes[3]));
//         {
//             let mut input = Cursor::new(&mut bytes);
//             let mut reader = BitStreamReader::new(&mut input);
//             assert_eq!(reader.read(1).unwrap(), 0);
//             assert_eq!(reader.read(2).unwrap(), 2);
//             assert_eq!(reader.read(3).unwrap(), 6);
//             assert_eq!(reader.read(4).unwrap(), 11);
//             assert_eq!(reader.read(5).unwrap(), 1);
//             assert_eq!(reader.read(6).unwrap(), 32);
//             assert_eq!(reader.read(7).unwrap(), 7);
//             // 4 bits remained
//             assert!(reader.read(5).is_err());
//         }
//     }
// }