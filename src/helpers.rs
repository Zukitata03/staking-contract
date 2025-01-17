use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
     Addr, CosmosMsg, StdResult, WasmMsg, to_json_binary
};

use crate::msg::{ ExecuteMsg};

/// CwTemplateContract is a wrapper around Addr that provides a lot of helpers
/// for working with this. Rename it to your contract name.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakingContract(pub Addr);

impl StakingContract {
    pub fn addr(&self) -> Addr {
        self.0.clone()
    }

    pub fn call<T: Into<ExecuteMsg>>(&self, msg: T) -> StdResult<CosmosMsg> {
        let msg = to_json_binary(&msg.into())?;
        Ok(WasmMsg::Execute {
            contract_addr: self.addr().into(),
            msg,
            funds: vec![],
        }
        .into())
    }

   
}
