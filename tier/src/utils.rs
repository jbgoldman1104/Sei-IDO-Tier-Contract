
use crate::contract::USEI;
use cosmwasm_std::{
     Coin,  Env, FullDelegation, StdResult, DepsMut, Addr,
};
use sei_cosmwasm::SeiQueryWrapper;
use serde::Deserialize;



#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct FixedDelegationResponse {
    pub _delegation: Option<FixedFullDelegation>,
}

#[derive(Debug, Deserialize)]
pub struct FixedFullDelegation {
    pub delegator: String,
    pub validator: String,
    pub amount: Coin,
    pub can_redelegate: Coin,
    pub accumulated_rewards: Vec<Coin>,
}

impl From<FixedFullDelegation> for FullDelegation {
    fn from(val: FixedFullDelegation) -> Self {
        let found_rewards = val
            .accumulated_rewards
            .into_iter()
            .find(|r| r.denom == USEI);

        let accumulated_rewards = found_rewards.unwrap_or_else(|| Coin::new(0, USEI));
        FullDelegation {
            delegator: Addr::unchecked(val.delegator),
            validator: val.validator,
            amount: val.amount,
            can_redelegate: val.can_redelegate,
            accumulated_rewards: vec![accumulated_rewards],
        }
    }
}

pub fn query_delegation(
    deps: &DepsMut<SeiQueryWrapper>,
    env: &Env,
    validator: &String,
) -> StdResult<Option<FullDelegation>> {
    let delegation = deps.querier.query_delegation(&env.contract.address, validator)?;

    Ok(delegation)
}
