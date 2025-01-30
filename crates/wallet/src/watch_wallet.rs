use std::{collections::BTreeMap, vec};

use bitcoin::{absolute::LockTime, bip32::{ChildNumber, DerivationPath, Fingerprint, Xpub}, consensus::{encode, Encodable}, key::Secp256k1, psbt::{self, Input, PsbtSighashType}, transaction::Version, Address, Amount, CompressedPublicKey, EcdsaSighashType, FeeRate, Network, OutPoint, Psbt, Script, ScriptBuf, Transaction, TxIn, TxOut, Weight, Witness};
use serde::Serialize;

use crate::{coin_selection::{CoinSelectionAlgorithm, DefaultCoinSelectionAlgorithm, Excess}, errors::{self, Error}, types::{self, KeychainKind, PartialUtxo, PubkeyDetails, Utxo, WeightedUtxo}};
use rand_core::RngCore;


#[derive(Copy, Clone, serde::Deserialize, Serialize)]
pub enum WalletType {
    P2WPKH,
}


#[derive(serde::Deserialize, Serialize)]
#[allow(dead_code)]
pub struct  WatchOnly {
    account_xpub: Xpub,
    network: Network,
    pubkey_map: BTreeMap<Vec<u8>, PubkeyDetails>,
    wallet_type: WalletType,
    receive_depth: u32,
    change_depth: u32,
    utxo_map: BTreeMap<OutPoint, WeightedUtxo>,
    account_derivation: DerivationPath,
    master_fingerprint: Fingerprint,

}



#[allow(dead_code)]
impl WatchOnly {

    pub fn new(account_xpub: Xpub, network: Network, account_derivation: DerivationPath, master_fingerprint: Fingerprint) -> Self {
        WatchOnly {
            account_xpub,
            network,
            pubkey_map: BTreeMap::new(),
            utxo_map: BTreeMap::new(),
            wallet_type: WalletType::P2WPKH,
            receive_depth: 0,
            change_depth: 0,
            account_derivation,
            master_fingerprint,
        }
    }

    pub fn from(state: Vec<u8>) -> Self {
        let deserialized_state: WatchOnly = bincode::deserialize(&state).unwrap();
        return deserialized_state
    }

    pub fn get_utxos(& self) -> Result< Vec<PartialUtxo>, errors::Error> {
        let partial_utxos: Vec<PartialUtxo> = self.utxo_map.values().cloned().map(|utxo| utxo.utxo.into()).collect();
        Ok(partial_utxos)
    }

    pub fn get_pubkeys(& self) -> Result< Vec<Vec<u8>>, errors::Error> {
        let pubkeys: Vec<Vec<u8>> = self.pubkey_map.keys().cloned().collect();
        Ok(pubkeys)
    }

    pub fn insert_utxos(&mut self, partial_utxos: Vec<types::PartialUtxo>) -> Result<(), errors::Error> {

        for partial_utxo in partial_utxos {
            match self.utxo_map.get(&partial_utxo.outpoint) {
                Some(utxo) =>  {
                    let mut modified_utxo = utxo.clone();
                    modified_utxo.utxo.is_spent = partial_utxo.is_spent;
                    self.utxo_map.insert(partial_utxo.outpoint, modified_utxo);
                },
                None => {
                    let pubkey_details  = self.pubkey_map.get(&partial_utxo.script).ok_or(errors::Error::NoPubKey)?;
                    let txout = TxOut { value: Amount::from_sat(partial_utxo.amount), script_pubkey: ScriptBuf::from_bytes(partial_utxo.script) };
                    let utxo = Utxo { outpoint: partial_utxo.outpoint, keychain: pubkey_details.key_type, txout , derivation_index: pubkey_details.key_depth, chain_position: None, is_spent: partial_utxo.is_spent};
                    let weighted_utxo = WeightedUtxo { utxo, satisfaction_weight:  Weight::ZERO };
                    self.utxo_map.insert(partial_utxo.outpoint, weighted_utxo);
                },
            }
        }

        Ok(())
    }

    pub fn balance(&mut self) -> Result<Amount, errors::Error> {
        let mut utxos: Vec<_> = self.utxo_map.values().cloned().collect();
        
        utxos.sort_by(|a,b | a.utxo.is_spent.cmp(&b.utxo.is_spent));
        println!("utxo length {}", utxos.len());

        let mut balance = Amount::ZERO;
        for utxo in utxos {
            let inner = utxo.utxo;
            
            if !inner.is_spent {
                balance = balance.checked_add(inner.txout.value).unwrap();
            }
        }
    
        Ok(balance)

    }

    pub fn get_receive_address(& mut self) -> Result<String ,errors::Error>{
        let secp = Secp256k1::new();
        let child_pub = self.account_xpub
            .ckd_pub(&secp, bitcoin::bip32::ChildNumber::Normal { index: 0 })
            .map_err(|err| errors::Error::PubKeyError(err) )?
            .ckd_pub(&secp, bitcoin::bip32::ChildNumber::Normal { index: self.receive_depth })
            .map_err(|err| errors::Error::PubKeyError(err) )?.to_pub();

        let pub_key = Address::p2wpkh(&child_pub, self.network);
        let script_pub =  pub_key.script_pubkey().to_bytes();
        self.pubkey_map.insert(script_pub, PubkeyDetails{ key_type: KeychainKind::External, key_depth: self.receive_depth });
        return  Ok(pub_key.to_string())
        
    }

    fn get_change_script(& mut self) -> Result< Vec<u8> ,errors::Error>{
        let secp = Secp256k1::new();
        let child_pub = self.account_xpub
            .ckd_pub(&secp, bitcoin::bip32::ChildNumber::Normal { index: 1 })
            .map_err(|err| errors::Error::PubKeyError(err) )?
            .ckd_pub(&secp, bitcoin::bip32::ChildNumber::Normal { index: self.change_depth })
            .map_err(|err| errors::Error::PubKeyError(err) )?.to_pub();

        self.change_depth +=1;    
        let script_pub = Address::p2wpkh(&child_pub, self.network)
            .script_pubkey();

        self.pubkey_map.insert(script_pub.to_bytes(), PubkeyDetails{ key_type: KeychainKind::External, key_depth: self.receive_depth });

        return  Ok(script_pub.to_bytes().to_vec())
        
    }

    fn derive_pubkey(&self, utxo: Utxo) -> Result<CompressedPublicKey, errors::Error> {
        let secp = Secp256k1::new();
        let child_pub = self.account_xpub
            .ckd_pub(&secp, bitcoin::bip32::ChildNumber::Normal { index: utxo.keychain.as_u32()})
            .map_err(|err| errors::Error::PubKeyError(err) )?
            .ckd_pub(&secp, bitcoin::bip32::ChildNumber::Normal { index: utxo.derivation_index })
            .map_err(|err| errors::Error::PubKeyError(err) )?.to_pub();

        Ok(child_pub)
    }

    pub fn create_psbt_tx<T: RngCore>(& mut self, recipient: Vec<u8>, fee_rate: FeeRate, amount: Amount, mut rand: T) -> Result<Vec<u8>, errors::Error> {
        let change_script = self.get_change_script()?;
        let utxos: Vec<_> = self.utxo_map.values().cloned().collect();
        let coinselection = DefaultCoinSelectionAlgorithm::default().coin_select(vec![], utxos, fee_rate, amount, Script::from_bytes(&change_script), &mut rand).map_err(|err| errors::Error::CoinSelection(err))?;
        
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

            let partial_derivation_path = DerivationPath::from(vec![ChildNumber::Normal{index: utxo.keychain.as_u32()}, ChildNumber::Normal{index: utxo.derivation_index }]);
            let full_derivation_path = self.account_derivation.extend(partial_derivation_path);

            map.insert(child_pub.0, (self.master_fingerprint, full_derivation_path ));

            let wpkh = child_pub.wpubkey_hash();
            let redeem_script = ScriptBuf::new_p2wpkh(&wpkh);

            let input = Input { witness_utxo: Some(utxo.txout) ,witness_script: Some(redeem_script),bip32_derivation: map, sighash_type: Some(ty),  ..Default::default()};
            inputs.push(input);
            
        };

        psbt.inputs = inputs;

        Ok(psbt.serialize())

    }

    pub fn finalise_psbt_tx(& mut self, mut psbt: Psbt) -> Result<Vec<u8>, errors::Error> {


        for (index, input) in psbt.inputs.clone().into_iter().enumerate() {
            let (pubkey, signature) = input.partial_sigs.first_key_value().unwrap();
            let mut script_witness: Witness = Witness::new();
            script_witness.push(signature.serialize());
            script_witness.push(pubkey.to_bytes());
            psbt.inputs[index].final_script_witness = Some(script_witness);
        }

        // Clear all the data fields as per the spec.
        psbt.inputs[0].partial_sigs = BTreeMap::new();
        psbt.inputs[0].sighash_type = None;
        psbt.inputs[0].redeem_script = None;
        psbt.inputs[0].witness_script = None;
        psbt.inputs[0].bip32_derivation = BTreeMap::new();

        Ok(encode::serialize(&psbt.extract_tx().unwrap()))

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
        let result = wallet.get_receive_address();

        assert!(result.is_ok());
        let address_details = result.unwrap();
        assert!(!address_details.is_empty());
        assert_eq!(address_details, "bc1qcyhpagfzct3dskfefrh7mefrv5hqfy7txzhq24".to_string());
    }

    #[test]
    fn test_derive_p2wpkh_change_script() {
        let mut wallet = WatchOnly::new(get_xpub(), Network::Bitcoin);
        let result = wallet.get_change_script();

        assert!(result.is_ok());
        let script = result.unwrap();
        assert_eq!(script, Vec::from_hex("001478e81513288cb8697189df5aa8561bee7048e192").unwrap());
    }

    #[test]
    fn test_create_psbt_tx() {
        let mut wallet = WatchOnly::new(get_xpub(), Network::Bitcoin);
        wallet.get_receive_address().unwrap();
        let pubkey  = wallet.get_pubkeys().unwrap()[0].clone();
        let utxo = PartialUtxo{ outpoint: OutPoint::from_str("90c6b3b368a8aa8e5ba3b2140d8e178431d3003a9e85f0d303f63b11437451da:0").unwrap(), amount: 100000, is_spent: false,
            script: ScriptBuf::from_bytes(pubkey).into()  };
        let _ = wallet.insert_utxos(vec![utxo]);
        let recipient = Vec::from_hex("0014c12e1ea122c2e2d8593948efede523652e0493cb").unwrap();
        let fee_rate = FeeRate::from_sat_per_vb(3).unwrap();
        let amount = Amount::from_sat(1000);
        let mut rng = StepRng::new(2, 1);

        let result = wallet.create_psbt_tx(recipient, fee_rate, amount, &mut rng);

        assert!(result.is_ok());
        assert!(wallet.pubkey_map.get(&Vec::from_hex("001478e81513288cb8697189df5aa8561bee7048e192").unwrap()).is_some());

    }
}
