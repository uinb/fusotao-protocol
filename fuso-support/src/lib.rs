// Copyright 2021 UINB Technologies Pte. Ltd.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]
pub extern crate alloc;

pub use alloc::collections;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub mod external_chain;
pub mod traits;
use crate::chainbridge::ResourceId;
pub use external_chain::*;

pub mod constants {
    pub const RESERVE_FOR_STAKING: u8 = 0u8;
    pub const RESERVE_FOR_AUTHORIZING: u8 = 1u8;
    pub const RESERVE_FOR_AUTHORIZING_STASH: u8 = 2u8;
    pub const RESERVE_FOR_PENDING_UNSTAKE: u8 = 3u8;
    pub const DOMINATOR_REGISTERED: u8 = 0u8;
    pub const DOMINATOR_INACTIVE: u8 = 1u8;
    pub const DOMINATOR_ACTIVE: u8 = 2u8;
    pub const DOMINATOR_EVICTED: u8 = 3u8;
    pub const STANDARD_DECIMALS: u8 = 18;
    pub const MAX_DECIMALS: u8 = 24;
}

pub fn derive_resource_id(chain: u8, id: &[u8]) -> Result<ResourceId, String> {
    let mut r_id: ResourceId = [0; 32];
    let id_len = id.len();
    r_id[31] = chain; // last byte is chain id
    if id_len > 30 {
        return Err("id is too long".to_string());
    }
    for i in 0..id_len {
        r_id[30 - i] = id[id_len - 1 - i]; // Ensure left padding for eth compatibilit
    }
    r_id[0] = id_len as u8;
    Ok(r_id)
}

pub fn decode_resource_id(r_id: ResourceId) -> (u8, Vec<u8>) {
    let chainid = r_id[31];
    let id_len = r_id[0];
    let start = (31 - id_len) as usize;
    let v: &[u8] = &r_id[start..31];
    (chainid, v.to_vec())
}
