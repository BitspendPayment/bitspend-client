use std::cell::RefCell;
use std::sync::Arc;
use std::{hash::Hash, iter::zip, vec};

use bitcoin::{
    block, network as bitcoin_network,
};

use crate::bindings::component::kv::types::{Kvstore, Error as StoreError };
use crate::bindings::component::wallet::types::{WatchOnly, Initialization, Config as WalletConfig, BitcoinNetwork as WalletBitcoinNetwork };
use serde::Serialize;
use wasi::sockets::network::Ipv4SocketAddress;

use crate::chain::CompactChain;
use crate::db::{KeyValueDb, CHAIN_STATE_KEY, WALLET_STATE_KEY};
use crate::util::Error;
use crate::util::Hash256;






#[derive(serde::Deserialize, Serialize, Clone)]
pub struct CustomIPV4SocketAddress {
    pub ip: (u8,u8,u8,u8),
    pub port: u16
}

#[derive(Clone)]
pub struct NodeConfig {
    pub socket_address: CustomIPV4SocketAddress,
    pub network: bitcoin_network::Network,
    pub genesis_blockhash: Hash256,
    pub xpub: String,
}



pub struct Node {
    chain: CompactChain,
    wallet: Arc<WatchOnly>,
    node_state: NodeState,
    db: Arc<KeyValueDb>

}

#[derive(serde::Deserialize, Serialize)]
pub struct NodeState {
    socket_address: CustomIPV4SocketAddress,
    network: bitcoin_network::Network
}

impl Into<WalletConfig> for NodeConfig {
    fn into(self) -> WalletConfig {
        let network = match self.network {
            bitcoin::Network::Bitcoin => WalletBitcoinNetwork::Bitcoin,
            bitcoin::Network::Testnet => WalletBitcoinNetwork::Testnet,
            bitcoin::Network::Testnet4 => WalletBitcoinNetwork::Testnet4,
            bitcoin::Network::Signet => WalletBitcoinNetwork::Signet,
            bitcoin::Network::Regtest => WalletBitcoinNetwork::Regtest,
            _ =>  WalletBitcoinNetwork::Bitcoin,
        }; 

         WalletConfig { 
            xpub: self.xpub,
            network
            
         }
    }
}


impl Node {

    pub fn new(node_config: NodeConfig) -> Self {
        let store  = Kvstore::new();
        let db = Arc::new(KeyValueDb::new(store.into()));

        let wallet = Arc::new(WatchOnly::new(&Initialization::Config(node_config.clone().into())));
         
        let chain = CompactChain::new(node_config.socket_address.clone(), node_config.network, wallet.clone());

        Self { chain, wallet, node_state: NodeState{ socket_address: node_config.socket_address, network: node_config.network }, db: db.clone() }

    }

    pub fn restore() -> Self {
        let store  = Kvstore::new();
        let db = Arc::new(KeyValueDb::new(store.into()));

        let serialized_node_state = db.get(WALLET_STATE_KEY.to_string()).expect("cannot retrieve node state");
        let node_state: NodeState = bincode::deserialize(&serialized_node_state).unwrap();

        let wallet_state = db.get(WALLET_STATE_KEY.to_string()).expect("cannot retrieve old wallet state");
        let wallet = Arc::new(WatchOnly::new(&Initialization::OldState(wallet_state)));

        let chain_state = db.get(CHAIN_STATE_KEY.to_string()).expect("cannot retrieve old chain state");
        let chain = CompactChain::restore(node_state.socket_address.clone(), node_state.network, wallet.clone(), chain_state);

        Self {  chain, wallet, node_state, db }

    }

    pub fn balance(&mut self) -> Result<u64, Error> {
        self.chain.sync_state()?;
        
        return self.wallet.balance().map_err(|_| Error::WalletError(3));
    }

    pub fn get_receive_address(&self) -> Result<String, Error> {
        return self.wallet.get_receive_address().map_err(|_| Error::WalletError(4));
    }




 
}
