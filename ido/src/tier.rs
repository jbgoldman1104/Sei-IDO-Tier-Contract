#[cfg(not(test))]
mod query {
    use crate::{ msg::ContractStatus, state::Config};
    use cosmwasm_std::{ StdError, StdResult,  Uint128, DepsMut};
    use cw721::{
        AllNftInfoResponse, 
        TokensResponse, Cw721QueryMsg,
    };
    use schemars::JsonSchema;
    // use secret_toolkit_snip721::{
    //     all_nft_info_query, private_metadata_query, tokens_query, Extension, Metadata, ViewerInfo,
    // };
    
    use serde::{Deserialize, Serialize};
    use sei_cosmwasm::SeiQueryWrapper;

    #[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
    pub struct Trait {
        // pub display_type: Option<String>,
        pub trait_type: String,
        pub value: String,
    }

    #[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
    pub struct Metadata {
        // pub image: Option<String>,
        // pub image_data: Option<String>,
        // pub external_url: Option<String>,
        // pub description: Option<String>,
        // pub name: Option<String>,
        pub attributes: Option<Vec<Trait>>,
        // pub background_color: Option<String>,
        // pub animation_url: Option<String>,
        // pub youtube_url: Option<String>,
    }

    // pub type Extension = Option<Metadata>;

    #[derive(Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum TierContractQuery {
        Config {},
        UserInfo { address: String },
    }

    // impl Query for TierContractQuery {
    //     const BLOCK_SIZE: usize = 256;
    // }

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum TierResponse {
        UserInfo {
            tier: u8,
        },
        Config {
            admin: String,
            validator: String,
            status: ContractStatus,
            usd_deposits: Vec<Uint128>,
            min_tier: u8,
        },
    }

    fn find_tier_in_metadata(metadata: Metadata) -> Option<u8> {
        let attrubutes = metadata.attributes.unwrap_or_default();

        for attribute in attrubutes {
            let trait_type = attribute.trait_type.to_lowercase();
            if trait_type != "id" {
                continue;
            }

            let tier = match attribute.value.as_str() {
                "XYZA" => 1,
                "XYZB" => 2,
                "XYZC" => 3,
                "XYZD" => 4,
                _ => 5,
            };
            return Some(tier);
        }

        Some(4)
    }

    pub fn get_tier_from_nft_contract(
        deps: &DepsMut<SeiQueryWrapper>,
        address: &String,
        config: &Config,
        _viewing_key: String,
    ) -> StdResult<Option<u8>> {
        let nft_contract = config.nft_contract.to_string();

        let msg = Cw721QueryMsg::Tokens { owner: address.clone(), start_after: None, limit: None };

        let tokensresponse:TokensResponse =  deps.querier.query_wasm_smart(nft_contract, &msg)?;
        
        let token_list = tokensresponse.tokens.iter();
        let mut result_tier = 5;
        for token_id in token_list {
            let nft_contract = config.nft_contract.to_string();
            let msg = Cw721QueryMsg::AllNftInfo { token_id: token_id.clone(), include_expired: Some(false) };
            let nft_info:AllNftInfoResponse<Metadata> =  deps.querier.query_wasm_smart(nft_contract, &msg)?;


            if nft_info.access.owner != address.to_string() {
                continue;
            }

            let public_metadata = nft_info.info;
            let tier = find_tier_in_metadata(public_metadata.extension);
            if let Some(tier) = tier {
                if tier < result_tier {
                    result_tier = tier;
                }
                continue;
            }

        }
        return Ok(Some(result_tier));
    }

    fn get_tier_from_tier_contract(
        deps: &DepsMut<SeiQueryWrapper>,
        address: String,
        config: &Config,
    ) -> StdResult<u8> {
        let tier_contract = config.tier_contract.to_string();
        let user_info = TierContractQuery::UserInfo { address };

        if let TierResponse::UserInfo { tier } = deps.querier.query_wasm_smart(tier_contract, &user_info)? {
            Ok(tier)
        } else {
            Err(StdError::generic_err("Cannot get tier"))
        }
    }

    pub fn get_tier(
        deps: &DepsMut<SeiQueryWrapper>,
        address: String,
        viewing_key: Option<String>,
    ) -> StdResult<u8> {
        let config = Config::load(deps.storage)?;

        let from_nft_contract = viewing_key
            .map(|viewing_key| get_tier_from_nft_contract(deps.clone(), &address, &config, viewing_key))
            .unwrap_or(Ok(None))?;

        let mut tier = get_tier_from_tier_contract(deps, address, &config)?;
        if let Some(nft_tier) = from_nft_contract {
            if nft_tier < tier {
                tier = nft_tier
            }
        }

        Ok(tier)
    }

    pub fn get_min_tier(
        deps: &DepsMut<SeiQueryWrapper>,
        config: &Config,
    ) -> StdResult<u8> {
        let tier_contract = config.tier_contract.to_string();
        let user_info = TierContractQuery::Config {};

        if let TierResponse::Config { min_tier, .. } = deps.querier.query_wasm_smart(tier_contract, &user_info)? {
            Ok(min_tier)
        } else {
            Err(StdError::generic_err("Cannot get min tier"))
        }
    }
}


#[cfg(test)]
pub mod manual {
    use crate::state::Config;
    use cosmwasm_std::{ StdResult,  DepsMut};
    use sei_cosmwasm::SeiQueryWrapper;
    use std::sync::Mutex;

    static TIER: Mutex<u8> = Mutex::new(0);
    static MIN_TIER: Mutex<u8> = Mutex::new(4);

    pub fn set_tier(tier: u8) {
        let mut tier_lock = TIER.lock().unwrap();
        *tier_lock = tier;
    }

    pub fn set_min_tier(tier: u8) {
        let mut tier_lock = MIN_TIER.lock().unwrap();
        *tier_lock = tier;
    }

    pub fn get_tier(
        _deps: &DepsMut<SeiQueryWrapper>,
        _address: String,
        _viewing_key: Option<String>,
    ) -> StdResult<u8> {
        let tier_lock = TIER.lock().unwrap();
        Ok(*tier_lock)
    }

    pub fn get_min_tier(
        _deps: &DepsMut<SeiQueryWrapper>,
        _config: &Config,
    ) -> StdResult<u8> {
        let tier_lock = MIN_TIER.lock().unwrap();
        Ok(*tier_lock)
    }

    pub fn get_tier_from_nft_contract(
        _deps: &DepsMut<SeiQueryWrapper>,
        _address: &String,
        _config: &Config,
        _viewing_key: String,
    ) -> StdResult<Option<u8>> {
        let tier_lock = TIER.lock().unwrap();
        Ok(Some(*tier_lock))
    }
}

#[cfg(not(test))]
pub use query::get_tier;

#[cfg(not(test))]
pub use query::get_min_tier;

#[cfg(not(test))]
pub use query::get_tier_from_nft_contract;

#[cfg(test)]
pub use manual::get_tier;

#[cfg(test)]
pub use manual::get_min_tier;

#[cfg(test)]
pub use manual::get_tier_from_nft_contract;

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;

    use cosmwasm_std::{OwnedDeps, testing::{MockStorage, MockApi, MockQuerier}};
    use sei_cosmwasm::SeiQueryWrapper;

    use super::manual::{get_tier, set_tier};

    #[test]
    fn manual_tier() {
        let mut deps = OwnedDeps {
            storage: MockStorage::default(),
            api: MockApi::default(),
            querier: MockQuerier::default(),
            custom_query_type: PhantomData::<SeiQueryWrapper>,
        };
        let address = "address".to_string();
        let tier = get_tier(&deps.as_mut(), address.clone(), None).unwrap();

        for i in 1..=4 {
            set_tier(i);
            assert_eq!(get_tier(&deps.as_mut(), address.clone(), None), Ok(i));
        }
        set_tier(tier);
    }
}

