use std::str::FromStr;

use crate::client::BitspendClient;
use bitcoin::{Address, Amount};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use rand::Rng;



pub fn test_new(bitspend_client: & mut BitspendClient) -> u64 {
    let rpc_auth = Auth::UserPass("regtest".into(), "regtest".into());
    let bitcoin_rpc = Client::new("127.0.0.1:18743", rpc_auth).unwrap();

    let address = bitspend_client.get_receive_address();
    let mut rng = rand::thread_rng();
    let mut total_amount = 0;
    let  transfer_amount = 100_000;
    let transformed_address = Address::from_str(&address).unwrap().assume_checked();
    let mine_to_address = Address::from_str("bcrt1qlhwg8036lga3c2t4pmmc6wf49f8t0m5gshjzpj").unwrap().assume_checked();
    for _ in 1..rng.gen_range(50..=100) {

        bitcoin_rpc.send_to_address(&transformed_address, Amount::from_sat(transfer_amount), None, None, Some(false), None, None, None).unwrap();
        total_amount += transfer_amount;
        bitcoin_rpc.generate_to_address(1, &mine_to_address).unwrap();
    }

    for _ in 1..rng.gen_range(50..=100) {
        let address = bitspend_client.get_receive_address();
        let transformed_address = Address::from_str(&address).unwrap().assume_checked();
        bitcoin_rpc.send_to_address(&transformed_address, Amount::from_sat(transfer_amount), None, None, Some(false), None, None, None).unwrap();
        total_amount += transfer_amount;
        bitcoin_rpc.generate_to_address(1, &mine_to_address).unwrap();
    }

    let balance = bitspend_client.balance();
    assert_eq!(balance, total_amount);
    
    let sending = bitspend_client.send_to_address(mine_to_address.script_pubkey().to_bytes(), transfer_amount, 3);
    bitcoin_rpc.generate_to_address(1, &mine_to_address).unwrap();
    let balance = bitspend_client.balance();
    let fee = 341;
    assert_eq!(balance, total_amount - (100_000 + fee));

    return balance;
    
}


pub fn test_restore(bitspend_client: & mut BitspendClient, stored_balance: u64) {
    let rpc_auth = Auth::UserPass("regtest".into(), "regtest".into());
    let bitcoin_rpc = Client::new("127.0.0.1:18743", rpc_auth).unwrap();

    let balance = bitspend_client.balance();
    assert_eq!(balance, stored_balance);

    let address = bitspend_client.get_receive_address();
    let mut rng = rand::thread_rng();
    let mut total_amount = balance;
    let  transfer_amount = 100_000;
    let transformed_address = Address::from_str(&address).unwrap().assume_checked();
    let mine_to_address = Address::from_str("bcrt1qlhwg8036lga3c2t4pmmc6wf49f8t0m5gshjzpj").unwrap().assume_checked();
    for _ in 1..rng.gen_range(50..=100) {

        bitcoin_rpc.send_to_address(&transformed_address, Amount::from_sat(transfer_amount), None, None, Some(false), None, None, None).unwrap();
        total_amount += transfer_amount;
        bitcoin_rpc.generate_to_address(1, &mine_to_address).unwrap();
    }

    for _ in 1..rng.gen_range(50..=100) {
        let address = bitspend_client.get_receive_address();
        let transformed_address = Address::from_str(&address).unwrap().assume_checked();
        bitcoin_rpc.send_to_address(&transformed_address, Amount::from_sat(transfer_amount), None, None, Some(false), None, None, None).unwrap();
        total_amount += transfer_amount;
        bitcoin_rpc.generate_to_address(1, &mine_to_address).unwrap();
    }

    let balance = bitspend_client.balance();
    assert_eq!(balance, total_amount);
    
    let sending = bitspend_client.send_to_address(mine_to_address.script_pubkey().to_bytes(), transfer_amount, 3);
    bitcoin_rpc.generate_to_address(1, &mine_to_address).unwrap();
    let balance = bitspend_client.balance();
    let fee = 341;
    assert_eq!(balance, total_amount - (100_000 + fee));

    
}