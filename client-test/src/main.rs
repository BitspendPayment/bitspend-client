use std::env;
use std::path::PathBuf;
use test::test;
use client::{generate_node_regtest_config, BitspendClient};
use wasmtime::component::*;
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{ DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{self, WasiHttpCtx, WasiHttpView};
mod client;
mod test;
mod bitcoind;

fn main() {
   let config = generate_node_regtest_config();
   let mut client = BitspendClient::new(config);
   test(& mut client);

}





