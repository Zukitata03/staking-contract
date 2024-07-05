use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Coin, StdResult, Storage, Uint128, Addr};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub monthly_reward: Coin,
    pub total_value_locked: Coin,
    pub eps: Uint128,
    pub last_update_time: u64,
    pub global_exchange_rate: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct User {
    pub staked_amount: Coin,
    pub exchange_rate: Uint128,
    pub last_staked_time: u64,
    pub rewards: Uint128,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const USERS: Map<&Addr, User> = Map::new("users");

pub fn save_config(storage: &mut dyn Storage, config: &Config) -> StdResult<()> {
    CONFIG.save(storage, config)
}

pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
    CONFIG.load(storage)
}
