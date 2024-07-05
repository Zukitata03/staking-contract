use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Coin, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub monthly_reward: Coin,
    pub eps: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Stake { amount: Coin },
    Withdraw { amount: Coin },
    Claim {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Configure {},
    User { address: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UserResponse {
    pub staked_amount: Coin,
    pub exchange_rate: Uint128,
    pub rewards: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub monthly_reward: Coin,
    pub total_value_locked: Coin,
    pub eps: Uint128,
    pub global_exchange_rate: Uint128,
}
