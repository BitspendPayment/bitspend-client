use std::env;
use std::path::PathBuf;
use bitcoin::bip32::{ExtendedPrivKey, ExtendedPubKey};
use exports::component::node::types::{Initialization, NodeConfig, BitcoinNetwork, Ipv4SocketAdress};
use rand::Rng;
use wasmtime::component::*;
use bitcoin::key::Secp256k1;
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{ DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{self, WasiHttpCtx, WasiHttpView};

include!(concat!(env!("OUT_DIR"), "/node_WIT.rs"));

pub struct BitspendClient {
    store: Store<ClientWasiView>,
    component: ResourceAny,
    world: Nodeworld
}

impl BitspendClient {

    pub fn new(node_config: NodeConfig) -> Self {
        let mut config = Config::default();
        config.wasm_component_model(true);
        config.async_support(false);
        let engine = Engine::new(&config).unwrap();
        let mut linker = Linker::new(&engine);
        let pathtowasm  = PathBuf::from(env::var_os("OUT_DIR").unwrap())
                .join(format!("wasm32-wasi/debug/node-composed.wasm"));
    
        // Add the command world (aka WASI CLI) to the linker
        wasmtime_wasi::add_to_linker_sync(&mut linker).unwrap();
        wasmtime_wasi_http::add_only_http_to_linker_sync(&mut linker).unwrap();
        
        let wasi_view = ClientWasiView::new();
        let mut store = Store::new(&engine, wasi_view);
        
        let component = Component::from_file(&engine, pathtowasm).unwrap();
        let instance =  Nodeworld::instantiate(&mut store, &component, &linker)
            .unwrap();
        let init = Initialization::Config(node_config);
        let resource = instance.component_node_types().client_node().call_constructor(&mut store, &init).unwrap();

        return Self { store, component: resource, world: instance };
    }

    pub fn balance(& mut self) -> u64 {
        let balance = self.world.component_node_types().client_node().call_get_balance(&mut self.store, self.component.clone()).unwrap().unwrap();
        return balance
    }

    pub fn get_receive_address(& mut self) -> String {
        let address = self.world.component_node_types().client_node().call_get_receive_address(&mut self.store, self.component.clone()).unwrap().unwrap();
        return address
    }
}


pub fn generate_node_regtest_config() -> NodeConfig {
    let secp = Secp256k1::new();
    let genesis_blockhash = "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206".to_string();
    let network = BitcoinNetwork::Regtest;
    let socket_address = Ipv4SocketAdress { address: (127,0,0,1) ,  port: 18744 };
    let mut rng = rand::thread_rng();
    let entropy: [u8; 16] = rng.gen();
    let  xpriv = ExtendedPrivKey::new_master(bitcoin::Network::Regtest, &entropy).unwrap();
    let xpub = ExtendedPubKey::from_priv(&secp, &xpriv).to_string();

    return NodeConfig { genesis_blockhash, network, xpub, socket_address}

}

struct ClientWasiView {
    table: ResourceTable,
    ctx: WasiCtx,
    http_ctx: WasiHttpCtx,
}

impl ClientWasiView {
    fn new() -> Self {
        let table = ResourceTable::new();
        let http_ctx = WasiHttpCtx::new();
        let ctx = WasiCtxBuilder::new()
            .inherit_stdio()
            .preopened_dir("/tmp", ".", DirPerms::all(), FilePerms::all()).unwrap()
            .inherit_network()
            .allow_ip_name_lookup(true)
            .allow_tcp(true)
            .build();

        Self { table, ctx, http_ctx }
    }
}

impl WasiView for ClientWasiView {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}

impl WasiHttpView for ClientWasiView {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http_ctx
    }
}