#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Coin, BankMsg, coins, CosmosMsg,  coin, Uint128, SubMsg};

use cosmwasm_std::StakingMsg;
use cosmwasm_std::DistributionMsg;

use sei_cosmwasm::SeiQueryWrapper;

use crate::band::BandProtocol;
// use crate::utils;
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, ExecuteResponse, QueryResponse, InstantiateMsg, QueryMsg, ContractStatus, ResponseStatus, SerializedWithdrawals};
use crate::state::{Config, CONFIG_ITEM, WITHDRAWALS_LIST, self, USER_INFOS, UserWithdrawal};
use crate::utils;
use cosmwasm_std::StdError;


pub const BLOCK_SIZE: usize = 256;
pub const UNBOUND_LATENCY: u64 = 21 * 24 * 60 * 60;
pub const USEI: &str = "usei";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut<SeiQueryWrapper>,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let deposits = msg.deposits.iter().map(|v| v.u128()).collect::<Vec<_>>();

    if deposits.is_empty() {
        return Err(ContractError::Std(StdError::generic_err("Deposits array is empty")));
    }

    let is_sorted = deposits.as_slice().windows(2).all(|v| v[0] > v[1]);
    if !is_sorted {
        return Err(ContractError::Std(StdError::generic_err(
            "Specify deposits in decreasing order",
        )));
    }

    let admin = msg.admin.unwrap_or("".to_string());
    let initial_config: Config = Config {
        status: ContractStatus::Active as u8,
        admin: admin,
        validator: msg.validator,
        usd_deposits: deposits,
    };

    CONFIG_ITEM.save(deps.storage, &initial_config)?;
    // initial_config.save(&deps.storage)?;
    
    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut<SeiQueryWrapper>,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let response = match msg {
        ExecuteMsg::ChangeAdmin { admin, .. } => try_change_admin(deps, env, info, admin),
        ExecuteMsg::ChangeStatus { status, .. } => try_change_status(deps, env, info, status),
        ExecuteMsg::Deposit { .. } => try_deposit(deps, env,  info),
        ExecuteMsg::Withdraw { .. } => try_withdraw(deps, env, info),
        ExecuteMsg::Claim {
            recipient,
            start,
            limit,
            ..
        } => try_claim(deps, env, info, recipient, start, limit),
        ExecuteMsg::WithdrawRewards { recipient, .. } => try_withdraw_rewards(deps, env,info, recipient),
        ExecuteMsg::Redelegate {
            validator_address,
            recipient,
            ..
        } => try_redelegate(deps, env,info, validator_address, recipient),
    };

    return response
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::UserInfo { address } => to_binary(&query_user_info(deps, address)?),
        QueryMsg::Withdrawals {
            address,
            start,
            limit,
        } => to_binary(&query_withdrawals(deps, address, start, limit)?),
    }
}

pub fn try_change_admin(
    deps: DepsMut<SeiQueryWrapper>,
    _env: Env,
    info: MessageInfo,
    new_admin: String,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG_ITEM.load(deps.storage)?;
    if  info.sender.clone() != config.admin {
        return Err(ContractError::Std(StdError::generic_err("Unauthorized")));
    }
    
    CONFIG_ITEM.update(deps.storage, |mut exists| -> StdResult<_> {
        exists.admin = new_admin;
        Ok(exists)
    })?;

    
    Ok(Response::new().add_attribute("action", "changed admin"))

}

pub fn try_change_status(
    deps: DepsMut<SeiQueryWrapper>,
    _env: Env,
    info: MessageInfo,
    status: ContractStatus,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG_ITEM.load(deps.storage)?;
    if  info.sender.clone() != config.admin {
        return Err(ContractError::Std(StdError::generic_err("Unauthorized")));
    }
    
    CONFIG_ITEM.update(deps.storage, |mut exists| -> StdResult<_> {
        exists.status = status as u8;
        Ok(exists)
    })?;
    Ok(Response::new().add_attribute("action", "changed status"))
}

pub fn get_received_funds(_deps: &DepsMut<SeiQueryWrapper>, info: &MessageInfo) -> Result<Coin, ContractError> {
    
    match info.funds.get(0) {
        None => { return Err(ContractError::Std(StdError::generic_err("No Funds"))) }
        Some(received) => {
            /* Amount of tokens received cannot be zero */
            if received.amount.is_zero() {
                return Err(ContractError::Std(StdError::generic_err("Not Allow Zero Amount"))) 
            }

            /* Allow to receive only token denomination defined
            on contract instantiation "config.stable_denom" */
            if received.denom.clone() != "usei" {
                return Err(ContractError::Std(StdError::generic_err("Unsopported token"))) 
            }

            /* Only one token can be received */
            if info.funds.len() > 1 {
                return Err(ContractError::Std(StdError::generic_err("Not Allowed Multiple Funds")));
            }
            Ok(received.clone())
        }
    }
}

pub fn try_deposit(
    deps: DepsMut<SeiQueryWrapper>,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG_ITEM.load(deps.storage)?;
    config.assert_contract_active()?;



    let received_funds = get_received_funds(&deps, &info)?;

    let mut sei_deposit = received_funds.amount.u128();

    let band_protocol = BandProtocol::new(
        &deps,
    )?;

    let usd_deposit = band_protocol.usd_amount(sei_deposit);

    let sender = info.sender.to_string();
    let min_tier = config.min_tier();

    let mut user_info = USER_INFOS
        .may_load(deps.storage, sender)?
        .unwrap_or(state::UserInfo {
            tier: min_tier,
            ..Default::default()
        });
    let current_tier = user_info.tier;
    let old_usd_deposit = user_info.usd_deposit;
    let new_usd_deposit = old_usd_deposit.checked_add(usd_deposit).unwrap();

    let new_tier = config.tier_by_deposit(new_usd_deposit);

    if current_tier == new_tier {
        if current_tier == config.max_tier() {
            return Err(ContractError::Std(StdError::generic_err("Reached max tier"))) 
        }

        let next_tier = current_tier.checked_sub(1).unwrap();
        let next_tier_deposit = config.deposit_by_tier(next_tier);

        let expected_deposit_usd = next_tier_deposit.checked_sub(old_usd_deposit).unwrap();
        let expected_deposit_scrt = band_protocol.usei_amount(expected_deposit_usd);

        let err_msg = format!(
            "You should deposit at least {} USD ({} USEI)",
            expected_deposit_usd, expected_deposit_scrt
        );

        return Err(ContractError::Std(StdError::generic_err(&err_msg))) 
    }

    let mut messages:Vec<SubMsg> = Vec::with_capacity(2);
    let new_tier_deposit = config.deposit_by_tier(new_tier);

    let usd_refund = new_usd_deposit.checked_sub(new_tier_deposit).unwrap();
    let sei_refund = band_protocol.usei_amount(usd_refund);

    if sei_refund != 0 {
        sei_deposit = sei_deposit.checked_sub(sei_refund).unwrap();

        let send_msg = BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(sei_refund, USEI),
        };

        let msg = CosmosMsg::Bank(send_msg);
        messages.push(SubMsg::new(msg));
    }
    let old_sei_deposit = user_info.sei_deposit;
    user_info.tier = new_tier;
    user_info.timestamp = env.block.time.seconds();
    user_info.usd_deposit = new_tier_deposit;
    user_info.sei_deposit = user_info.sei_deposit.checked_add(sei_deposit).unwrap();
    USER_INFOS.save(deps.storage, info.sender.to_string(), &user_info)?;

    let delegate_msg = StakingMsg::Delegate {
        validator: config.validator,
        amount: coin(
            user_info
                .sei_deposit
                .checked_sub(old_sei_deposit)
                .unwrap(),
            USEI,
        ),
    };

    let msg = CosmosMsg::Staking(delegate_msg);
    messages.push(SubMsg::new(msg));

    let answer = to_binary(&ExecuteResponse::Deposit {
        usd_deposit: Uint128::new(user_info.usd_deposit),
        sei_deposit: Uint128::new(user_info.sei_deposit),
        tier: new_tier,
        status: ResponseStatus::Success,
    })?;

    Ok(Response::new().add_submessages(messages).set_data(answer))
}

pub fn try_withdraw(
    deps: DepsMut<SeiQueryWrapper>,
    env: Env,    
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG_ITEM.load(deps.storage)?;
    config.assert_contract_active()?;

    let sender = info.sender.to_string();
    
    let min_tier = config.min_tier();
    let user_info = USER_INFOS
        .may_load(deps.storage, sender)?
        .unwrap_or(state::UserInfo {
            tier: min_tier,
            ..Default::default()
        });

    let amount = user_info.sei_deposit;
    
    USER_INFOS.remove(deps.storage, info.sender.to_string());

    let current_time = env.block.time.seconds();
    let claim_time = current_time.checked_add(UNBOUND_LATENCY).unwrap();
    let withdrawal = UserWithdrawal {
        amount,
        timestamp: current_time,
        claim_time,
    };

    let mut withdrawals = WITHDRAWALS_LIST
        .may_load(deps.storage, info.sender.to_string())?
        .unwrap_or_default();

    withdrawals.push(withdrawal);
    WITHDRAWALS_LIST.save(deps.storage, info.sender.to_string(), &withdrawals)?;
    

    let config = CONFIG_ITEM.load(deps.storage)?;
    let validator = config.validator;
    let amount = coin(amount - 4, USEI);

    let withdraw_msg = StakingMsg::Undelegate { validator, amount };
    let msg = CosmosMsg::Staking(withdraw_msg);

    let answer = to_binary(&ExecuteResponse::Withdraw {
        status: ResponseStatus::Success,
    })?;

    
    Ok(Response::new().add_message(msg).set_data(answer))

}

pub fn try_claim(
    deps: DepsMut<SeiQueryWrapper>,
    env: Env,
    info: MessageInfo,
    recipient: Option<String>,
    start: Option<u32>,
    limit: Option<u32>,
) -> Result<Response, ContractError> {
    let config = CONFIG_ITEM.load(deps.storage)?;
    config.assert_contract_active()?;

    let sender = info.sender.to_string();
    let mut withdrawals = WITHDRAWALS_LIST
        .may_load(deps.storage, sender)?
        .unwrap_or_default();


    let length = withdrawals.len();

    if length == 0 {
        return Err(ContractError::Std(StdError::generic_err("Nothing to claim"))) 
        
    }

    let recipient = recipient.unwrap_or(info.sender.to_string());
    let start: usize = start.unwrap_or(0) as usize;
    let limit = limit.unwrap_or(50) as usize;
    let withdrawals_iter: std::iter::Take<std::iter::Skip<std::slice::Iter<'_, UserWithdrawal>>> = withdrawals.iter().skip(start).take(limit);

    let current_time = env.block.time.seconds();
    let mut remove_indices = Vec::new();
    let mut claim_amount = 0u128;

    for (index, withdrawal) in withdrawals_iter.enumerate() {
        let claim_time = withdrawal.claim_time;

        if current_time >= claim_time {
            remove_indices.push(index);
            claim_amount = claim_amount.checked_add(withdrawal.amount).unwrap();
        }
    }

    if claim_amount == 0 {
        return Err(ContractError::Std(StdError::generic_err("Nothing to claim"))) 
    }

    for (shift, index) in remove_indices.into_iter().enumerate() {
        let position = index.checked_sub(shift).unwrap();
        withdrawals.remove(position);
    }

    let send_msg = BankMsg::Send {
        to_address: recipient,
        amount: coins(claim_amount, USEI),
    };

    let msg = CosmosMsg::Bank(send_msg);
    let answer = to_binary(&ExecuteResponse::Claim {
        amount: claim_amount.into(),
        status: ResponseStatus::Success,
    })?;

    Ok(Response::new().add_message(msg).set_data(answer))
}

pub fn try_withdraw_rewards(
    deps: DepsMut<SeiQueryWrapper>,
    env: Env,
    info: MessageInfo,
    _recipient: Option<String>,
) -> Result<Response, ContractError> {
    
    let config: Config = CONFIG_ITEM.load(deps.storage)?;
    if info.sender.clone() != config.admin {
        return Err(ContractError::Std(StdError::generic_err("Unauthorized")));
    }
    

    let validator = config.validator;
    let delegation = utils::query_delegation(&deps, &env, &validator);

    let can_withdraw = delegation
        .map(|d|  d.unwrap().accumulated_rewards[0].amount.u128())
        .unwrap_or(0);

    if can_withdraw == 0 {
        return Err(ContractError::Std(StdError::generic_err("There is nothing to withdraw")));
    }

    // let admin = config.admin;
    // let recipient = recipient.unwrap_or(admin);
    let withdraw_msg = DistributionMsg::WithdrawDelegatorReward {
        validator,
    };

    let msg = CosmosMsg::Distribution(withdraw_msg);
    let answer = to_binary(&ExecuteResponse::WithdrawRewards {
        amount: Uint128::new(can_withdraw),
        status: ResponseStatus::Success,
    })?;

    Ok(Response::new().add_message(msg).set_data(answer))
}

pub fn try_redelegate(
    deps: DepsMut<SeiQueryWrapper>,
    env: Env,
    info: MessageInfo,
    validator_address: String,
    recipient: Option<String>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG_ITEM.load(deps.storage)?;
    if info.sender.clone() != config.admin {
        return Err(ContractError::Std(StdError::generic_err("Unauthorized")));
    }

    let old_validator = config.validator;
    let delegation = utils::query_delegation(&deps, &env, &old_validator);

    if old_validator == validator_address {
        return Err(ContractError::Std(StdError::generic_err("Redelegation to the same validator")));
    }

    if delegation.is_err() {
        config.validator = validator_address;
        CONFIG_ITEM.save(deps.storage, &config)?;

        let answer = to_binary(&ExecuteResponse::Redelegate {
            amount: Uint128::zero(),
            status: ResponseStatus::Success,
        })?;

        return Ok(Response::new().set_data(answer));
    }

    let delegation = delegation.unwrap().unwrap();
    let can_withdraw = delegation.accumulated_rewards[0].amount.u128();
    let can_redelegate = delegation.can_redelegate.amount.u128();
    let delegated_amount = delegation.amount.amount.u128();

    if can_redelegate != delegated_amount {
        return Err(ContractError::Std(StdError::generic_err("Cannot redelegate full delegation amount")));
    }

    config.validator = validator_address.clone();
    CONFIG_ITEM.save(deps.storage, &config)?;

    let mut messages = Vec::with_capacity(2);
    if can_withdraw != 0 {
        let admin = config.admin;
        let _recipient = recipient.unwrap_or(admin);
        let withdraw_msg = DistributionMsg::WithdrawDelegatorReward {
            validator:old_validator.clone(),
        };
    
        let msg = CosmosMsg::Distribution(withdraw_msg);

        messages.push(msg);
    }

    let coin = coin(can_redelegate, USEI);
    let redelegate_msg = StakingMsg::Redelegate {
        src_validator: old_validator,
        dst_validator: validator_address,
        amount: coin,
    };

    messages.push(CosmosMsg::Staking(redelegate_msg));
    let answer = to_binary(&ExecuteResponse::Redelegate {
        amount: Uint128::new(can_redelegate),
        status: ResponseStatus::Success,
    })?;

    return Ok(Response::new().add_messages(messages).set_data(answer));
}

fn query_config(deps: Deps) -> StdResult<QueryResponse> {
    let config = CONFIG_ITEM.load(deps.storage)?;
    config.to_answer()
}

pub fn query_user_info(deps: Deps, address: String,) -> StdResult<QueryResponse> {
    let config = CONFIG_ITEM.load(deps.storage)?;
    let min_tier = config.min_tier();
    let user_info = USER_INFOS
        .may_load(deps.storage, address)?
        .unwrap_or(state::UserInfo {
            tier: min_tier,
            ..Default::default()
        });

    
    let answer = user_info.to_answer();
    return Ok(answer);
}

pub fn query_withdrawals(
    deps: Deps,
    address: String,
    start: Option<u32>,
    limit: Option<u32>,
) -> StdResult<QueryResponse> {

    let withdrawals = WITHDRAWALS_LIST
        .may_load(deps.storage, address)?
        .unwrap_or_default();
    let amount = withdrawals.len();

    let start = start.unwrap_or(0);
    let limit = limit.unwrap_or(50);

    // let withdrawals = withdrawals.partition_point(pred) .paging(&deps.storage, start, limit)?;
    // let serialized_withdrawals = withdrawals.into_iter().map(|w| w.to_serialized()).collect();

    let mut serialized_withdrawals : Vec<SerializedWithdrawals> = Vec::new();
    for i in start..start+limit {
        let index:usize = i.try_into().unwrap();
        if index < amount {
            serialized_withdrawals.push(withdrawals[index].to_serialized())
        }
    }
    

    let answer = QueryResponse::Withdrawals {
        amount: amount.try_into().unwrap(),
        withdrawals: serialized_withdrawals,
    };

    Ok(answer)
}


#[cfg(test)]
mod tests {
    use std::marker::PhantomData;

    use super::*;
    use cosmwasm_std::testing::{ mock_env, mock_info, MockStorage, MockApi, MockQuerier};
    use cosmwasm_std::{coins,  OwnedDeps};

    #[test]
    fn deposite() {

        let msg = InstantiateMsg {
            validator: "sei183xtf2wmcah9fh5kpdr47wlspv3w47c0lgvwvf".to_string(),
            admin: Some("sei1zwlmtugzr5wk5rxcmrchj2aeu8s8unktlqzmat".to_string()),
            deposits: [
              Uint128::new(300),
              Uint128::new(100),
              Uint128::new(20),
              Uint128::new(10),
              Uint128::new(1)
            ].to_vec()
          };

        let mut mydeps = OwnedDeps {
            storage: MockStorage::default(),
            api: MockApi::default(),
            querier: MockQuerier::default(),
            custom_query_type: PhantomData::<SeiQueryWrapper>,
        };
        
        let info: MessageInfo = mock_info("creator", &coins(2, "usei"));
        let _res = instantiate(mydeps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(200000, "usei"));
        let msg = ExecuteMsg::Deposit { padding: Some("".to_string()) };
        let _res = execute(mydeps.as_mut(), mock_env(), info, msg).unwrap();

    }

    #[test]
    fn change_admin() {

        let msg = InstantiateMsg {
            validator: "sei183xtf2wmcah9fh5kpdr47wlspv3w47c0lgvwvf".to_string(),
            admin: Some("sei1zwlmtugzr5wk5rxcmrchj2aeu8s8unktlqzmat".to_string()),
            deposits: [
              Uint128::new(300),
              Uint128::new(100),
              Uint128::new(20),
              Uint128::new(10),
              Uint128::new(1)
            ].to_vec()
          };

        let mut mydeps = OwnedDeps {
            storage: MockStorage::default(),
            api: MockApi::default(),
            querier: MockQuerier::default(),
            custom_query_type: PhantomData::<SeiQueryWrapper>,
        };
        
        let info: MessageInfo = mock_info("creator", &coins(2, "usei"));
        let _res = instantiate(mydeps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("sei1zwlmtugzr5wk5rxcmrchj2aeu8s8unktlqzmat", &coins(20000, "usei"));
        let msg = ExecuteMsg::ChangeAdmin { admin: "sei1zwlmtugzr5wk5rxcmrchj2aeu8s8unktlqzmat".to_string(),  padding: Some("".to_string()) };
        let _res = execute(mydeps.as_mut(), mock_env(), info, msg).unwrap();

    }


    #[test]
    fn withdraw() {

        let msg = InstantiateMsg {
            validator: "sei183xtf2wmcah9fh5kpdr47wlspv3w47c0lgvwvf".to_string(),
            admin: Some("sei1zwlmtugzr5wk5rxcmrchj2aeu8s8unktlqzmat".to_string()),
            deposits: [
              Uint128::new(300),
              Uint128::new(100),
              Uint128::new(20),
              Uint128::new(10),
              Uint128::new(1)
            ].to_vec()
          };

        let mut mydeps = OwnedDeps {
            storage: MockStorage::default(),
            api: MockApi::default(),
            querier: MockQuerier::default(),
            custom_query_type: PhantomData::<SeiQueryWrapper>,
        };
        
        let info: MessageInfo = mock_info("creator", &coins(2, "usei"));
        let _res = instantiate(mydeps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("sei1zwlmtugzr5wk5rxcmrchj2aeu8s8unktlqzmat", &coins(20000000, "usei"));
        let msg = ExecuteMsg::Deposit { padding: None };
        let _res = execute(mydeps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("sei1zwlmtugzr5wk5rxcmrchj2aeu8s8unktlqzmat", &coins(20000000, "usei"));
        let msg = ExecuteMsg::Withdraw { padding: None } ;
        let _res = execute(mydeps.as_mut(), mock_env(), info, msg).unwrap();

    }

}
