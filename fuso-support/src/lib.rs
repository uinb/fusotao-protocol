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

pub mod external_chain;
pub mod traits;

pub mod constants {
    pub const RESERVE_FOR_STAKING: u8 = 0u8;
    pub const RESERVE_FOR_AUTHORIZING: u8 = 1u8;
	pub const RESERVE_FOR_AUTHORIZING_STASH: u8 = 2u8;
    pub const DOMINATOR_INACTIVE: u8 = 0u8;
    pub const DOMINATOR_ACTIVE: u8 = 1u8;
    pub const DOMINATOR_EVICTED: u8 = 2u8;
}
