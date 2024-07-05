use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Insufficient staked amount")]
    InsufficientStaked {}, 

    #[error("Insufficient funds to withdraw")]
    InsufficientFunds {},

    #[error("InvalidAmount")]
    InvalidAmount {},

    #[error("Zero Claim")]
    InvalidClaim {},
}

impl From<ContractError> for StdError {
    fn from(err: ContractError) -> Self {
        StdError::generic_err(err.to_string())
    }
}