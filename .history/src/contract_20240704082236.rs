#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Binary, Coin, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, UserResponse, ConfigResponse};
use crate::state::{Config, User, USERS, read_config, save_config};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
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
        eps: Uint128::zero(),
        last_update_time: env.block.time.seconds(),
        global_exchange_rate: Uint128::new(1_000_000), // 1:1 ratio initially
    };
    save_config(deps.storage, &config)?;
    Ok(Response::new().add_attribute("method", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Stake { amount } => try_stake(deps, env, info, amount),
        ExecuteMsg::Withdraw { amount } => try_withdraw(deps, env, info, amount),
    }
}

fn try_stake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Coin,
) -> Result<Response, ContractError> {
    let mut config = read_config(deps.storage)?;
    let mut user = USERS.may_load(deps.storage, &info.sender)?
        .unwrap_or_else(|| User {
            staked_amount: Coin {
                denom: amount.denom.clone(),
                amount: Uint128::zero(),
            },
            exchange_rate: config.global_exchange_rate,
            last_staked_time: env.block.time.seconds(),
            rewards: Uint128::zero(),
        });

    update_global_state(&mut config, env.block.time.seconds())?;

    // Calculate and update user's rewards before staking
    let rewards = calculate_rewards(&config, &user, env.block.time.seconds());
    user.rewards += rewards;

    // Update TVL and user's staked amount
    config.total_value_locked.amount += amount.amount;
    user.staked_amount.amount += amount.amount;

    // Update user's exchange rate and last staked time
    user.exchange_rate = config.global_exchange_rate;
    user.last_staked_time = env.block.time.seconds();

    save_config(deps.storage, &config)?;
    USERS.save(deps.storage, &info.sender, &user)?;

    Ok(Response::new()
        .add_attribute("action", "stake")
        .add_attribute("amount", amount.to_string()))
}

fn try_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Coin,
) -> Result<Response, ContractError> {
    let mut config = read_config(deps.storage)?;
    let mut user = USERS.load(deps.storage, &info.sender)?;

    update_global_state(&mut config, env.block.time.seconds())?;

    // Calculate and update user's rewards before withdrawing
    let rewards = calculate_rewards(&config, &user, env.block.time.seconds());
    user.rewards += rewards;

    // Check user has enough staked
    if user.staked_amount.amount < amount.amount {
        return Err(ContractError::InsufficientStaked {});
    }

    // Update TVL and user's staked amount
    config.total_value_locked.amount -= amount.amount;
    user.staked_amount.amount -= amount.amount;

    // Update user's exchange rate and last staked time
    user.exchange_rate = config.global_exchange_rate;
    user.last_staked_time = env.block.time.seconds();

    save_config(deps.storage, &config)?;
    USERS.save(deps.storage, &info.sender, &user)?;

    Ok(Response::new()
        .add_attribute("action", "withdraw")
        .add_attribute("amount", amount.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: DepsMut, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Configure {} => to_json_binary(&query_config(deps)?),
        QueryMsg::User { address } => to_json_binary(&query_user(deps, env, address)?),
    }
}

fn query_config(deps: DepsMut) -> StdResult<ConfigResponse> {
    let config = read_config(deps.storage)?;
    Ok(ConfigResponse {
        monthly_reward: config.monthly_reward,
        total_value_locked: config.total_value_locked,
        apr: config.apr,
        eps: config.eps,
        global_exchange_rate: config.global_exchange_rate,
    })
}

fn query_user(deps: DepsMut, env: Env, address: String) -> StdResult<UserResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let user = USERS.load(deps.storage, &addr)?;
    let config = read_config(deps.storage)?;
    
    let current_time = env.block.time.seconds();
    let latest_rewards = calculate_rewards(&config, &user, current_time);
    
    Ok(UserResponse {
        staked_amount: user.staked_amount,
        exchange_rate: user.exchange_rate,
        rewards: user.rewards + latest_rewards,
    })
}

fn update_global_state(config: &mut Config, current_time: u64) -> StdResult<()> {
    let time_elapsed = current_time - config.last_update_time;
    let rewards = config.eps * Uint128::from(time_elapsed);
    
    if !config.total_value_locked.amount.is_zero() {
        config.global_exchange_rate = (config.total_value_locked.amount + rewards)
            .multiply_ratio(Uint128::new(1_000_000), config.total_value_locked.amount);
    }
    
    config.eps = calculate_eps(config);
    config.last_update_time = current_time;
    Ok(())
}

fn calculate_eps(config: &Config) -> Uint128 {
    if config.total_value_locked.amount.is_zero() {
        Uint128::zero()
    } else {
        config.monthly_reward.amount
            .multiply_ratio(Uint128::new(1), Uint128::new(30 * 24 * 60 * 60))
    }
}

fn calculate_rewards(config: &Config, user: &User, current_time: u64) -> Uint128 {
    let time_elapsed = current_time - user.last_staked_time;
    if config.total_value_locked.amount.is_zero() {
        return Uint128::zero();
    }
    user.staked_amount.amount
        .multiply_ratio(config.eps * Uint128::from(time_elapsed), config.total_value_locked.amount)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_json};

    #[test]
    fn initialization() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg {
            monthly_reward: Coin {
                denom: "orai".to_string(),
                amount: Uint128::new(1000000),
            },
            apr: 10,
        };

        let res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        let res = query(deps.as_mut(), env, QueryMsg::Configure {}).unwrap();
        let config: ConfigResponse = from_json(&res).unwrap();
        assert_eq!(config.monthly_reward.amount, Uint128::new(1000000));
        assert_eq!(config.total_value_locked.amount, Uint128::zero());
        assert_eq!(config.apr, 10);
    }

    #[test]
    fn stake_tokens() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg {
            monthly_reward: Coin {
                denom: "orai".to_string(),
                amount: Uint128::new(1000000),
            },
            apr: 10,
        };
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let info = mock_info("alice", &coins(100, "orai"));
        let msg = ExecuteMsg::Stake {
            amount: Coin {
                denom: "orai".to_string(),
                amount: Uint128::new(100),
            },
        };
        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(res.attributes[0].value, "stake");

        let res = query(deps.as_mut(), env.clone(), QueryMsg::User { address: "alice".to_string() }).unwrap();
        let user: UserResponse = from_json(&res).unwrap();
        assert_eq!(user.staked_amount.amount, Uint128::new(100));
    }

    #[test]
    fn withdraw_tokens() {
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg {
            monthly_reward: Coin {
                denom: "orai".to_string(),
                amount: Uint128::new(1000000),
            },
            apr: 10,
        };
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let info = mock_info("alice", &coins(100, "orai"));
        let msg = ExecuteMsg::Stake {
            amount: Coin {
                denom: "orai".to_string(),
                amount: Uint128::new(100),
            },
        };
        execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        env.block.time = env.block.time.plus_seconds(86400); // 1 day later

        let info = mock_info("alice", &[]);
        let msg = ExecuteMsg::Withdraw {
            amount: Coin {
                denom: "orai".to_string(),
                amount: Uint128::new(50),
            },
        };
        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(res.attributes[0].value, "withdraw");

        let res = query(deps.as_mut(), env, QueryMsg::User { address: "alice".to_string() }).unwrap();
        let user: UserResponse = from_json(&res).unwrap();
        assert_eq!(user.staked_amount.amount, Uint128::new(50));
    }

    #[test]
    fn cannot_withdraw_more_than_staked() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg {
            monthly_reward: Coin {
                denom: "orai".to_string(),
                amount: Uint128::new(1000000),
            },
            apr: 10,
        };
        instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let info = mock_info("alice", &coins(100, "orai"));
        let msg = ExecuteMsg::Stake {
            amount: Coin {
                denom: "orai".to_string(),
                amount: Uint128::new(100),
            },
        };
        execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        let info = mock_info("alice", &[]);
        let msg = ExecuteMsg::Withdraw {
            amount: Coin {
                denom: "orai".to_string(),
                amount: Uint128::new(150),
            },
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::InsufficientStaked {});
    }
}
