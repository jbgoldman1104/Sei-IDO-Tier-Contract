use cosmwasm_std::{Uint128};
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
#[repr(u8)]
pub enum ContractStatus {
    Active,
    Stopped,
}

impl From<u8> for ContractStatus {
    fn from(status: u8) -> Self {
        if status == ContractStatus::Active as u8 {
            ContractStatus::Active
        } else if status == ContractStatus::Stopped as u8 {
            ContractStatus::Stopped
        } else {
            panic!("Wrong status");
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    pub validator: String,
    pub deposits: Vec<Uint128>,
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
    Deposit {
        padding: Option<String>,
    },
    Withdraw {
        padding: Option<String>,
    },
    Claim {
        recipient: Option<String>,
        start: Option<u32>,
        limit: Option<u32>,
        padding: Option<String>,
    },
    WithdrawRewards {
        recipient: Option<String>,
        padding: Option<String>,
    },
    Redelegate {
        validator_address: String,
        recipient: Option<String>,
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
    Deposit {
        usd_deposit: Uint128,
        sei_deposit: Uint128,
        tier: u8,
        status: ResponseStatus,
    },
    Withdraw {
        status: ResponseStatus,
    },
    Claim {
        amount: Uint128,
        status: ResponseStatus,
    },
    WithdrawRewards {
        amount: Uint128,
        status: ResponseStatus,
    },
    Redelegate {
        amount: Uint128,
        status: ResponseStatus,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    UserInfo {
        Stringess: String,
    },
    Withdrawals {
        Stringess: String,
        start: Option<u32>,
        limit: Option<u32>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SerializedWithdrawals {
    pub amount: Uint128,
    pub claim_time: u64,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryResponse {
    Config {
        admin: String,
        validator: String,
        status: ContractStatus,
        usd_deposits: Vec<Uint128>,
        min_tier: u8,
    },
    UserInfo {
        tier: u8,
        timestamp: u64,
        usd_deposit: Uint128,
        scrt_deposit: Uint128,
    },
    Withdrawals {
        amount: u32,
        withdrawals: Vec<SerializedWithdrawals>,
    },
}
