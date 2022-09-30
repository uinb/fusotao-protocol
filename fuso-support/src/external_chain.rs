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
        match *self {
            XToken::NEP141(_, _, _, stable, _) => stable,
            XToken::ERC20(_, _, _, stable, _) => stable,
            XToken::BEP20(_, _, _, stable, _) => stable,
            XToken::FND10(_, _) => false,
        }
    }
}
