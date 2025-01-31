#[allow(warnings)]
mod bindings;
use std::{cell::RefCell};

use node::{ CustomIPV4SocketAddress, Node, NodeConfig};
use bindings::component::kv::types::{Kvstore};
use bindings::exports::component::node::types::{BitcoinNetwork as WasiBitcoinNetwork, Guest, GuestClientNode, Initialization, NodeConfig as WasiNodeConfig };
use bitcoin::network as bitcoin_network;
use util::Hash256;


mod node;
mod p2p;
mod tcpsocket;
mod util;
mod messages;
mod chain;
mod db;
struct Component;

struct BitcoinNode {
    inner: RefCell<Node>,
}


impl From<WasiBitcoinNetwork> for bitcoin_network::Network {
    fn from(val: WasiBitcoinNetwork) -> Self {
        match val {
            WasiBitcoinNetwork::Bitcoin => bitcoin_network::Network::Bitcoin,
            WasiBitcoinNetwork::Testnet => bitcoin_network::Network::Testnet,
            WasiBitcoinNetwork::Regtest => bitcoin_network::Network::Regtest,
            WasiBitcoinNetwork::Testnet4 => bitcoin_network::Network::Testnet4,
            WasiBitcoinNetwork::Signet => bitcoin_network::Network::Signet,
        }
    }
}




impl From<WasiNodeConfig> for NodeConfig {
    fn from(val: WasiNodeConfig) -> Self {
        let WasiNodeConfig { network, socket_address, xpriv    } = val;

        // Convert the network type
        let network: bitcoin_network::Network = network.into();

        // Construct and return the NodeConfig
        NodeConfig {
            network,
            socket_address: CustomIPV4SocketAddress{ ip: socket_address.address, port: socket_address.port  },
            xpriv
        }
    }
}





impl GuestClientNode for BitcoinNode {
    fn get_balance(&self) -> Result<u64, u32> {
        return  self.inner.borrow_mut().balance().map_err(|err| err.to_error_code());
    }

    fn new(init: Initialization) -> Self {
        match init {
            Initialization::OldState => {
                Self{ inner:  Node::restore().into()}
            },
            Initialization::Config(config) => {
                Self{ inner:  Node::new(config.into()).into()}
            },
        }

    }
    
    fn get_receive_address(&self) -> Result<String, u32> {
        return  self.inner.borrow_mut().get_receive_address().map_err(|err| err.to_error_code());
    }
    
    fn send_to_address(
        &self,
        recepient: Vec<u8>,
        amount: u64,
        fee_rate: u64,
    ) -> Result<(), u32> {
        return self.inner.borrow_mut().send_to_address(&recepient, amount, fee_rate).map_err(|err| err.to_error_code());
    }

    


}

impl Guest for Component {
    
    type ClientNode  = BitcoinNode;
   
}

bindings::export!(Component with_types_in bindings);

