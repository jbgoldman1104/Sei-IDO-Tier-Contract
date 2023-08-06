#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, DepsMut, Env, MessageInfo, Response, BankMsg, coins, CosmosMsg, Uint128, SubMsg, WasmMsg, StdResult, Binary, Deps};


use cw20::Cw20ExecuteMsg;
use sei_cosmwasm::SeiQueryWrapper;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, ExecuteResponse,  InitMsg,  ContractStatus, ResponseStatus, PaymentMethod, Whitelist, QueryMsg, QueryResponse};
use crate::state::{Config, Ido,  WHITELIST, OWNER_TO_IDOS, Purchase, PURCHASES, IDO_TO_INFO, USERINFO, ACTIVE_IDOS, ARCHIVED_PURCHASES, CONFIG_KEY};
use crate::tier::{get_min_tier, get_tier};
use crate::utils::{self, assert_admin, assert_contract_active, assert_ido_admin};
use cosmwasm_std::StdError;


pub const BLOCK_SIZE: usize = 256;
pub const USEI: &str = "usei";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut<SeiQueryWrapper>,
    _env: Env,
    _info: MessageInfo,
    msg: InitMsg,
) -> Result<Response, ContractError> {
    let admin = msg.admin.unwrap_or(_info.sender.to_string());
    let canonical_admin = admin.to_string();
    let tier_contract = msg.tier_contract.to_string();
    let nft_contract = msg.nft_contract.to_string();
    let lock_periods_len = msg.lock_periods.len();

    let mut config = Config {
        admin: canonical_admin,
        status: ContractStatus::Active as u8,
        tier_contract,
        nft_contract,
        lock_periods: msg.lock_periods,
        min_tier: 0,
    };

    let min_tier = get_min_tier(&deps, &config)?;
    config.min_tier = min_tier;

    if lock_periods_len != min_tier as usize {
        return Err(ContractError::Std(StdError::generic_err(&format!(
            "Lock periods array must have {} items",
            min_tier
        ))));
    }

    CONFIG_KEY.save(deps.storage, &config)?;

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
        ExecuteMsg::ChangeAdmin { admin, .. } => change_admin(deps, env, info, admin),
        ExecuteMsg::ChangeStatus { status, .. } => change_status(deps, env, info, status),
        ExecuteMsg::StartIdo {
            start_time,
            end_time,
            token_contract: token_contract_addr,
            price,
            total_amount,
            soft_cap,
            tokens_per_tier,
            whitelist,
            payment,
            ..
        } => {
            let mut ido = Ido::default();
            assert_admin(&deps, &info.sender.to_string())?;
            let admin = info.sender.to_string();
            let token_contract = token_contract_addr.to_string();
            ido.admin = admin;
            ido.start_time = start_time;
            ido.end_time = end_time;
            ido.token_contract = token_contract;
            ido.price = price.u128();
            ido.total_tokens_amount = total_amount.u128();
            ido.soft_cap = soft_cap.u128();
            ido.remaining_tokens_per_tier = tokens_per_tier.into_iter().map(|v| v.u128()).collect();

            if let PaymentMethod::Token {
                contract,
                code_hash,
            } = payment
            {
                let payment_token_contract = contract.to_string();
                ido.payment_token_contract = Some(payment_token_contract);
                ido.payment_token_hash = Some(code_hash);
            }

            start_ido(deps, env, info, ido, whitelist)
        }
        ExecuteMsg::BuyTokens {
            amount,
            ido_id,
            viewing_key,
            ..
        } => buy_tokens(deps, env, info, ido_id, amount.u128(), viewing_key),
        ExecuteMsg::WhitelistAdd {
            addresses, ido_id, ..
        } => whitelist_add(deps, env, info, addresses, ido_id),
        ExecuteMsg::WhitelistRemove {
            addresses, ido_id, ..
        } => whitelist_remove(deps, env, info, addresses, ido_id),
        ExecuteMsg::RecvTokens {
            ido_id,
            start,
            limit,
            purchase_indices,
            ..
        } => recv_tokens(deps, env, info, ido_id, start, limit, purchase_indices),
        ExecuteMsg::Withdraw { ido_id, .. } => withdraw(deps, env, info, ido_id),
    };

    return response
}


fn change_admin(
    deps: DepsMut<SeiQueryWrapper>,
    _env: Env,
    info: MessageInfo,
    admin: String,
) -> Result<Response, ContractError> {
    assert_admin(&deps, &info.sender.to_string())?;

    let mut config = Config::load(deps.storage)?;
    let new_admin = admin.to_string();
    config.admin = new_admin;

    config.save(deps.storage)?;

    Ok(Response::new().add_attribute("action", "changed admin"))
}

fn change_status(
    deps: DepsMut<SeiQueryWrapper>,
    _env: Env,
    info: MessageInfo,
    status: ContractStatus,
) -> Result<Response, ContractError> {
    assert_admin(&deps, &info.sender.to_string())?;

    let mut config = Config::load(deps.storage)?;
    config.status = status as u8;
    config.save(deps.storage)?;

    Ok(Response::new().add_attribute("action", "changed status"))
}

fn start_ido(
    deps: DepsMut<SeiQueryWrapper>,
    env: Env,
    info: MessageInfo,
    mut ido: Ido,
    whitelist: Whitelist,
) -> Result<Response, ContractError> {
    assert_contract_active(deps.storage)?;
    assert_admin(&deps, &info.sender.to_string())?;
    let config = Config::load(deps.storage)?;
    if ido.remaining_tokens_per_tier.len() != config.min_tier as usize {
        return Err(ContractError::Std(StdError::generic_err("`tokens_per_tier` has wrong size"))) 
    }

    let sum = ido.remaining_tokens_per_tier.iter().sum::<u128>();
    if sum < ido.total_tokens_amount {
        return Err(ContractError::Std(StdError::generic_err(
            "Sum of `tokens_per_tier` can't be less than total tokens amount",
        )));
    }

    if ido.start_time >= ido.end_time {
        return Err(ContractError::Std(StdError::generic_err(
            "End time must be greater than start time",
        )));
    }

    if ido.price == 0 {
        return Err(ContractError::Std(StdError::generic_err("Ido price should be initialized")));
    }
    if env.block.time.seconds() >= ido.end_time {
        return Err(ContractError::Std(StdError::generic_err("Ido ends in the past")));
    }

    if ido.soft_cap == 0 {
        return Err(ContractError::Std(StdError::generic_err("soft_cap should be initialized.")));
    }

    if ido.soft_cap > ido.total_tokens_amount {
        return Err(ContractError::Std(StdError::generic_err(
            "soft_cap should be less than total amount",
        )));
    }
    ido.shared_whitelist = match whitelist {
        Whitelist::Shared { .. } => true,
        Whitelist::Empty { .. } => false,
    };

    let ido_id = ido.save(deps.storage)?;
    // let ido_whitelist = state::ido_whitelist(ido_id);

    match whitelist {
        Whitelist::Empty { with } => {
            for address in with.unwrap_or_default() {
                let canonical_address = address.to_string();
                WHITELIST.save(deps.storage, (ido_id, canonical_address), &true)?;
            }
        }
        Whitelist::Shared { with_blocked } => {
            for address in with_blocked.unwrap_or_default() {
                let canonical_address = address.to_string();
                WHITELIST.save(deps.storage, (ido_id, canonical_address), &false)?;
            }
        }
    }

    ido.save(deps.storage)?;

    let canonical_sender = info.sender.to_string();

    let mut startup_ido_list = OWNER_TO_IDOS
        .may_load(deps.storage, canonical_sender)?
        .unwrap_or_default();
    startup_ido_list.push(ido_id);
    OWNER_TO_IDOS.save(deps.storage, info.sender.to_string(), &startup_ido_list)?;
    

    let token_address = ido.token_contract.to_string();
    let transfer_msg = Cw20ExecuteMsg::TransferFrom { 
        owner: info.sender.to_string(), 
        recipient: env.contract.address.to_string(),
        amount: Uint128::new(ido.total_tokens_amount)
    };

    let sub_msg = SubMsg::new(WasmMsg::Execute {
        contract_addr: token_address,
        msg: to_binary(&transfer_msg)?,
        funds: vec![],
    });
    
    let answer = to_binary(&ExecuteResponse::StartIdo {
        ido_id,
        status: ResponseStatus::Success,
    })?;

    Ok(Response::new().set_data(answer).add_submessage(sub_msg))
}

fn buy_tokens(
    deps: DepsMut<SeiQueryWrapper>,
    env: Env,
    info: MessageInfo,
    ido_id: u32,
    mut amount: u128,
    viewing_key: Option<String>,
) -> Result<Response, ContractError> {
    assert_contract_active(deps.storage)?;

    let sender = info.sender.to_string();
    let canonical_sender = sender.to_string();

    let mut ido = Ido::load(deps.storage, ido_id)?;
    if !ido.is_active(env.block.time.seconds()) {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "IDO is not active {}",
            env.block.time,
        ))));
    }

    if ido.is_native_payment() {
        let sei_amount = utils::sent_funds(&info.funds)?;
        amount = sei_amount.checked_mul(ido.price).unwrap();
    }

    if amount == 0 {
        return Err(ContractError::Std(StdError::generic_err("Zero amount")));
    }

    let config = Config::load(deps.storage)?;
    let tier = if utils::in_whitelist(deps.storage, &sender, ido_id)? {
        get_tier(&deps, sender.clone(), viewing_key.clone())?
    } else {
        config.min_tier
    };

    let remaining_amount = ido.remaining_tokens_per_tier(tier);
    if remaining_amount == 0 {
        if ido.total_tokens_amount == ido.sold_amount {
            return Err(ContractError::Std(StdError::generic_err("All tokens are sold")));
        } else {
            return Err(ContractError::Std(StdError::generic_err("All tokens are sold for your tier")));
        }
    }

    if amount > remaining_amount {
        let msg = format!("You cannot buy more than {} tokens", remaining_amount);
        return Err(ContractError::Std(StdError::generic_err(&msg)));
    }

    let payment = amount.checked_div(ido.price).unwrap();
    let lock_period = config.lock_period(tier);

    let unlock_time = ido.end_time.checked_add(lock_period).unwrap();
    let tokens_amount = Uint128::new(amount);
    let purchase = Purchase {
        timestamp: env.block.time.seconds(),
        tokens_amount: tokens_amount.u128(),
        unlock_time,
    };

    let mut purchases = PURCHASES
        .may_load(deps.storage, (canonical_sender.to_string(), ido_id))?
        .unwrap_or_default();
    purchases.push(purchase);
    PURCHASES.save(deps.storage, (canonical_sender.to_string(), ido_id), &purchases)?;

    let mut user_ido_info = IDO_TO_INFO
    .may_load(deps.storage, (canonical_sender.to_string(), ido_id))?
    .unwrap_or_default();

    if user_ido_info.total_payment == 0 {
        ido.participants = ido.participants.checked_add(1).unwrap();
    }

    user_ido_info.total_payment = user_ido_info.total_payment.checked_add(payment).unwrap();
    user_ido_info.total_tokens_bought = user_ido_info
        .total_tokens_bought
        .checked_add(amount)
        .unwrap();

    
    let mut user_info = USERINFO
        .may_load(deps.storage, canonical_sender.to_string())?
        .unwrap_or_default();

    user_info.total_payment = user_info.total_payment.checked_add(payment).unwrap();
    user_info.total_tokens_bought = user_info.total_tokens_bought.checked_add(amount).unwrap();

    USERINFO.save(deps.storage, canonical_sender.to_string(), &user_info)?;

    IDO_TO_INFO.save(deps.storage, (canonical_sender.to_string(), ido_id), &user_ido_info)?;
    
    ACTIVE_IDOS.save(deps.storage, (canonical_sender.to_string(), ido_id), &true)?;

    ido.sold_amount = ido.sold_amount.checked_add(amount).unwrap();
    ido.total_payment = ido.total_payment.checked_add(payment).unwrap();

    let tier_index = tier.checked_sub(1).unwrap() as usize;
    ido.remaining_tokens_per_tier[tier_index] = ido.remaining_tokens_per_tier[tier_index]
        .checked_sub(amount)
        .unwrap();

    ido.save(deps.storage)?;

    let answer = to_binary(&ExecuteResponse::BuyTokens {
        unlock_time,
        amount: Uint128::new(amount),
        status: ResponseStatus::Success,
    })?;

    if !ido.is_native_payment() {
        let token_contract_canonical = ido.payment_token_contract.unwrap();
        // let token_contract_hash = ido.payment_token_hash.unwrap();
        let token_contract = token_contract_canonical.to_string();

        let transfer_msg = Cw20ExecuteMsg::TransferFrom { 
            owner: info.sender.to_string(), 
            recipient: env.contract.address.to_string(),
            amount: Uint128::new(payment)
        };
    
        let sub_msg = SubMsg::new(WasmMsg::Execute {
            contract_addr: token_contract,
            msg: to_binary(&transfer_msg)?,
            funds: vec![],
        });
    
        return Ok(Response::new().set_data(answer).add_submessage(sub_msg));
        
    }
    // else ---> scrt tokens are in the contract itself.
    Ok(Response::new().set_data(answer))
}

fn recv_tokens(
    deps: DepsMut<SeiQueryWrapper>,
    env: Env,
    info: MessageInfo,
    ido_id: u32,
    start: Option<u32>,
    limit: Option<u32>,
    purchase_indices: Option<Vec<u32>>,
) -> Result<Response, ContractError> {
    assert_contract_active(deps.storage)?;
    //
    let canonical_sender = info.sender.to_string();
    let current_time = env.block.time;

    let ido = Ido::load(deps.storage, ido_id)?;
    let mut user_info = USERINFO
        .may_load(deps.storage, canonical_sender.to_string())?
        .unwrap_or_default();
    let mut user_ido_info = IDO_TO_INFO
        .may_load(deps.storage, (canonical_sender.to_string(), ido_id))?
        .unwrap_or_default();

    // when ido failed, withdraw the payment tokens.
    if current_time.seconds() > ido.end_time && ido.soft_cap > ido.sold_amount {
        user_info.total_payment = user_info
            .total_payment
            .checked_sub(user_ido_info.total_payment)
            .unwrap_or_default();
        user_info.total_tokens_bought = user_info
            .total_payment
            .checked_sub(user_ido_info.total_tokens_bought)
            .unwrap_or_default();
        user_ido_info.total_tokens_received = 0;
        user_ido_info.total_tokens_bought = 0;
        user_ido_info.total_payment = 0;

        USERINFO.save(deps.storage, canonical_sender.to_string(), &user_info)?;

        IDO_TO_INFO.save(deps.storage, (canonical_sender.to_string(), ido_id), &user_ido_info)?;
        ACTIVE_IDOS.remove(deps.storage, (canonical_sender.to_string(), ido_id));
    
        let answer = to_binary(&ExecuteResponse::RecvTokens {
            amount: Uint128::new(user_info.total_payment),
            status: ResponseStatus::Success,
            ido_success: false,
        })?;

        if ido.is_native_payment() {
            let transfer_msg = CosmosMsg::Bank(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: coins(user_ido_info.total_payment, USEI),
            });
            return Ok(Response::new().set_data(answer).add_message(transfer_msg));
            
        } else {
            let token_contract_canonical = ido.payment_token_contract.unwrap();
            // let token_contract_hash = ido.payment_token_hash.unwrap();
            let token_contract = token_contract_canonical.to_string();

            let transfer_msg = Cw20ExecuteMsg::TransferFrom { 
                owner: info.sender.to_string(), 
                recipient: env.contract.address.to_string(),
                amount: Uint128::new(user_ido_info.total_payment)
            };
        
            let sub_msg = SubMsg::new(WasmMsg::Execute {
                contract_addr: token_contract,
                msg: to_binary(&transfer_msg)?,
                funds: vec![],
            });
            return Ok(Response::new().set_data(answer).add_submessage(sub_msg));
        };
        
    }
    let start = start.unwrap_or(0);
    let limit = limit.unwrap_or(300);
    let mut purchases = PURCHASES
        .may_load(deps.storage, (canonical_sender.to_string(), ido_id))?
        .unwrap_or_default();
    let purchases_iter = purchases
        .iter()
        .skip(start as usize)
        .take(limit as usize);

    let mut indices = Vec::new();
    for (i, purchase) in purchases_iter.enumerate() {

        if current_time.seconds() >= purchase.unlock_time {
            let index = i.checked_add(start as usize).unwrap();
            indices.push(index);
        }
    }

    if let Some(purchase_indices) = purchase_indices {
        let end = start.checked_add(limit).unwrap();
        for index in purchase_indices {
            if index >= start && index < end {
                continue;
            }

            let purchase = purchases.get(index as usize).unwrap();
            if current_time.seconds() >= purchase.unlock_time {
                indices.push(index as usize);
            }
        }
    }

    indices.sort();
    indices.dedup();

    let mut recv_amount: u128 = 0;

    let mut archived_purchases = ARCHIVED_PURCHASES
    .may_load(deps.storage, (canonical_sender.to_string(), ido_id))?
    .unwrap_or_default();


    for (shift, index) in indices.into_iter().enumerate() {
        let position = index.checked_sub(shift).unwrap();
        let purchase = purchases.remove(position as usize);

        recv_amount = recv_amount.checked_add(purchase.tokens_amount).unwrap();
        archived_purchases.push(purchase);
    }
    PURCHASES.save(deps.storage, (canonical_sender.to_string(), ido_id), &purchases)?;
    ARCHIVED_PURCHASES.save(deps.storage, (canonical_sender.to_string(), ido_id), &archived_purchases)?;

    if recv_amount == 0 {
        return Err(ContractError::Std(StdError::generic_err("Nothing to receive")));
    }

    let answer = to_binary(&ExecuteResponse::RecvTokens {
        amount: Uint128::new(recv_amount),
        status: ResponseStatus::Success,
        ido_success: true,
    })?;

    user_info.total_tokens_received = user_info
        .total_tokens_received
        .checked_add(recv_amount)
        .unwrap();

    user_ido_info.total_tokens_received = user_ido_info
        .total_tokens_received
        .checked_add(recv_amount)
        .unwrap();

    USERINFO.save(deps.storage, canonical_sender.to_string(), &user_info)?;

    IDO_TO_INFO.save(deps.storage, (canonical_sender.to_string(), ido_id), &user_ido_info)?;
    
    
    if user_ido_info.total_tokens_bought == user_ido_info.total_tokens_received {
        ACTIVE_IDOS.remove(deps.storage, (canonical_sender.to_string(), ido_id));
    }

    let token_contract = ido.token_contract.to_string();

    let transfer_msg = Cw20ExecuteMsg::TransferFrom { 
        owner: info.sender.to_string(), 
        recipient: env.contract.address.to_string(),
        amount: Uint128::new(recv_amount)
    };

    let sub_msg = SubMsg::new(WasmMsg::Execute {
        contract_addr: token_contract,
        msg: to_binary(&transfer_msg)?,
        funds: vec![],
    });
    return Ok(Response::new().set_data(answer).add_submessage(sub_msg));
}

fn withdraw(
    deps: DepsMut<SeiQueryWrapper>,
    env: Env,
    info: MessageInfo,
    ido_id: u32,
) -> Result<Response, ContractError> {
    let ido_admin = info.sender.to_string();
    assert_ido_admin(&deps, &ido_admin, ido_id)?;
    assert_contract_active(deps.storage)?;

    let mut ido = Ido::load(deps.storage, ido_id)?;
    if ido.withdrawn {
        return Err(ContractError::Std(StdError::generic_err("Already withdrawn")));
    }

    if env.block.time.seconds() < ido.end_time {
        return Err(ContractError::Std(StdError::generic_err("IDO is not finished yet")));
    }

    ido.withdrawn = true;
    ido.save(deps.storage)?;

    let remaining_tokens: Uint128;
    if ido.soft_cap > ido.sold_amount {
        remaining_tokens = Uint128::from(ido.total_tokens_amount);
    } else {
        remaining_tokens = Uint128::from(ido.remaining_tokens());
    }

    let ido_token_contract = ido.token_contract.to_string();

    let mut msgs = vec![];
    let mut submsgs = vec![];
    if !remaining_tokens.is_zero() {

        let transfer_msg = Cw20ExecuteMsg::TransferFrom { 
            owner: ido_admin.to_string(), 
            recipient: env.contract.address.to_string(),
            amount: remaining_tokens
        };
    
        let sub_msg = SubMsg::new(WasmMsg::Execute {
            contract_addr: ido_token_contract,
            msg: to_binary(&transfer_msg)?,
            funds: vec![],
        });
        
        submsgs.push(sub_msg);
    }
    //withdraw payment tokens.
    let payment_amount = Uint128::new(ido.sold_amount.checked_div(ido.price).unwrap());
    if ido.sold_amount >= ido.soft_cap {
        if ido.is_native_payment() {
            msgs.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: ido_admin,
                amount: coins(ido.sold_amount.checked_div(ido.price).unwrap(), USEI),
            }))
            
        } else {
            let token_contract_canonical = ido.payment_token_contract.unwrap();
            // let token_contract_hash = ido.payment_token_hash.unwrap();
            let token_contract = token_contract_canonical.to_string();
            
            let transfer_msg = Cw20ExecuteMsg::TransferFrom { 
                owner: ido_admin.to_string(), 
                recipient: env.contract.address.to_string(),
                amount: payment_amount
            };
        
            let sub_msg = SubMsg::new(WasmMsg::Execute {
                contract_addr: token_contract,
                msg: to_binary(&transfer_msg)?,
                funds: vec![],
            });
            
            submsgs.push(sub_msg);
        };
        
    }

    let answer = to_binary(&ExecuteResponse::Withdraw {
        ido_amount: remaining_tokens,
        payment_amount: payment_amount,
        status: ResponseStatus::Success,
    })?;

    return Ok(Response::new().set_data(answer).add_messages(msgs).add_submessages(submsgs));
}

fn whitelist_add(
    deps: DepsMut<SeiQueryWrapper>,
    _env: Env,
    info: MessageInfo,
    addresses: Vec<String>,
    ido_id: u32,
) -> Result<Response, ContractError> {
    assert_contract_active(deps.storage)?;
    assert_ido_admin(&deps, &info.sender.to_string(), ido_id)?;

    // let whitelist = state::ido_whitelist(ido_id);
    for address in addresses {
        let canonical_address = address.to_string();
        WHITELIST.save(deps.storage, (ido_id, canonical_address), &true)?;
    }
    

    let answer = to_binary(&ExecuteResponse::WhitelistAdd {
        status: ResponseStatus::Success,
    })?;

    return Ok(Response::new().set_data(answer));
}

fn whitelist_remove(
    deps: DepsMut<SeiQueryWrapper>,
    _env: Env,
    info: MessageInfo,
    addresses: Vec<String>,
    ido_id: u32,
) -> Result<Response, ContractError> {
    assert_contract_active(deps.storage)?;
    assert_ido_admin(&deps, &info.sender.to_string(), ido_id)?;

    // let whitelist = state::ido_whitelist(ido_id);

    for address in addresses {
        let canonical_address = address.to_string();
        WHITELIST.save(deps.storage, (ido_id, canonical_address), &false)?;
    }

    let answer = to_binary(&ExecuteResponse::WhitelistRemove {
        status: ResponseStatus::Success,
    })?;

    return Ok(Response::new().set_data(answer));
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps<SeiQueryWrapper>, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    let response = match msg {
        QueryMsg::Config {} => {
            let config = Config::load(deps.storage)?;
            config.to_answer()?
        }
        QueryMsg::IdoAmount {} => {
            let amount = Ido::len(deps.storage)?;
            QueryResponse::IdoAmount { amount }
        }
        QueryMsg::IdoInfo { ido_id } => {
            let ido = Ido::load(deps.storage, ido_id)?;
            ido.to_answer()?
        }
        QueryMsg::InWhitelist { address, ido_id } => {
            let in_whitelist = utils::in_whitelist(deps.storage, &address, ido_id)?;
            QueryResponse::InWhitelist { in_whitelist }
        }
        QueryMsg::IdoListOwnedBy {
            address,
            start,
            limit,
        } => {
            let canonical_address = address.clone();
            
            let ido_list = OWNER_TO_IDOS
                .may_load(deps.storage, canonical_address.clone())?
                .unwrap_or_default();
            let amount = ido_list.len() as u32;
            let mut ido_ids = Vec::new();
            
            for i in start..start+limit {
                if i < amount {
                    ido_ids.push(ido_list[i as usize])
                }
            }

            QueryResponse::IdoListOwnedBy { ido_ids, amount }
        }
        QueryMsg::Purchases {
            ido_id,
            address,
            start,
            limit,
        } => {
            let canonical_address = address.clone();

            let purchases = PURCHASES
                .may_load(deps.storage, (canonical_address.to_string(), ido_id))?
                .unwrap_or_default();
            let amount = purchases.len() as u32;

            let start = start.unwrap_or(0);
            let limit = limit.unwrap_or(300);
            
            let mut raw_purchases:Vec<Purchase> = Vec::new();
            for i in start..start+limit {
                if i < amount {
                    raw_purchases.push(purchases.get(i as usize).unwrap().clone())
                }
            }

            let purchases = raw_purchases.into_iter().map(|p| p.to_answer()).collect();

            QueryResponse::Purchases { purchases, amount}
        }
        QueryMsg::ArchivedPurchases {
            ido_id,
            address,
            start,
            limit,
        } => {
            let canonical_address = address.clone();
            let purchases = ARCHIVED_PURCHASES
                .may_load(deps.storage, (canonical_address.to_string(), ido_id))?
                .unwrap_or_default();
            let amount = purchases.len() as u32;

            let mut raw_purchases:Vec<Purchase> = Vec::new();
            for i in start..start+limit {
                if i < amount {
                    raw_purchases.push(purchases.get(i as usize).unwrap().clone())
                }
            }

            let purchases = raw_purchases.into_iter().map(|p| p.to_answer()).collect();

            QueryResponse::ArchivedPurchases { purchases, amount }
        }
        QueryMsg::UserInfo { address, ido_id } => {
            let canonical_address = address.clone();



            let user_info = if let Some(ido_id) = ido_id {
                IDO_TO_INFO
                    .may_load(deps.storage, (canonical_address.to_string(), ido_id))?
                    .unwrap_or_default()
            } else {
                USERINFO
                    .may_load(deps.storage, canonical_address.to_string())?
                    .unwrap_or_default()
            };

            user_info.to_answer()
        }
        
    };
    to_binary(&response)
}

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::tier::manual;

    use super::*;
    use cosmwasm_std::testing::{  MockStorage, MockApi, MockQuerier, mock_env, mock_info};
    use cosmwasm_std::{OwnedDeps, from_binary};
    use rand::{thread_rng, Rng};

    fn get_init_msg() -> InitMsg {
        InitMsg {
            admin: None,
            tier_contract: "tier".to_string(),
            nft_contract: "nft".to_string(),
            lock_periods: vec![250, 200, 150, 100],
        }
    }

    fn initialize_with(msg: InitMsg) -> Result<OwnedDeps<cosmwasm_std::MemoryStorage, MockApi, MockQuerier, SeiQueryWrapper>, ContractError> {
        let mut deps = OwnedDeps {
            storage: MockStorage::default(),
            api: MockApi::default(),
            querier: MockQuerier::default(),
            custom_query_type: PhantomData::<SeiQueryWrapper>,
        };
        let info: MessageInfo = mock_info("admin", &coins(2, "usei"));

        instantiate(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
        
        Ok(deps)
    }

    fn initialize_with_default() -> OwnedDeps<cosmwasm_std::MemoryStorage, MockApi, MockQuerier, SeiQueryWrapper> {
        let msg = get_init_msg();
        initialize_with(msg).unwrap()
    }

    fn start_ido_msg() -> ExecuteMsg {
        let mut rng = thread_rng();
        let token_contract = format!("token_{}", rng.gen_range(0..1000));

        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let end_time = start_time + rng.gen::<u64>();

        let price = rng.gen();
        let total_amount = rng.gen();

        let mut whitelist = Vec::new();
        for i in 0..rng.gen_range(20..100) {
            let address = format!("address_{}", i);
            whitelist.push(address);
        }

        let mut tokens_per_tier = Vec::new();
        let mut remaining_tokens = total_amount;
        for _ in 0..3 {
            let tokens_amount = rng.gen_range(0..=remaining_tokens);
            tokens_per_tier.push(Uint128::new(tokens_amount));
            remaining_tokens -= tokens_amount;
        }
        tokens_per_tier.push(Uint128::new(remaining_tokens));

        ExecuteMsg::StartIdo {
            start_time,
            end_time,
            token_contract: token_contract,
            payment: PaymentMethod::Token {
                contract: "token".to_string(),
                code_hash: String::from("token_hash"),
            },
            price: Uint128::new(price),
            total_amount: Uint128::new(total_amount),
            soft_cap: Uint128::new(total_amount),
            whitelist: Whitelist::Empty {
                with: Some(whitelist),
            },
            tokens_per_tier,
            padding: None,
        }
    }

    fn extract_error(response: Result<Response, ContractError>) -> String {
        match response {
            Ok(_) => panic!("Response is not an error"),
            Err(err) => match err {
                ContractError::Std(StdError::GenericErr { msg, .. }) => msg,
                ContractError::Unauthorized{ .. } => "Unauthorized".into(),
                _ => panic!("Unexpected error"),
            },
        }
    }

    #[test]
    fn initialize() {

        let msg = get_init_msg();
        let mut deps = initialize_with(msg.clone()).unwrap();

        let config: Config = Config::load(&deps.storage).unwrap();
        
        let min_tier = manual::get_min_tier(&deps.as_mut(), &config).unwrap();

        let admin = "admin".to_string();
        
        assert_eq!(config.admin, admin);
        assert_eq!(config.lock_periods, msg.lock_periods);
        assert_eq!(config.tier_contract, msg.tier_contract.to_string());
        assert_eq!(config.nft_contract, msg.nft_contract.to_string());
        assert_eq!(config.min_tier, min_tier);
    }

    #[test]
    fn initialize_with_wrong_lock_periods() {

        let mut msg = get_init_msg();
        msg.lock_periods = vec![1, 2, 3];

        let mut deps = OwnedDeps {
            storage: MockStorage::default(),
            api: MockApi::default(),
            querier: MockQuerier::default(),
            custom_query_type: PhantomData::<SeiQueryWrapper>,
        };
        let info: MessageInfo = mock_info("admin", &coins(2, "usei"));

        let response = instantiate(deps.as_mut(), mock_env(), info, msg.clone());
        let error = extract_error(response);

        assert!(error.contains("Lock periods array must have 4 items"));
    }

    #[test]
    fn start_ido() {
        let mut deps = initialize_with_default();

        let ido_admin = "admin".to_string();
        let canonical_ido_admin = ido_admin.to_string();
        let info: MessageInfo = mock_info(&ido_admin, &[]);
        let env = mock_env();
        let msg = start_ido_msg();
        
        let startup_ido_list = OWNER_TO_IDOS
            .may_load(&deps.storage, canonical_ido_admin.clone())
            .unwrap_or_default()
            .unwrap_or_default();
        assert_eq!(startup_ido_list.len(), 0);
        assert_eq!(Ido::len(&deps.storage), Ok(0));

        let response =
            execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        let messages = response.messages;
        let data = response.data;

        match from_binary(&data.unwrap()).unwrap() {
            ExecuteResponse::StartIdo { ido_id, status, .. } => {
                assert_eq!(ido_id, 0);
                assert_eq!(status, ResponseStatus::Success);
            }
            _ => unreachable!(),
        }

        assert_eq!(Ido::len(&deps.storage), Ok(1));
        let ido = Ido::load(&deps.storage, 0).unwrap();

        let startup_ido_list = OWNER_TO_IDOS
            .may_load(&deps.storage, canonical_ido_admin)
            .unwrap_or_default()
            .unwrap_or_default();

        assert_eq!(startup_ido_list.len(), 1);
        

        if let ExecuteMsg::StartIdo {
            start_time,
            end_time,
            token_contract,
            price,
            total_amount,
            whitelist: _whitelist,
            payment,
            ..
        } = msg
        {
            let sender = info.sender.to_string();
            let token_contract_canonical = token_contract.to_string();

            let payment_token_contract_canonical = match payment {
                PaymentMethod::Native => None,
                PaymentMethod::Token { contract, .. } => Some(contract),
            };

            assert_eq!(ido.admin, sender);
            assert_eq!(ido.start_time, start_time);
            assert_eq!(ido.end_time, end_time);
            assert_eq!(ido.token_contract, token_contract_canonical);
            assert_eq!(ido.price, price.u128());
            assert_eq!(ido.participants, 0);
            assert_eq!(ido.sold_amount, 0);
            assert_eq!(ido.total_tokens_amount, total_amount.u128());
            assert_eq!(ido.payment_token_contract, payment_token_contract_canonical);
            assert_eq!(ido.payment_token_hash, Some(String::from("token_hash")));

            let transfer_msg = Cw20ExecuteMsg::TransferFrom { 
                owner: info.sender.to_string(), 
                recipient: env.contract.address.to_string(),
                amount: total_amount
            };
        
            let sub_msg = SubMsg::new(WasmMsg::Execute {
                contract_addr: token_contract,
                msg: to_binary(&transfer_msg).unwrap(),
                funds: vec![],
            });
            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0], sub_msg);
        } else {
            unreachable!();
        }
    }

}