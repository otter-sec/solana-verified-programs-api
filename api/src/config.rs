use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub rpc_url: String,
}
