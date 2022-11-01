// Copyright 2021 UINB Technologies Pte. Ltd.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_std::vec::Vec;

#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, TypeInfo)]
pub enum XToken<Balance> {
    // symbol, contract_address, total, stable, decimals
    NEP141(Vec<u8>, Vec<u8>, Balance, bool, u8),
    ERC20(Vec<u8>, Vec<u8>, Balance, bool, u8),
    BEP20(Vec<u8>, Vec<u8>, Balance, bool, u8),
    // symbol, total
    FND10(Vec<u8>, Balance),
}

impl<Balance> XToken<Balance> {
    pub fn is_stable(&self) -> bool {
        match self {
            XToken::NEP141(_, _, _, stable, _)
            | XToken::ERC20(_, _, _, stable, _)
            | XToken::BEP20(_, _, _, stable, _) => *stable,
            XToken::FND10(_, _) => false,
        }
    }

    pub fn symbol(&self) -> Vec<u8> {
        match self {
            XToken::NEP141(symbol, _, _, _, _)
            | XToken::ERC20(symbol, _, _, _, _)
            | XToken::BEP20(symbol, _, _, _, _)
            | XToken::FND10(symbol, _) => symbol.clone(),
        }
    }

    pub fn contract(&self) -> Vec<u8> {
        match self {
            XToken::NEP141(_, contract, _, _, _)
            | XToken::ERC20(_, contract, _, _, _)
            | XToken::BEP20(_, contract, _, _, _) => contract.clone(),
            XToken::FND10(_, _) => Vec::new(),
        }
    }
}

pub mod chainbridge {
    use crate::XToken;
    use alloc::string::ToString;
    use sp_std::vec::Vec;

    pub type ChainId = u8;
    pub type DepositNonce = u64;
    pub type ResourceId = [u8; 32];
    pub type EthAddress = [u8; 20];

    /// [len, ..., dex, chain]
    pub fn derive_resource_id(
        chain: u8,
        dex: u8,
        id: &[u8],
    ) -> Result<ResourceId, alloc::string::String> {
        let mut r_id: ResourceId = [0; 32];
        let id_len = id.len();
        r_id[31] = chain; // last byte is chain id
        r_id[30] = dex;
        if id_len >= 29 {
            return Err("id is too long".to_string());
        }
        for i in 0..id_len {
            r_id[29 - i] = id[id_len - 1 - i]; // Ensure left padding for eth compatibilit
        }
        r_id[0] = id_len as u8;
        Ok(r_id)
    }

    pub fn decode_resource_id(r_id: ResourceId) -> (u8, u8, Vec<u8>) {
        let chainid = r_id[31];
        let dex = r_id[30];
        let id_len = r_id[0];
        let start = (30 - id_len) as usize;
        let v: &[u8] = &r_id[start..30];
        (chainid, dex, v.to_vec())
    }

    pub fn chain_id_of<B>(token_info: &XToken<B>) -> u8 {
        match token_info {
            XToken::NEP141(_, _, _, _, _) => 1u8,
            XToken::ERC20(_, _, _, _, _) => 5u8,
            XToken::BEP20(_, _, _, _, _) => 6u8,
            XToken::FND10(_, _) => 42u8,
        }
    }

    pub trait AssetIdResourceIdProvider<TokenId> {
        type Err;

        fn try_get_asset_id(
            chain_id: ChainId,
            contract_id: impl AsRef<[u8]>,
        ) -> Result<TokenId, Self::Err>;
    }
}
