use std::env;
use std::path::PathBuf;
use test::{test_new, test_restore };
use client::{generate_node_regtest_config, BitspendClient};
use wasmtime::component::*;
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{ DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{self, WasiHttpCtx, WasiHttpView};
mod client;
mod test;
mod bitcoind;

fn main() {

   // test new config
   let config = generate_node_regtest_config();
   let mut client = BitspendClient::new(config);
   let balance = test_new(& mut client);

   let mut client = BitspendClient::restore();
   test_restore(& mut client, balance);



}





