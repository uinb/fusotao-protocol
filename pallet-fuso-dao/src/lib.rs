// Copyright 2022 UINB Technologies Pte. Ltd.

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

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use codec::{Codec, EncodeLike};
    use frame_support::{pallet_prelude::*, traits::Get, weights::GetDispatchInfo};
    use frame_system::{ensure_signed, pallet_prelude::*};
    use fuso_support::{
        constants::*,
        traits::{ReservableToken, Rewarding, Token},
    };
    use sp_runtime::{
        traits::{CheckedAdd, Dispatchable, TrailingZeroInput, Zero},
        DispatchError, DispatchResult, Percent,
    };
    use sp_std::{boxed::Box, vec::Vec};

    pub type TokenId<T> =
        <<T as Config>::Asset as Token<<T as frame_system::Config>::AccountId>>::TokenId;

    pub type Balance<T> =
        <<T as Config>::Asset as Token<<T as frame_system::Config>::AccountId>>::Balance;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type Asset: Token<Self::AccountId>;
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
    pub struct DAO<AccountId, TokenId, Balance> {
        pub name: Vec<u8>,
        pub logo: Vec<u8>,
        pub lang: Vec<u8>,
        pub treasury: AccountId,
        pub originator: AccountId,
        pub max_members: u32,
        pub governance_token: TokenId,
        pub rule: DAORule,
        pub entry_threshold: Balance,
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
    pub enum DAORule {
        Unanimity,
        Majority(Percent),
        Delegation(u16),
    }

    #[pallet::storage]
    #[pallet::getter(fn orgs)]
    pub type Orgs<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        DAO<T::AccountId, TokenId<T>, Balance<T>>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn org_members)]
    pub type OrgMembers<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        T::AccountId,
        Balance<T>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {}

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::pallet]
    #[pallet::without_storage_info]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {}
}
