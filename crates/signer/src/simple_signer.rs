use bitcoin::{bip32::{DerivationPath, Fingerprint, IntoDerivationPath, Xpriv, Xpub}, key::Secp256k1, Psbt};
use serde::Serialize;

use crate::errors::Error;


type ExportedData = (Xpub, Fingerprint, DerivationPath);

#[derive(serde::Deserialize, Serialize)]
pub struct SimpleSigner {
    /// The master extended private key.
    master_xpriv: Xpriv,
    /// The master extended public key.
    master_xpub: Xpub,
}

impl SimpleSigner {
    pub fn new(master_xpriv: Xpriv) -> Self {
        let secp = Secp256k1::new();
        let master_xpub = Xpub::from_priv(&secp, &master_xpriv);

       
        Self { master_xpriv, master_xpub }
    }

    pub fn from(state: Vec<u8>) -> Self {
        let deserialized_state: SimpleSigner = bincode::deserialize(&state).unwrap();
        return deserialized_state
    }

    pub fn derive_account(& self) -> Result<ExportedData, Error>  {
        // Only One Account is used for now
        let secp = Secp256k1::new();
        let path = "84h/0h/0h".into_derivation_path().map_err(|err| Error::DerivationError(err) )?;
        let account_0_xpriv = self.master_xpriv.derive_priv(&secp, &path).map_err(|err| Error::DerivationError(err) )?;
        let account_0_xpub = Xpub::from_priv(&secp, &account_0_xpriv);
 
        Ok((account_0_xpub, self.master_xpub.fingerprint(), path))
    } 

    /// Signs `psbt` with this signer.
    pub fn sign_psbt(&self, mut psbt: Psbt) -> Result<Psbt, Error> {
        let secp = Secp256k1::new();
        if let Ok(_) =  psbt.sign(&self.master_xpriv, &secp) {
            Ok(psbt)
        } else {
            Err(Error::SigningError)
        }
    }

    pub fn get_state(& self) -> Vec<u8> {
        return bincode::serialize(self).unwrap();
    }
}