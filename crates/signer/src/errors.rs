use bitcoin::bip32;

pub enum Error {
   DerivationError(bip32::Error),
   SigningError
    
}
