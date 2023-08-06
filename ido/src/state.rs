use crate::msg::{PaymentMethod, PurchaseAnswer, QueryResponse};
use cosmwasm_std::{StdResult, Storage, Uint128, Order};
use cw_storage_plus::{Item, Map};
use serde::{Deserialize, Serialize};
use std::cmp::min;


pub const CONFIG_KEY: Item<Config> = Item::new("config");
pub const PURCHASES: Map<(String, u32), Vec<Purchase>> = Map::new("purchase");//Deque<UserWithdrawal> = Deque::new("withdraw");
pub const ARCHIVED_PURCHASES: Map<(String, u32), Vec<Purchase>> = Map::new("archive");
pub const ACTIVE_IDOS: Map<(String, u32), bool> = Map::new("active_idos");
pub const IDO_TO_INFO: Map<(String, u32), UserInfo> = Map::new("ido2info");
pub const OWNER_TO_IDOS: Map<String, Vec<u32>> = Map::new("owner2idos");
pub const WHITELIST: Map<(u32, String), bool> = Map::new("whitelist");
pub const USERINFO: Map<String, UserInfo> = Map::new("usr2info");
pub const IDO_ITEM: Map<u32, Ido> = Map::new("ido_list");


// pub fn ido_whitelist(ido_id: u32, storage: &dyn Storage) -> Map<String, bool> {

//     let key = format!("whitelist_{}", ido_id);

//     WHITELIST
//         .may_load(storage, ido_id)?
//         .unwrap_or(Map::new(&key))
// }

// pub fn active_ido_list(user: &String, storage: &dyn Storage) -> Map<'static,u32, bool> {
//     let key = format!("active_idos_{}", user);

//     ACTIVE_IDOS
//         .may_load(storage, user)?
//         .unwrap_or(Map::new(&key))

// }

// pub fn user_info() -> Map<'static, String, UserInfo> {
//     return USERINFO;
// }

// pub fn user_info_in_ido(user: &String, storage: &dyn Storage) -> Map<'static, u32, UserInfo> {
//     let key = format!("ido2info_{}", user);

//     IDO_TO_INFO
//         .may_load(storage, user)?
//         .unwrap_or(Map::new(&key))
// }

// pub fn purchases(user: &String, ido_id: u32, storage: &dyn Storage) -> Vec<Purchase> {
//     let key = format!("purchase_{}", user);

//     PURCHASES
//         .may_load(storage, (user, ido_id))?
//         .unwrap_or(Vec::new())
// }

// pub fn archived_purchases(user: &String, ido_id: u32, storage: &dyn Storage) -> Vec<Purchase> {
//     ARCHIVED_PURCHASES
//         .add_suffix(user.as_slice())
//         .add_suffix(&ido_id.to_le_bytes())
// }

// pub fn ido_list_owned_by(ido_admin: &String, storage: &dyn Storage) -> Vec<u32> {
//     OWNER_TO_IDOS.add_suffix(ido_admin.as_slice())
// }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub admin: String,
    pub status: u8,
    pub tier_contract: String,
    pub nft_contract: String,
    pub lock_periods: Vec<u64>,
    pub min_tier: u8,
}

impl Config {
    pub fn load(storage: &dyn Storage) -> StdResult<Self> {
        CONFIG_KEY.load(storage)
    }

    pub fn save(&self, storage: &mut dyn Storage) -> StdResult<()> {
        CONFIG_KEY.save(storage, self)
    }

    pub fn lock_period(&self, tier: u8) -> u64 {
        let tier_index = tier.checked_sub(1).unwrap();
        self.lock_periods[tier_index as usize]
    }

    pub fn to_answer(self) -> StdResult<QueryResponse> {
        let admin = self.admin.to_string();
        let tier_contract = self.tier_contract.to_string();
        let nft_contract = self.nft_contract.to_string();

        Ok(QueryResponse::Config {
            admin,
            tier_contract,
            nft_contract,
            lock_periods: self.lock_periods,
        })
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Purchase {
    pub tokens_amount: u128,
    pub timestamp: u64,
    pub unlock_time: u64,
}

impl Purchase {
    pub fn to_answer(&self) -> PurchaseAnswer {
        PurchaseAnswer {
            tokens_amount: Uint128::new(self.tokens_amount),
            timestamp: self.timestamp,
            unlock_time: self.unlock_time,
        }
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserInfo {
    pub total_payment: u128,
    pub total_tokens_bought: u128,
    pub total_tokens_received: u128,
}

impl UserInfo {
    pub fn to_answer(&self) -> QueryResponse {
        QueryResponse::UserInfo {
            total_payment: Uint128::new(self.total_payment),
            total_tokens_bought: Uint128::new(self.total_tokens_bought),
            total_tokens_received: Uint128::new(self.total_tokens_received),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Ido {
    #[serde(skip)]
    id: Option<u32>,
    pub admin: String,
    pub start_time: u64,
    pub end_time: u64,
    pub token_contract: String,
    pub token_contract_hash: String,
    pub payment_token_contract: Option<String>,
    pub payment_token_hash: Option<String>,
    pub price: u128,
    pub participants: u64,
    pub sold_amount: u128,
    pub remaining_tokens_per_tier: Vec<u128>,
    pub total_tokens_amount: u128,
    pub soft_cap: u128,
    pub total_payment: u128,
    pub withdrawn: bool,
    pub shared_whitelist: bool,
}

impl Ido {
    
    pub fn load(storage: &dyn Storage, id: u32) -> StdResult<Self> {
        let mut ido = IDO_ITEM.may_load(storage, id)?.unwrap_or_default();
        ido.id = Some(id);
        Ok(ido)
    }

    pub fn len(storage: &dyn Storage) -> StdResult<u32> {
        let len =IDO_ITEM.keys(storage, None, None, Order::Ascending).count();
        Ok(len as u32)
    }

    pub fn save(&mut self, storage: &mut dyn  Storage) -> StdResult<u32> {

        let id = if let Some(id) = self.id {
            id
        } else {
            let id = Self::len(storage)?;
            self.id = Some(id);
            id
        };

        IDO_ITEM.save(storage, id, self)?;

        Ok(id)
    }

    pub fn id(&self) -> u32 {
        self.id.unwrap()
    }

    pub fn is_stored(&self) -> bool {
        self.id.is_some()
    }

    pub fn is_active(&self, current_time: u64) -> bool {
        current_time >= self.start_time && current_time < self.end_time
    }

    pub fn is_native_payment(&self) -> bool {
        self.payment_token_contract.is_none() && self.payment_token_hash.is_none()
    }

    pub fn remaining_tokens(&self) -> u128 {
        self.total_tokens_amount
            .checked_sub(self.sold_amount)
            .unwrap()
    }

    pub fn remaining_tokens_per_tier(&self, tier: u8) -> u128 {
        let tier_index = tier.checked_sub(1).unwrap() as usize;
        let remaining_tokens_per_tier = self.remaining_tokens_per_tier[tier_index];
        let remaining_total_amount = self.remaining_tokens();

        min(remaining_tokens_per_tier, remaining_total_amount)
    }

    pub fn to_answer(&self) -> StdResult<QueryResponse> {
        let admin = self.admin.to_string();
        let token_contract = self.token_contract.to_string();

        let payment = if self.is_native_payment() {
            PaymentMethod::Native
        } else {
            let payment_contract =self.payment_token_contract.clone().unwrap();
            let payment_contract_hash = self.payment_token_hash.clone().unwrap();

            PaymentMethod::Token {
                contract: payment_contract,
                code_hash: payment_contract_hash,
            }
        };
        let mut remaining_per_tiers: Vec<Uint128> = vec![];
        for tier in 1..=(self.remaining_tokens_per_tier.len() as u8) {
            remaining_per_tiers.push(Uint128::new(self.remaining_tokens_per_tier(tier)));
        }
        Ok(QueryResponse::IdoInfo {
            admin,
            start_time: self.start_time,
            end_time: self.end_time,
            token_contract,
            price: Uint128::new(self.price),
            payment,
            remaining_per_tiers,
            participants: self.participants,
            sold_amount: Uint128::new(self.sold_amount),
            total_tokens_amount: Uint128::new(self.total_tokens_amount),
            total_payment: Uint128::new(self.total_payment),
            soft_cap: Uint128::new(self.soft_cap),
            withdrawn: self.withdrawn,
            shared_whitelist: self.shared_whitelist,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::testing::mock_dependencies;

    #[test]
    fn ido() {
        let deps = mock_dependencies();
        let mut storage = deps.storage;

        assert_eq!(Ido::len(&storage), Ok(0));

        let token_address: String = "token".to_string();
        let canonical_token_address = token_address.clone();

        let mut new_ido = Ido {
            start_time: 100,
            end_time: 150,
            token_contract: canonical_token_address,
            price: 100,
            total_tokens_amount: 1000,
            ..Ido::default()
        };

        assert!(!new_ido.is_stored());
        assert_eq!(Ido::len(&storage), Ok(0));

        new_ido.save(&mut storage).unwrap();
        assert!(new_ido.is_stored());
        assert_eq!(new_ido.id(), 0);
        assert_eq!(Ido::len(&storage), Ok(1));

        new_ido.save(&mut storage).unwrap();
        assert!(new_ido.is_stored());
        assert_eq!(new_ido.id(), 0);
        assert_eq!(Ido::len(&storage), Ok(1));

        let mut loaded_ido = Ido::load(&storage, 0).unwrap();
        assert_eq!(new_ido, loaded_ido);

        loaded_ido.save(&mut storage).unwrap();
        assert!(loaded_ido.is_stored());
        assert_eq!(new_ido, loaded_ido);
        assert_eq!(loaded_ido.id(), 0);
        assert_eq!(Ido::len(&storage), Ok(1));

        loaded_ido.id = None;
        loaded_ido.save(&mut storage).unwrap();
        assert!(loaded_ido.is_stored());
        assert_eq!(loaded_ido.id(), 1);
        assert_eq!(Ido::len(&storage), Ok(2));
    }
}
