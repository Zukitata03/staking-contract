#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{ to_json_binary, Binary, Coin, DepsMut, Env, MessageInfo, Response, StdResult, Storage, Uint128, Addr
};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, save_config, read_config, USERS, User};


const CONTRACT_NAME: &str = "crates.io:staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let config = Config {
        monthly_reward: msg.monthly_reward.clone(),
        total_value_locked: Coin {
            denom: msg.monthly_reward.denom.clone(),
            amount: Uint128::zero(),
        },
        apr: msg.apr,
        eps: 0,
        last_update_time: _env.block.time.seconds(),
    };
    save_config(deps.storage, &config)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Stake {amount} => unimplemented!(),
        ExecuteMsg:: Withdraw {amount} => unimplemented!(),
        ExecuteMsg:: User{address} => unimplemented!(),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: DepsMut, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Configure {} => to_json_binary(&read_config(deps.storage)?),
        QueryMsg::User {address} => to_json_binary(&query_user(&deps, address)?),
    }
}

fn try_stake(
    deps: DepsMut,
    env: Env, 
    info: MessageInfo,
    amount: Coin,
    ) -> StdResult<Response> {
    let mut config = read_config(deps.storage)?;
    let mut user = query_or_create_user(deps.storage, info.sender.as_bytes())?;
    
    //calculate eps
    config.eps = calculate_eps(&config, env.block.time.seconds());

    //update tvl
    config.total_value_locked.amount = config.total_value_locked.amount + amount.amount;
    save_config(deps.storage, &config)?;

    //user state
    user.staked_amount.amount += amount.amount;
    user.exchange_rate = calculate_exchange_rate(config.total_value_locked.amount, user.staked_amount.amount);
    user.last_staked_time = env.block.time.seconds();
    USERS.save(deps.storage, info.sender.as_bytes(), &user)?;

    Ok(Response::new().add_attribute("action", "stake").add_attribute("amount", amount.to_string()))
}

fn try_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Coin,
) -> StdResult<Response> {
    let mut config = read_config(deps.storage)?;
    let mut user = query_user(&deps, info.sender.to_string())?;

    //cal eps
    config.eps = calculate_eps(&config, env.block.time.seconds());

    //update TVL
    if config.total_value_locked.amount < amount.amount {
        return Err(ContractError:: InsufficientFunds {}.into());
    }
    config.total_value_locked.amount = config.total_value_locked.amount - amount.amount;
    save_config(deps.storage, &config)?;

    //update user state
    if user.staked_amount.amount < amount.amount {
        return Err(ContractError:: InsufficientStaked {}.into());
    }

    user.staked_amount.amount = user.staked_amount.amount - amount.amount;
    user.exchange_rate = calculate_exchange_rate(config.total_value_locked.amount, user.staked_amount.amount);
    USERS.save(deps.storage, info.sender.as_bytes(), &user)?;
    Ok(Response::new().add_attribute("action", "withdraw").add_attribute("amount", amount.to_string()))

}

pub fn query_user(deps: &DepsMut, address: String) -> StdResult<User> {
    let validated_address: Addr = deps.api.addr_validate(&address)?;
    USERS.load(deps.storage, validated_address.as_bytes())
}


fn query_or_create_user(storage: &mut dyn Storage, sender: &[u8]) -> StdResult<User> {
    match USERS.may_load(storage, sender)? {
        Some(user) => Ok(user),
        None => {
            let new_user = User {
                staked_amount: Coin {
                    denom: "".to_string(),
                    amount: Uint128::new(0),
                },
                exchange_rate: 0,
                last_staked_time: 0,
            };
            USERS.save(storage, sender, &new_user)?;
            Ok(new_user)
        }
    }
}

fn calculate_exchange_rate(tvl: Uint128, user_stake: Uint128) -> u64 {
    if tvl == Uint128::new(0) {
        0
    } else {
        ((user_stake.u128() * 1_000_000) / tvl.u128()) as u64
    }
}
fn calculate_eps(config: &Config, current_time: u64) -> u64 {
    if config.total_value_locked.amount == Uint128::zero() {
        0
    } else {
        let seconds_in_month = 30 * 24 * 60 * 60;
        (config.monthly_reward.amount.u128() / seconds_in_month) as u64
    }
}




#[cfg(test)]
mod tests {}
