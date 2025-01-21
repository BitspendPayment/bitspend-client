use reqwest::blocking::Client;
use serde_json::json;
use std::error::Error;

pub struct BitcoinRpc {
    client: Client,
    url: String,
    username: String,
    password: String,
}

impl BitcoinRpc {
    pub fn new(url: &str, username: &str, password: &str) -> Self {
        BitcoinRpc {
            client: Client::new(),
            url: url.to_string(),
            username: username.to_string(),
            password: password.to_string(),
        }
    }

    fn call_method(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, Box<dyn Error>> {
        let payload = json!({
            "jsonrpc": "1.0",
            "id": "curltest",
            "method": method,
            "params": params,
        });

        let response = self
            .client
            .post(&self.url)
            .basic_auth(&self.username, Some(&self.password))
            .json(&payload)
            .send()?;

        let result: serde_json::Value = response.json()?;
        if let Some(error) = result.get("error") {
            if !error.is_null() {
                return Err(format!("RPC Error: {:?}", error).into());
            }
        }

        Ok(result["result"].clone())
    }

    /// Sends Bitcoin to a specific address.
    pub  fn send_to_address(&self, address: &str, amount: f64) -> Result<String, Box<dyn Error>> {
        let params = json!([address, amount]);
        let result = self.call_method("sendtoaddress", params)?;
        Ok(result.as_str().unwrap().to_string())
    }

    /// Generates blocks (mining) to a specified address.
    pub fn generate(&self, nblocks: u32, address: &str) -> Result<(), Box<dyn Error>> {
        let params = json!([nblocks, address]);
        let result = self.call_method("generatetoaddress", params)?;
        Ok(())
    }
}


