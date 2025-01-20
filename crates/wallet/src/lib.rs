#[allow(warnings)]
mod bindings;

use std::{cell::RefCell, str::FromStr};

use bindings::exports::component::wallet::{self, types::{Error, Guest, GuestWatchOnly, BitcoinNetwork as ConfigNetwork, PartialUtxo, WatchOnly}};

use bitcoin::{bip32::Xpub, hashes::Hash, Amount, FeeRate, Network, OutPoint, Txid};
use rand_core::RngCore;
use wasi::random::random::{get_random_u64, get_random_bytes};

mod coin_selection;
mod utils;
mod types;
mod errors;
mod watch_wallet;

struct WasiRandom;

impl RngCore for WasiRandom {
    fn next_u32(&mut self) -> u32 {
        get_random_u64() as u32
    }

    fn next_u64(&mut self) -> u64 {
        get_random_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let source = get_random_bytes(dest.len() as u64);
        dest[..source.len()].copy_from_slice(&source);
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        Ok(self.fill_bytes(dest))
    }
}

struct WatchOnyWallet {
    inner: RefCell<watch_wallet::WatchOnly>,
}

impl Into<Network> for  ConfigNetwork {
    fn into(self) -> Network {
        match self {
            ConfigNetwork::Bitcoin => Network::Bitcoin,
            ConfigNetwork::Testnet => Network::Testnet,
            ConfigNetwork::Testnet4 => Network::Testnet4,
            ConfigNetwork::Signet => Network::Signet,
            ConfigNetwork::Regtest => Network::Regtest,
        }
    }
}

impl From<errors::Error> for  Error {
    fn from(value: errors::Error) -> Self {
        match value {
            errors::Error::CoinSelection(_) => Error::CoinSelection,
            errors::Error::Psbt(_) => Error::Psbt,
            errors::Error::MissingNonWitnessUtxo(_) => Error::MissingNonWitnessUtxo,
            errors::Error::PubKeyError(_) => Error::PubkeyError,
            errors::Error::NoPubKey => Error::NoPubkey,
        }
    }
}

impl Into <wallet::types::PartialUtxo> for types::PartialUtxo {
    fn into(self) -> wallet::types::PartialUtxo {
        return PartialUtxo {
            txid: self.outpoint.txid.as_raw_hash().to_byte_array().to_vec(),
            vout: self.outpoint.vout,
            script: self.script,
            is_spent: self.is_spent,
            amount: self.amount,
        }
    }
}

impl From<wallet::types::PartialUtxo> for types::PartialUtxo {
    fn from(value: wallet::types::PartialUtxo) -> Self {
        let txid = Txid::from_slice(&value.txid).unwrap();
        let outpoint = OutPoint{ txid , vout: value.vout };
        return Self {
            outpoint,
            amount: value.amount,
            is_spent: value.is_spent,
            script: value.script,
        }
    }
} 

impl GuestWatchOnly for WatchOnyWallet {
    fn new(init: wallet::types::Initialization) -> Self {
        match init {
            wallet::types::Initialization::OldState(state) => {
                let wallet =  watch_wallet::WatchOnly::from(state);
                Self{ inner:  RefCell::new(wallet)}
            },
            wallet::types::Initialization::Config(config) => {
                let xpub = Xpub::from_str(&config.xpub).unwrap();
                let wallet =  watch_wallet::WatchOnly::new(xpub, config.network.into());
                Self{ inner:  RefCell::new(wallet)}
            },
        }

    }

    fn new_address(&self) -> Result<String, Error> {
        return self.inner.borrow_mut().get_receive_address().map_err(|err| {
            err.into()
        })
    }

    fn create_transaction(
        &self,
        recipient: Vec<u8>,
        amount: u64,
        fee_rate: u64,
    ) -> Result<Vec<u8>, Error> {
        let fee_rate = FeeRate::from_sat_per_vb(fee_rate).unwrap();
        let amount = Amount::from_sat(amount);
        return self.inner.borrow_mut().create_psbt_tx(recipient, fee_rate, amount, & mut WasiRandom).map_err(|err| err.into())
    }

    
    fn get_utxos(&self) -> Result<Vec<wallet::types::PartialUtxo>, Error> {
        let partial_utxos = self.inner.borrow_mut().get_utxos().map_err(Error::from)?;
        return Ok(partial_utxos.into_iter().map(|utxo| utxo.into()).collect())
    }
    
    fn insert_utxos(&self, utxos: Vec<wallet::types::PartialUtxo>) -> Result<(), Error> {
        let mapped_utxos: Vec<_> = utxos.into_iter().map(From::from).collect();
        return self.inner.borrow_mut().insert_utxos(mapped_utxos).map_err(|err| err.into())
    }
    
    fn get_pubkeys(&self) -> Result<Vec<wallet::types::Pubkey>, Error> {
        return self.inner.borrow_mut().get_pubkeys().map_err(|err| err.into())
    }
    
    fn balance(&self) -> Result<u64, Error> {
        return self.inner.borrow_mut().balance().map(|amount| amount.to_sat()).map_err(|err| err.into())
    }
    
    fn get_receive_address(&self) -> Result<String, Error> {
        return self.inner.borrow_mut().get_receive_address().map_err(|err| err.into())
    }

}


struct Component;

impl Guest for Component {
    
    type WatchOnly = WatchOnyWallet;
}

bindings::export!(Component with_types_in bindings);
