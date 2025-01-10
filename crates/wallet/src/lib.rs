#[allow(warnings)]
mod bindings;

use std::{cell::RefCell, str::FromStr};

use bindings::exports::component::wallet::{self, types::{Error, Guest, Network as ConfigNetwork, GuestWatchOnly, WatchOnly}};

use bitcoin::{amount, bip32::Xpub, Amount, FeeRate, Network};
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

impl GuestWatchOnly for WatchOnyWallet {
    fn new(xpub: String, network: ConfigNetwork) -> Self {
        let xpub = Xpub::from_str(&xpub).unwrap();
        let wallet =  watch_wallet::WatchOnly::new(xpub, network.into());
        Self{ inner:  RefCell::new(wallet)}

    }

    fn new_address(&self) -> Result<String, Error> {
        return self.inner.borrow_mut().derive_p2wpkh_receive_address().map_err(|err| {
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
}


struct Component;

impl Guest for Component {
    
    type WatchOnly = WatchOnyWallet;
}

bindings::export!(Component with_types_in bindings);
