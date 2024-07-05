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
        eps: calculate_eps(&Config {
            monthly_reward: msg.monthly_reward.clone(),
            total_value_locked: Coin {
                denom: msg.monthly_reward.denom.clone(),
                amount: Uint128::new(1), // Avoid division by zero
            },
            eps: Uint128::zero(),
            last_update_time: env.block.time.seconds(),
            global_exchange_rate: Uint128::new(1_000_000),
        }),
        last_update_time: env.block.time.seconds(),
        global_exchange_rate: Uint128::new(1_000_000),
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
        ExecuteMsg::Claim {  } => try_claim(deps, env, info),
    }
}

fn try_stake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Coin,
) -> Result<Response, ContractError> {
    let mut config = read_config(deps.storage)?;
    let mut user = USERS.may_load(deps.storage, &info.sender)?.unwrap_or_else(|| User {
        staked_amount: Coin {
            denom: amount.denom.clone(),
            amount: Uint128::zero(),
        },
        exchange_rate: config.global_exchange_rate,
        last_staked_time: env.block.time.seconds(),
        rewards: Uint128::zero(),
    });

    update_global_state(&mut config, env.block.time.seconds())?;
    let rewards = calculate_rewards(&config, &user, env.block.time.seconds());
    user.rewards += rewards;

    config.total_value_locked.amount += amount.amount;
    user.staked_amount.amount += amount.amount;

    user.last_staked_time = env.block.time.seconds();
    config.eps = calculate_eps(&config);
    user.exchange_rate = config.global_exchange_rate;

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
    let rewards = calculate_rewards(&config, &user, env.block.time.seconds());
    user.rewards += rewards;

    if user.staked_amount.amount < amount.amount {
        return Err(ContractError::InsufficientStaked {});
    }

    config.total_value_locked.amount -= amount.amount;
    user.staked_amount.amount -= amount.amount;

    user.last_staked_time = env.block.time.seconds();
    config.eps = calculate_eps(&config);
    user.exchange_rate = config.global_exchange_rate;

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

fn try_claim(deps: DepsMut, env:Env, info: MessageInfo) -> Result<Response, ContractError> {
    let mut config = read_config(deps.storage)?;
    let mut user = USERS.load(deps.storage, &info.sender)?;
    update_global_state(&mut config, env.block.time.seconds())?;
    let rewards = calculate_rewards(&config, &user, env.block.time.seconds());
    if rewards == Uint128::zero() {
        return Err(ContractError:: InvalidClaim {  });

    }
    user.rewards = Uint128::zero();
    user.exchange_rate = config.global_exchange_rate;
    user.last_staked_time = env.block.time.seconds();
    USERS.save(deps.storage, &info.sender, &user);
    save_config(deps.storage, &config);
    let reward_coin = Coin {
        denom : config.monthly_reward.denom.clone(),
        amount: rewards,

    };

    Ok(Response::new()
    .add_attribute("action", "claim")
    .add_attribute("amount", reward_coin.amount.to_string()))
}





fn query_config(deps: DepsMut) -> StdResult<ConfigResponse> {
    let config = read_config(deps.storage)?;
    Ok(ConfigResponse {
        monthly_reward: config.monthly_reward,
        total_value_locked: config.total_value_locked,
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
    if time_elapsed > 0 && !config.total_value_locked.amount.is_zero() {
        let rewards = config.eps * Uint128::from(time_elapsed as u128);
        config.global_exchange_rate += rewards.multiply_ratio(Uint128::new(1_000_000), config.total_value_locked.amount);
        println!("Updating Global State - Time Elapsed: {}, Rewards: {}, New Global Exchange Rate: {}", time_elapsed, rewards, config.global_exchange_rate);
    }
    config.last_update_time = current_time;
    Ok(())
}






fn calculate_eps(config: &Config) -> Uint128 {
    if config.total_value_locked.amount.is_zero() {
        Uint128::zero()
    } else {
        // Monthly reward divided by seconds in a month (30 days) and then divided by TVL
        let monthly_seconds = 30 * 24 * 60 * 60; 
        let reward_per_second = config.monthly_reward.amount.u128() as f64 / monthly_seconds as f64;
        let eps = reward_per_second / config.total_value_locked.amount.u128() as f64;
        let eps_uint128 = Uint128::from((eps * 1_000_000_000.0).round() as u128);
        println!(
            "Calculating EPS - Monthly Reward: {}, TVL: {}, EPS: {}",
            config.monthly_reward.amount, config.total_value_locked.amount, eps_uint128
        );
        eps_uint128
    }
}

 

fn calculate_rewards(config: &Config, user: &User, current_time: u64) -> Uint128 {
    if config.total_value_locked.amount.is_zero() {
        return Uint128::zero();
    }
    let exchange_rate_diff = config.global_exchange_rate - user.exchange_rate;
    let rewards = user.staked_amount.amount.multiply_ratio(exchange_rate_diff, Uint128::new(1_000_000));
    println!(
        "Calculating Rewards - Exchange Rate Diff: {}, User Staked Amount: {}, Rewards: {}",
        exchange_rate_diff, user.staked_amount.amount, rewards
    );
    rewards
}



#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

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
            eps: Uint128::new(1),
        };
    
        let res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());
    
        let res = query(deps.as_mut(), env, QueryMsg::Configure {}).unwrap();
        let config: ConfigResponse = from_binary(&res).unwrap();
        assert_eq!(config.monthly_reward.amount, Uint128::new(1000000));
        assert_eq!(config.total_value_locked.amount, Uint128::zero());
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
            eps: Uint128::new(1),
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
        let user: UserResponse = from_binary(&res).unwrap();
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
            eps: Uint128::new(1),
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
        let user: UserResponse = from_binary(&res).unwrap();
        assert_eq!(user.staked_amount.amount, Uint128::new(50));
    }
    

    #[test]
fn two_person_acts() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    let info = mock_info("creator", &[]);
    let msg = InstantiateMsg {
        monthly_reward: Coin {
            denom: "orai".to_string(),
            amount: Uint128::new(1000000),
        },
        eps: Uint128::new(1),
    };
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Alice stakes 100 ORAI
    let info = mock_info("alice", &coins(100, "orai"));
    let msg = ExecuteMsg::Stake {
        amount: Coin {
            denom: "orai".to_string(),
            amount: Uint128::new(100),
        },
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Bob stakes 200 ORAI
    let info = mock_info("bob", &coins(200, "orai"));
    let msg = ExecuteMsg::Stake {
        amount: Coin {
            denom: "orai".to_string(),
            amount: Uint128::new(200),
        },
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Forward time
    env.block.time = env.block.time.plus_seconds(1_296_000);
    update_global_state(&mut read_config(deps.as_mut().storage).unwrap(), env.block.time.seconds()).unwrap();

    // Bob withdraws half (100 ORAI)
    let info = mock_info("bob", &[]);
    let msg = ExecuteMsg::Withdraw {
        amount: Coin {
            denom: "orai".to_string(),
            amount: Uint128::new(100),
        },
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Forward time
    env.block.time = env.block.time.plus_seconds(2_206_000);
    update_global_state(&mut read_config(deps.as_mut().storage).unwrap(), env.block.time.seconds()).unwrap();

    // Alice withdraws all (100 ORAI)
    let info = mock_info("alice", &[]);
    let msg = ExecuteMsg::Withdraw {
        amount: Coin {
            denom: "orai".to_string(),
            amount: Uint128::new(100),
        },
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Bob withdraws the rest (100 ORAI)
    let info = mock_info("bob", &[]);
    let msg = ExecuteMsg::Withdraw {
        amount: Coin {
            denom: "orai".to_string(),
            amount: Uint128::new(100),
        },
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Query Alice
    let res = query(deps.as_mut(), env.clone(), QueryMsg::User { address: "alice".to_string() }).unwrap();
    let user: UserResponse = from_binary(&res).unwrap();
    println!("Alice - Staked amount: {}, Rewards: {}", user.staked_amount.amount, user.rewards);
    assert_eq!(user.staked_amount.amount, Uint128::zero());
    assert!(user.rewards > Uint128::zero());

    // Query Bob
    let res = query(deps.as_mut(), env, QueryMsg::User { address: "bob".to_string() }).unwrap();
    let user: UserResponse = from_binary(&res).unwrap();
    println!("Bob - Staked amount: {}, Rewards: {}", user.staked_amount.amount, user.rewards);
    assert_eq!(user.staked_amount.amount, Uint128::zero());
    assert!(user.rewards > Uint128::zero());
}

#[test]
fn claim_rewards() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();
    let info = mock_info("creator", &[]);
    let msg = InstantiateMsg {
        monthly_reward: Coin {
            denom: "orai".to_string(),
            amount: Uint128::new(1000000),
        },
        eps: Uint128::new(1),
    };
    instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Alice stakes 100 ORAI
    let info = mock_info("alice", &coins(100, "orai"));
    let msg = ExecuteMsg::Stake {
        amount: Coin {
            denom: "orai".to_string(),
            amount: Uint128::new(100),
        },
    };
    execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    // Forward time
    env.block.time = env.block.time.plus_seconds(86400); // 1 day 

    // Alice claims rewards
    let info = mock_info("alice", &[]);
    let res = try_claim(deps.as_mut(), env.clone(), info.clone()).unwrap();
    assert_eq!(res.attributes[0].value, "claim");
    assert!(res.attributes.iter().any(|attr| attr.key == "amount" && attr.value != "0"));

    // Query Alice's state 
    let res = query(deps.as_mut(), env.clone(), QueryMsg::User { address: "alice".to_string() }).unwrap();
    let user: UserResponse = from_binary(&res).unwrap();
    assert_eq!(user.rewards, Uint128::zero());
}
    
}

