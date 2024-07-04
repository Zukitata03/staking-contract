use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Coin};


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub monthly_reward: Coin,
    pub apr: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Stake { amount: Coin },
    Withdraw {amount: Coin},
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Configure {},
    User {address: String},
}

