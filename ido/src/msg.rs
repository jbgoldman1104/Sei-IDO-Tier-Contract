use cosmwasm_std::Uint128;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Success,
    Failure,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ContractStatus {
    Active,
    Stopped,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct NftToken {
    pub token_id: String,
    pub viewing_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InitMsg {
    pub admin: Option<String>,
    pub lock_periods: Vec<u64>,
    pub tier_contract: String,
    pub nft_contract: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    Native,
    Token {
        contract: String,
        code_hash: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Whitelist {
    Empty {
        with: Option<Vec<String>>,
    },
    Shared {
        with_blocked: Option<Vec<String>>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    ChangeAdmin {
        admin: String,
        padding: Option<String>,
    },
    ChangeStatus {
        status: ContractStatus,
        padding: Option<String>,
    },
    StartIdo {
        start_time: u64,
        end_time: u64,
        token_contract: String,
        price: Uint128,
        soft_cap: Uint128,
        payment: PaymentMethod,
        total_amount: Uint128,
        tokens_per_tier: Vec<Uint128>,
        padding: Option<String>,
        whitelist: Whitelist,
    },
    WhitelistAdd {
        addresses: Vec<String>,
        ido_id: u32,
        padding: Option<String>,
    },
    WhitelistRemove {
        addresses: Vec<String>,
        ido_id: u32,
        padding: Option<String>,
    },
    BuyTokens {
        ido_id: u32,
        amount: Uint128,
        viewing_key: Option<String>,
        padding: Option<String>,
    },
    RecvTokens {
        ido_id: u32,
        start: Option<u32>,
        limit: Option<u32>,
        purchase_indices: Option<Vec<u32>>,
        padding: Option<String>,
    },
    Withdraw {
        ido_id: u32,
        padding: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteResponse {
    ChangeAdmin {
        status: ResponseStatus,
    },
    ChangeStatus {
        status: ResponseStatus,
    },
    StartIdo {
        ido_id: u32,
        status: ResponseStatus,
    },
    WhitelistAdd {
        status: ResponseStatus,
    },
    WhitelistRemove {
        status: ResponseStatus,
    },
    BuyTokens {
        amount: Uint128,
        unlock_time: u64,
        status: ResponseStatus,
    },
    RecvTokens {
        amount: Uint128,
        status: ResponseStatus,
        ido_success: bool,
    },
    Withdraw {
        ido_amount: Uint128,
        payment_amount: Uint128,
        status: ResponseStatus,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    IdoAmount {},
    IdoInfo {
        ido_id: u32,
    },
    InWhitelist {
        address: String,
        ido_id: u32,
    },
    IdoListOwnedBy {
        address: String,
        start: u32,
        limit: u32,
    },
    Purchases {
        ido_id: u32,
        address: String,
        start: Option<u32>,
        limit: Option<u32>,
    },
    ArchivedPurchases {
        ido_id: u32,
        address: String,
        start: u32,
        limit: u32,
    },
    UserInfo {
        address: String,
        ido_id: Option<u32>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PurchaseAnswer {
    pub tokens_amount: Uint128,
    pub timestamp: u64,
    pub unlock_time: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryResponse {
    Config {
        admin: String,
        tier_contract: String,
        nft_contract: String,
        lock_periods: Vec<u64>,
    },
    IdoAmount {
        amount: u32,
    },
    IdoInfo {
        admin: String,
        start_time: u64,
        end_time: u64,
        token_contract: String,
        price: Uint128,
        participants: u64,
        payment: PaymentMethod,
        sold_amount: Uint128,
        total_tokens_amount: Uint128,
        total_payment: Uint128,
        soft_cap: Uint128,
        withdrawn: bool,
        shared_whitelist: bool,
        remaining_per_tiers: Vec<Uint128>,
    },
    InWhitelist {
        in_whitelist: bool,
    },
    IdoListOwnedBy {
        ido_ids: Vec<u32>,
        amount: u32,
    },
    Purchases {
        purchases: Vec<PurchaseAnswer>,
        amount: u32,
    },
    ArchivedPurchases {
        purchases: Vec<PurchaseAnswer>,
        amount: u32,
    },
    UserInfo {
        total_payment: Uint128,
        total_tokens_bought: Uint128,
        total_tokens_received: Uint128,
    },
    TierInfo {
        tier: u8,
        nft_tier: u8,
    },
}



