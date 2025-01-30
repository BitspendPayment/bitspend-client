#[allow(warnings)]
mod bindings;
mod simple_signer;
mod errors;
use std::{cell::RefCell, str::FromStr};

use bindings::exports::component::signer::{self, types::{Error, Guest, GuestSimpleSigner, SimpleSigner}};

use bitcoin::{bip32::Xpriv, psbt, Psbt};

struct SimpleSignerStruct {
    inner: RefCell<simple_signer::SimpleSigner>,
}

impl GuestSimpleSigner for SimpleSignerStruct {
    fn new(init: signer::types::Initialization) -> Self {
        match init {
            signer::types::Initialization::OldState(state) => {
                let signer =  simple_signer::SimpleSigner::from(state);
                Self{ inner:  RefCell::new(signer)}
            },
            signer::types::Initialization::Config(config) => {
                let xpriv = Xpriv::from_str(&config.xpiv).unwrap();
                let signer =  simple_signer::SimpleSigner::new(xpriv);
                Self{ inner:  RefCell::new(signer)}
            },
        }
    }

    fn derive_account(
        &self,
    ) -> Result<(signer::types::AccountXpub, signer::types::MasterFingerprint, signer::types::AccountDerivation), Error> {
        let (xpub, fingerprint, derivation_path) = self.inner.borrow_mut().derive_account().map_err(|_| Error::DerivationError )?;
        let account_xpub = xpub.to_string();
        let master_fingerprint = fingerprint.to_string();
        let account_derivation_path = derivation_path.to_string();

        Ok((account_xpub, master_fingerprint, account_derivation_path))
    }

    fn sign_psbt(&self, psbt: Vec<u8>) -> Result<Vec<u8>, Error> {
        let psbt = Psbt::deserialize(&psbt).unwrap();
        let modified_psbt = self.inner.borrow_mut().sign_psbt(psbt).map_err(|_| Error::SigningError )?;

        Ok(modified_psbt.serialize())
    }
    
    fn get_state(&self) -> Vec<u8> {
      return self.inner.borrow_mut().get_state();
    }
    
}


struct Component;

impl Guest for Component {
    
    
    type SimpleSigner = SimpleSignerStruct;
}


bindings::export!(Component with_types_in bindings);
