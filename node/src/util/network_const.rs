use bitcoin::{hashes::hash160::Hash, Network};

use super::Hash256;

const REGTEST_MAGIC: [u8; 4] = [0xfa, 0xbf, 0xb5, 0xda];
const TESTNET_MAGIC: [u8; 4] = [0x0b, 0x11, 0x09, 0x07];
const MAINNET_MAGIC: [u8; 4] = [0xf9, 0xbe, 0xb4, 0xd9];
const SIGNET_MAGIC: [u8; 4] = [0x0A, 0x03, 0xcf, 0x40];

pub fn magic_from_network(network: Network) -> [u8; 4] {
    match network {
        Network::Bitcoin => MAINNET_MAGIC,
        Network::Testnet => TESTNET_MAGIC,
        Network::Regtest => REGTEST_MAGIC,
        Network::Signet => SIGNET_MAGIC,
        _ => MAINNET_MAGIC,
    }
}

const GENESIS_BLOCK_HASH_REGTEST: &str = "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206";
const GENESIS_BLOCK_HASH_TESTNET: &str = "000000000933ea01ad0ee984209779baaec3ced90fa3f408719526f8d77f4943";
const GENESIS_BLOCK_HASH_MAINNET: &str = "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f";
const GENESIS_BLOCK_HASH_SIGNET: &str = "00000008819873e925422c1ff0f99f7cc9bbb232af63a077a480a3633bee1ef6";

pub fn genesis_block_hash_from_network(network: Network) -> Hash256 {
    match network {
        Network::Bitcoin => Hash256::decode(GENESIS_BLOCK_HASH_MAINNET).expect("Failed to decode genesis blockhash"),
        Network::Testnet => Hash256::decode(GENESIS_BLOCK_HASH_TESTNET).expect("Failed to decode genesis blockhash"),
        Network::Regtest => Hash256::decode(GENESIS_BLOCK_HASH_REGTEST).expect("Failed to decode genesis blockhash"),
        Network::Signet => Hash256::decode(GENESIS_BLOCK_HASH_SIGNET).expect("Failed to decode genesis blockhash"),
        _ => Hash256::decode(GENESIS_BLOCK_HASH_MAINNET).expect("Failed to decode genesis blockhash"),
    }
}