use cosmwasm_std::{  StdResult,Decimal};

use sei_cosmwasm::{SeiQuerier, ExchangeRatesResponse, SeiQueryWrapper};
use cosmwasm_std::DepsMut;

pub struct BandProtocol {
    sei_per_usd: u128 ,
}

impl BandProtocol {
    pub const DECIMALS: u8 = 18;
    pub const ONE_USD: u128 = 1_000_000_000_000_000_000;

    pub fn new(deps: &DepsMut<SeiQueryWrapper>) -> StdResult<Self> {

        let querier: SeiQuerier<'_> = SeiQuerier::new(&deps.querier);
        let res = querier.query_exchange_rates().unwrap_or(ExchangeRatesResponse { denom_oracle_exchange_rate_pairs: vec![
        ], });
        
        
        let mut sei_per_usd = Self::ONE_USD / 2;
        for exratepair in res.denom_oracle_exchange_rate_pairs {
            if exratepair.denom.clone() == "usei" {
                let rate =  exratepair.oracle_exchange_rate.exchange_rate;
                sei_per_usd = (Decimal::raw(1000000u128) / rate).to_uint_floor().u128();
            }
        }
        Ok(BandProtocol { sei_per_usd })
    }

    pub fn usd_amount(&self, usei: u128) -> u128 {
        usei
            .checked_mul(self.sei_per_usd)
            .and_then(|v| v.checked_div(BandProtocol::ONE_USD))
            .unwrap()
    }

    pub fn usei_amount(&self, usd: u128) -> u128 {
        usd.checked_mul(BandProtocol::ONE_USD)
            .and_then(|v| v.checked_div(self.sei_per_usd))
            .unwrap()
    }
}
