use std::io::Cursor;
use std::sync::Arc;
use bincode::de;
use bitcoin::bip32::Xpriv;
use bitcoin::{network as bitcoin_network, Address, Amount, FeeRate};

use crate::bindings::component::kv::types::Kvstore ;
use crate::bindings::component::wallet::types::{WatchOnly, Initialization, Config as WalletConfig, BitcoinNetwork as WalletBitcoinNetwork };
use crate::bindings::component::signer::types::{SimpleSigner, Initialization as SignerInitialization, Config as SignerConfig };
use crate::bindings::exports::component::node::types::BitcoinNetwork;
use crate::messages::tx::Tx;

use serde::Serialize;

use crate::chain::CompactChain;
use crate::db::{KeyValueDb, CHAIN_STATE_KEY, SIGNER_STATE_KEY, WALLET_STATE_KEY};
use crate::util::{Error, Serializable};
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
    pub xpriv: String,
}



pub struct Node {
    chain: CompactChain,
    wallet: Arc<WatchOnly>,
    signer: Arc<SimpleSigner>,
    node_state: NodeState,
    db: Arc<KeyValueDb>

}

#[derive(serde::Deserialize, Serialize)]
pub struct NodeState {
    socket_address: CustomIPV4SocketAddress,
    network: bitcoin_network::Network
}

impl Into<WalletBitcoinNetwork> for bitcoin_network::Network {
    fn into(self) -> WalletBitcoinNetwork {
        let network = match self {
            bitcoin::Network::Bitcoin => WalletBitcoinNetwork::Bitcoin,
            bitcoin::Network::Testnet => WalletBitcoinNetwork::Testnet,
            bitcoin::Network::Testnet4 => WalletBitcoinNetwork::Testnet4,
            bitcoin::Network::Signet => WalletBitcoinNetwork::Signet,
            bitcoin::Network::Regtest => WalletBitcoinNetwork::Regtest,
            _ =>  WalletBitcoinNetwork::Bitcoin,
        }; 

        return network;
    }
}


impl Node {

    pub fn new(node_config: NodeConfig) -> Self {
        let store  = Kvstore::new();
        let db = Arc::new(KeyValueDb::new(store.into()));

        // Initialize P2WPKH Signer and Watch Only Wallet
        let signer = Arc::new(SimpleSigner::new(&SignerInitialization::Config(SignerConfig { xpiv: node_config.xpriv})));
        let  ( xpub, master_fingerprint, account_derivation )= signer.derive_account().unwrap();
        let wallet_config = WalletConfig {
            xpub,
            account_derivation,
            master_fingerprint,
            network: node_config.network.into(), 
        };
        let wallet = Arc::new(WatchOnly::new(&Initialization::Config(wallet_config)));
         
        let chain = CompactChain::new(node_config.socket_address.clone(), node_config.network, node_config.genesis_blockhash, wallet.clone());

        Self { chain, wallet, node_state: NodeState{ socket_address: node_config.socket_address, network: node_config.network }, db: db.clone(), signer }

    }

    pub fn restore() -> Self {
        let store  = Kvstore::new();
        let db = Arc::new(KeyValueDb::new(store.into()));

        let serialized_node_state = db.get(WALLET_STATE_KEY.to_string()).expect("cannot retrieve node state");
        let node_state: NodeState = bincode::deserialize(&serialized_node_state).unwrap();

        let wallet_state = db.get(WALLET_STATE_KEY.to_string()).expect("cannot retrieve old wallet state");
        let wallet = Arc::new(WatchOnly::new(&Initialization::OldState(wallet_state)));

        let signer_state = db.get(SIGNER_STATE_KEY.to_string()).expect("cannot retrieve old signer state");
        let signer = Arc::new(SimpleSigner::new(&SignerInitialization::OldState(signer_state)));

        let chain_state = db.get(CHAIN_STATE_KEY.to_string()).expect("cannot retrieve old chain state");
        let chain = CompactChain::restore(node_state.socket_address.clone(), node_state.network, wallet.clone(), chain_state);

        Self {  chain, wallet, node_state, db, signer }

    }

    pub fn balance(&mut self) -> Result<u64, Error> {
        self.chain.sync_state()?;
        
        return self.wallet.balance().map_err(|_| Error::WalletError(3));
    }

    pub fn get_receive_address(&self) -> Result<String, Error> {
        return self.wallet.get_receive_address().map_err(|_| Error::WalletError(4));
    }

    pub fn send_to_address(& mut self, recepient: &[u8], amount: u64, fee_rate: u64) -> Result<(), Error> {
        self.chain.sync_state()?;
    
        let transaction = self.wallet.create_transaction(recepient, amount, fee_rate).unwrap();
        let signed_transaction = self.signer.sign_psbt(&transaction).unwrap();
        let finalised_transaction = self.wallet.finalise_transaction(&signed_transaction).unwrap();
        let mut cursor_transaction = Cursor::new(finalised_transaction);
        let deserialised_transaction = Tx::read(&mut cursor_transaction).unwrap();

        self.chain.send_transaction(deserialised_transaction).unwrap();

        return Ok(());

    }




 
}
