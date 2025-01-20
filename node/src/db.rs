use std::{cell::RefCell, sync::Arc};
use crate::{bindings, util::Error};

use bindings::component::kv::types::Kvstore ;


pub const CHAIN_STATE_KEY: &str = "chain_state";
pub const WALLET_STATE_KEY: &str = "wallet_state";
pub const NODE_STATE_KEY: &str = "node_state";

pub struct KeyValueDb {
    conn: Arc<Kvstore>
}

impl KeyValueDb {

    pub fn new(store: RefCell<Kvstore>) -> Self {
        Self{ conn: Arc::new(store.into_inner()) }
    }
    pub fn insert(&self, key: String, value: Vec<u8>) -> Result<(), Error> {
        self.conn.insert(&key, &value).map_err(|err| Error::DBError(err))        
    }
     /// Retrieve a value by its key.
    pub fn get(&self, key: String) -> Result<Vec<u8>, Error> { 
            self.conn.get(&key).map_err(|err| Error::DBError(err))
      }

     /// Delete a key-value pair by its key.
     fn delete(&self, key: String) -> Result<(), Error> {
        self.conn.delete(&key).map_err(|err| Error::DBError(err))
     }
}