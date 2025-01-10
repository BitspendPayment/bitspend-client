use std::{collections::BTreeMap, vec};

use bitcoin::{absolute::LockTime, bip32::{ChildNumber, DerivationPath, Xpub}, key::Secp256k1, psbt::{self, Input, PsbtSighashType}, transaction::Version, Address, Amount, CompressedPublicKey, EcdsaSighashType, FeeRate, Network, Psbt, Script, ScriptBuf, Transaction, TxIn, TxOut};

use crate::{coin_selection::{CoinSelectionAlgorithm, DefaultCoinSelectionAlgorithm, Excess}, errors::{self, Error}, types::{self, KeychainKind, Utxo}};
use rand_core::RngCore;




#[derive(Copy, Clone)]
pub enum WalletType {
    P2WPKH,
}



#[allow(dead_code)]
pub struct  WatchOnly {
    master_public: Xpub,
    network: Network,
    utxos: Vec<types::WeightedUtxo>,
    wallet_type: WalletType,
    receive_depth: u32,
    change_depth: u32,
    created_script_pubkeys: BTreeMap<Vec<u8>, PubkeyDetails>

}

#[allow(dead_code)]
pub struct PubkeyDetails {
    key_type: KeychainKind,
    key_depth: u32
}


#[allow(dead_code)]
impl WatchOnly {

    pub fn new(master_public: Xpub, network: Network) -> Self {
        WatchOnly {
            master_public,
            network,
            utxos: Vec::new(),
            wallet_type: WalletType::P2WPKH,
            receive_depth: 0,
            change_depth: 0,
            created_script_pubkeys: BTreeMap::new()     
        }
    }

    pub fn add_utxo(&mut self, utxo: types::WeightedUtxo) {
        self.utxos.push(utxo);
    }

    pub fn derive_p2wpkh_receive_address(& mut self) -> Result<String ,errors::Error>{
        let secp = Secp256k1::new();
        let child_pub = self.master_public
            .ckd_pub(&secp, bitcoin::bip32::ChildNumber::Normal { index: 0 })
            .map_err(|err| errors::Error::PubKeyError(err) )?
            .ckd_pub(&secp, bitcoin::bip32::ChildNumber::Normal { index: self.receive_depth })
            .map_err(|err| errors::Error::PubKeyError(err) )?.to_pub();

        let pub_key = Address::p2wpkh(&child_pub, self.network);
        let script_pub =  pub_key.script_pubkey().to_bytes();
        self.created_script_pubkeys.insert(script_pub, PubkeyDetails{ key_type: KeychainKind::External, key_depth: self.receive_depth });
        return  Ok(pub_key.to_string())
        
    }

    fn derive_p2wpkh_change_script(& mut self) -> Result< Vec<u8> ,errors::Error>{
        let secp = Secp256k1::new();
        let child_pub = self.master_public
            .ckd_pub(&secp, bitcoin::bip32::ChildNumber::Normal { index: 1 })
            .map_err(|err| errors::Error::PubKeyError(err) )?
            .ckd_pub(&secp, bitcoin::bip32::ChildNumber::Normal { index: self.change_depth })
            .map_err(|err| errors::Error::PubKeyError(err) )?.to_pub();

        self.change_depth +=1;    
        let script_pub = Address::p2wpkh(&child_pub, self.network)
            .script_pubkey();

        self.created_script_pubkeys.insert(script_pub.to_bytes(), PubkeyDetails{ key_type: KeychainKind::External, key_depth: self.receive_depth });

        return  Ok(script_pub.to_bytes().to_vec())
        
    }

    fn derive_pubkey(&self, utxo: Utxo) -> Result<CompressedPublicKey, errors::Error> {
        let secp = Secp256k1::new();
        let child_pub = self.master_public
            .ckd_pub(&secp, bitcoin::bip32::ChildNumber::Normal { index: utxo.keychain.as_u32()})
            .map_err(|err| errors::Error::PubKeyError(err) )?
            .ckd_pub(&secp, bitcoin::bip32::ChildNumber::Normal { index: utxo.derivation_index })
            .map_err(|err| errors::Error::PubKeyError(err) )?.to_pub();

        Ok(child_pub)
    }

    pub fn create_psbt_tx<T: RngCore>(& mut self, recipient: Vec<u8>, fee_rate: FeeRate, amount: Amount, mut rand: T) -> Result<Vec<u8>, errors::Error> {
        let change_script = self.derive_p2wpkh_change_script()?;
        let coinselection = DefaultCoinSelectionAlgorithm::default().coin_select(vec![], self.utxos.clone(), fee_rate, amount, Script::from_bytes(&change_script), &mut rand).map_err(|err| errors::Error::CoinSelection(err))?;
        
        let inputs = coinselection.selected.clone().iter().map(|utxo| TxIn {
            previous_output: utxo.outpoint,
            script_sig: Default::default(),
            sequence: Default::default(),
            witness: Default::default(),
        }).collect();

        let mut recipients = vec![TxOut {
            script_pubkey: ScriptBuf::from(recipient),
            value: amount,
        }];

        if let Excess::Change { amount, .. } = coinselection.excess {
            recipients.push(TxOut {
                script_pubkey: ScriptBuf::from(change_script),
                value: amount,
            });
        }

        let transaction = Transaction {
            version:  Version::TWO,
            lock_time: LockTime::ZERO,
            input: inputs,
            output: recipients,
        };

        let  mut psbt = Psbt::from_unsigned_tx(transaction).map_err(errors::Error::Psbt)?;

        let mut inputs=  Vec::new();
        let ty = PsbtSighashType::from(EcdsaSighashType::All);
        
        
        for utxo in coinselection.selected {
            let child_pub = self.derive_pubkey(utxo.clone())?;
            let mut map = BTreeMap::new();

            let derivation_path = DerivationPath::from(vec![ChildNumber::Normal{index: utxo.keychain.as_u32()}, ChildNumber::Normal{index: utxo.derivation_index }]);
            map.insert(child_pub.0, (self.master_public.parent_fingerprint, derivation_path ));

            let wpkh = child_pub.wpubkey_hash();
            let redeem_script = ScriptBuf::new_p2wpkh(&wpkh);

            let input = Input { witness_utxo: Some(utxo.txout) ,witness_script: Some(redeem_script),bip32_derivation: map, sighash_type: Some(ty),  ..Default::default()};
            inputs.push(input);
            
        };

        psbt.inputs = inputs;

        Ok(psbt.serialize())

    }
}


#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bitcoin::{hex::FromHex, OutPoint, Weight};
    use rand::rngs::mock::StepRng;
    use types::WeightedUtxo;

    use super::*;

    fn get_xpub() -> Xpub {
        let xpub = Xpub::from_str("xpub6BgqrNmJjjiaQASxUaBH9xLTBtnVpSoTzSBiGt12K572ofLub5U2rvZok5MJ5qFnBqPVi2HmnMhzQsAuZ1jG7ppoizmEzbuuCTtj9rm9Cpp");
        return xpub.unwrap();
    }

    #[test]
    fn test_derive_p2wpkh_receive_address() {
        
        let mut wallet = WatchOnly::new(get_xpub(), Network::Bitcoin);
        let result = wallet.derive_p2wpkh_receive_address();

        assert!(result.is_ok());
        let address_details = result.unwrap();
        assert!(!address_details.is_empty());
        assert_eq!(address_details, "bc1qcyhpagfzct3dskfefrh7mefrv5hqfy7txzhq24".to_string());
    }

    #[test]
    fn test_derive_p2wpkh_change_script() {
        let mut wallet = WatchOnly::new(get_xpub(), Network::Bitcoin);
        let result = wallet.derive_p2wpkh_change_script();

        assert!(result.is_ok());
        let script = result.unwrap();
        assert_eq!(script, Vec::from_hex("001478e81513288cb8697189df5aa8561bee7048e192").unwrap());
    }

    #[test]
    fn test_create_psbt_tx() {
        let mut wallet = WatchOnly::new(get_xpub(), Network::Bitcoin);
        let utxo = Utxo{ outpoint: OutPoint::from_str("90c6b3b368a8aa8e5ba3b2140d8e178431d3003a9e85f0d303f63b11437451da:0").unwrap(), keychain: types::KeychainKind::External, 
        txout: TxOut { value: Amount::from_sat(2000), script_pubkey: ScriptBuf::from_hex("0014c12e1ea122c2e2d8593948efede523652e0493cb").unwrap() }, is_spent: false, derivation_index: 0, chain_position: None };


        wallet.add_utxo(WeightedUtxo { satisfaction_weight: Weight::ZERO, utxo });
        let recipient = Vec::from_hex("0014c12e1ea122c2e2d8593948efede523652e0493cb").unwrap();
        let fee_rate = FeeRate::from_sat_per_vb(3).unwrap();
        let amount = Amount::from_sat(1000);
        let mut rng = StepRng::new(2, 1);

        let result = wallet.create_psbt_tx(recipient, fee_rate, amount, &mut rng);

        assert!(result.is_ok());
        assert!(wallet.created_script_pubkeys.get(&Vec::from_hex("001478e81513288cb8697189df5aa8561bee7048e192").unwrap()).is_some());
    }
}
