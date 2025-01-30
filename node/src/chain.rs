use std::{iter::zip, sync::Arc};
use crate::{bindings::component::wallet::types::{PartialUtxo, WatchOnly}, messages::{block_locator::NO_HASH_STOP, compact_filter::CompactFilter, tx::Tx, tx_out::TxOut, Inv, InvVect}, util::{self, sha256d, Error}};

use bitcoin::network as bitcoin_network;
use serde::Serialize;

use crate::{node::CustomIPV4SocketAddress, p2p::{P2PControl, P2P}, util::Hash256};

pub struct CompactChain {
    p2p: P2P,
    chain_state: ChainState,
    wallet: Arc<WatchOnly>,
}


#[derive(serde::Deserialize, Serialize, Clone)]
pub struct ChainState {
    last_block_hash: Hash256,
    last_block_height: u64,
}

#[derive(serde::Deserialize, Serialize, Clone)]
pub struct Utxo  {
    pub tx_out: TxOut,
    hash: Hash256,
    index: usize,

}


const MAX_HEADER_LEN: usize = 2000;
const FILTER_SIZE: usize = 500;


impl CompactChain {

    pub fn new(socket: CustomIPV4SocketAddress, network: bitcoin_network::Network, genesis_block: Hash256,  wallet: Arc<WatchOnly>  ) -> Self {
        let mut p2p = P2P::new();
        p2p.connect_peer(socket, network).expect("Failed to connect to peer");

        let last_block_hash = genesis_block;
        let last_block_height = 0;
        Self{ p2p, chain_state: ChainState{ last_block_hash, last_block_height }, wallet }

    }

    pub fn restore(socket: CustomIPV4SocketAddress, network: bitcoin_network::Network, wallet: Arc<WatchOnly>, state: Vec<u8> ) -> Self {
        let mut p2p = P2P::new();
        p2p.connect_peer(socket, network).expect("Failed to connect to peer");

        let chain_state: ChainState = bincode::deserialize(&state).unwrap();
        Self{ p2p, chain_state: chain_state, wallet }
    }

    pub fn get_state(& self) -> ChainState {
        return self.chain_state.clone()
    }


    fn get_and_verify_compact_filters(& mut self, start_height: u32, last_block_hash: Hash256) -> Result<Vec<CompactFilter>, Error> {
        let filter_header = self.p2p.get_compact_filter_headers(start_height, last_block_hash).unwrap();
        let filters = self.p2p.get_compact_filters(start_height, last_block_hash).map_err(|err| Error::FetchCompactFilter(err.to_error_code()))?;

        
        for (filter_hash, compact_filter) in zip(filter_header.filter_hashes, filters.clone()) {
            let computed_hash = sha256d(&compact_filter.filter_bytes);
            if computed_hash != filter_hash {
                return Err(Error::FilterMatchEror)
            }
        }
        return Ok(filters);
    }

    fn fetch_and_save_utxos(&mut self, filters: Vec<CompactFilter>) -> Result<(), Error> {
        let pub_keys = &self.wallet.get_pubkeys().map_err(|_| Error::WalletError(1))?;

        let blockhash_present: Vec<_> = filters.into_iter().filter_map(|filter| {
            let filter_algo = util::block_filter::BlockFilter::new(&filter.filter_bytes);
            
            let result = filter_algo.match_any(&filter.block_hash, pub_keys.clone().into_iter()).unwrap();
            match result {
                true => Some(filter.block_hash),
                false => None,
            }
        }).collect();

        if blockhash_present.is_empty() {
            return Ok(());
        }


        let block_inv: Vec<_> = blockhash_present.into_iter().map(|hash| {
            InvVect{ obj_type: 2, hash }
        }).collect();

        let blocks = self.p2p.get_block(Inv{ objects: block_inv}).map_err(|err| Error::FetchBlock(err.to_error_code()))?;

        let utxos: Vec<PartialUtxo> = self.wallet.get_utxos().map_err(|_| Error::WalletError(1))?;
        let mut new_utxos: Vec<PartialUtxo> = vec![];
        for block in blocks {
             for txn in block.txns {
                 for (index, output) in txn.outputs.iter().enumerate() {
                    if pub_keys.contains(&output.lock_script) {
                        new_utxos.push(PartialUtxo { amount: output.satoshis as u64, txid:  txn.hash().0.to_vec(), vout: index as u32, script: output.lock_script.clone(), is_spent: false });
                    }
                 }

                for input in txn.inputs {
                   if input.prev_output.hash == NO_HASH_STOP {
                       continue;
                   } 

                   //TODO: Fix this use get
                   for (index,utxo) in  utxos.clone().iter().enumerate() {
                        if utxo.txid == input.prev_output.hash.0.to_vec() && utxo.vout == input.prev_output.index {
                            let mut utxo = utxos[index].clone();
                            utxo.is_spent = true;
                            new_utxos.push(utxo);
                        }       
                   } 
                }
             }
        }

        self.wallet.insert_utxos(&new_utxos).unwrap();
        Ok(())

    }

    pub fn sync_state(& mut self) -> Result<(),Error> {
        self.p2p.keep_alive().map_err(|_| Error::NetworkError)?;

        let mut is_sync = true;

        println!("syncing");

        while is_sync {

            let fetched_block_headers = self.p2p.fetch_headers(self.chain_state.last_block_hash)
            .map_err(|err| Error::FetchHeader(err.to_error_code()))?;
            if fetched_block_headers.len() == 0 {
                return Ok(());
            }

            let last_block_hash = fetched_block_headers.last()
                .expect("No block headers found")
                .hash();

            // Calculate the range for the for loop
            let start_block = self.chain_state.last_block_height + 1;
            let end_block = start_block + (fetched_block_headers.len() - 1) as u64;

            let mut block_numbers = vec![start_block];
            //ensure that at least one loop is run
            if start_block < end_block {
                block_numbers = (start_block..end_block).step_by(FILTER_SIZE).collect();
            }
            // Generate ranges for block numbers and block headers counter
            let block_headers_counters = (FILTER_SIZE..).step_by(FILTER_SIZE as usize);
           
            // Use zip to iterate over both ranges simultaneously
            for (current_block_num, block_headers_counter) in block_numbers.into_iter().zip(block_headers_counters) {
                // Get the last known block hash
                let last_known_block_hash = fetched_block_headers
                    .get(block_headers_counter as usize - 1)
                    .map(|header| header.hash())
                    .unwrap_or(last_block_hash);
                // Fetch and verify compact filters for the current range
                let block_filters = self.get_and_verify_compact_filters(
                    current_block_num as u32,
                    last_known_block_hash,
                )?;

                if block_filters.is_empty() {
                    continue;
                }
                // Fetch and save UTXOs for the verified block filters
                self.fetch_and_save_utxos(block_filters)?;
            }
    
            if fetched_block_headers.len() < MAX_HEADER_LEN {
                is_sync = false;
            }

            self.chain_state.last_block_height = end_block;
            self.chain_state.last_block_hash = last_block_hash;
        }  
        
        Ok(())
        
    }

    pub fn send_transaction(& mut self, transaction: Tx) -> Result<(),Error> {
        self.p2p.keep_alive().map_err(|_| Error::NetworkError)?;
        self.p2p.send_transaction(transaction)?;
        Ok(())
    }


    
}
