use crate::{
    msg::ContractStatus,
    state::{ Config, Ido, CONFIG_KEY, WHITELIST},
};
use cosmwasm_std::{
    Coin,  StdError, StdResult, Storage, DepsMut,
};
use sei_cosmwasm::SeiQueryWrapper;

pub fn assert_contract_active(storage: &dyn Storage) -> StdResult<()> {
    let config = Config::load(storage)?;
    let active_status = ContractStatus::Active as u8;

    if config.status != active_status {
        return Err(StdError::generic_err("Contract is not active"));
    }

    Ok(())
}

pub fn assert_admin(
    deps: &DepsMut<SeiQueryWrapper>,
    address: &String,
) -> StdResult<()> {
    let canonical_admin = address.clone();
    let config = CONFIG_KEY.load(deps.storage)?;

    if config.admin != canonical_admin {
        return Err(StdError::generic_err("Unauthorized"));
    }

    Ok(())
}

pub fn assert_ido_admin(
    deps: &DepsMut<SeiQueryWrapper>,
    address: &String,
    ido_id: u32,
) -> StdResult<()> {
    let canonical_admin = address.clone();
    let ido = Ido::load(deps.storage, ido_id)?;

    if ido.admin != canonical_admin {
        return Err(StdError::generic_err("Unauthorized"));
    }

    Ok(())
}

pub fn in_whitelist(
    storage: &dyn Storage,
    address: &String,
    ido_id: u32,
) -> StdResult<bool> {
    let canonical_address = address.clone();

    let whitelist_status = WHITELIST.may_load(storage, (ido_id, canonical_address))?;

    match whitelist_status {
        Some(value) => Ok(value),
        None => {
            let ido = Ido::load(storage, ido_id)?;
            Ok(ido.shared_whitelist)
        }
    }
}

pub fn sent_funds(coins: &[Coin]) -> StdResult<u128> {
    let mut amount: u128 = 0;

    for coin in coins {
        if coin.denom != "usei" {
            return Err(StdError::generic_err("Unsopported token"));
        }

        amount = amount.checked_add(coin.amount.u128()).unwrap();
    }

    Ok(amount)
}
