#![cfg_attr(not(feature = "std"), no_std)]
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

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		traits::{Get, NamedReservableCurrency},
		weights::Weight,
	};
	use frame_support::{pallet_prelude::*};
	use frame_system::pallet_prelude::*;
	use fuso_support::reserve_identifier_prefix;
	use sp_runtime::traits::{Saturating, Zero};
	pub type BalanceOf<T> = <T as pallet_balances::Config>::Balance;
    pub type IdentifierOf<T> = <T as pallet_balances::Config>::ReserveIdentifier;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_balances::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type MaxBlock: Get<Self::BlockNumber>;

        type MinBlock: Get<Self::BlockNumber>;
    }

   // pub const RESERVABLE_IDENTIFIER = T::AccountId::default();

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub fund: Vec<(T::AccountId, (T::BlockNumber, T::BlockNumber, u16, BalanceOf<T>))>,
        //TODO
        pub fund_total: Vec<(T::AccountId, BalanceOf<T>)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                fund: Vec::new(),
                fund_total: Vec::new(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T>
    where
        IdentifierOf<T>: From<(u8,T::AccountId)>,
    {
        fn build(&self) {
            for (account, balance) in &self.fund {
                Foundation::<T>::insert(account, balance);
            }
            for (account, balance) in &self.fund_total {
                <pallet_balances::Pallet<T>>::reserve_named(
                    &(reserve_identifier_prefix::FOUNDATION, T::AccountId::default()).into(),
                    &account,
                    *balance,
                );
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        PreLockedFundUnlocked(T::AccountId, BalanceOf<T>),
        UnlockedFundAllBalance(T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {}

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
    where
        IdentifierOf<T>: From<(u8, T::AccountId)>,
    {
        fn on_initialize(now: T::BlockNumber) -> Weight {
			Self::initialize(now)
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn foundation)]
    pub type Foundation<T: Config> =
    StorageMap<_, Blake2_128Concat, T::AccountId, (T::BlockNumber, T::BlockNumber, u16, BalanceOf<T>)>;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    impl<T: Config> Pallet<T>
    where
        IdentifierOf<T>: From<(u8,T::AccountId)>,
    {
        fn initialize(now: T::BlockNumber) -> Weight {
            let max_block: T::BlockNumber = T::MaxBlock::get();
            let min_block: T::BlockNumber = T::MinBlock::get();
			if now < min_block || now > max_block {
				0
			}else {
				Self::unlock_fund(now)
			}

        }

        fn unlock_fund(now: T::BlockNumber) -> Weight {
            let mut weight: Weight = 100_000_000u64;
            for item in Foundation::<T>::iter() {
				weight = weight.saturating_add(T::DbWeight::get().reads(1 as Weight));
				let account = item.0;
				let balance: (T::BlockNumber,T::BlockNumber, u16, BalanceOf<T>) = item.1;
				if now.saturating_sub(balance.0) % balance.1 == Zero::zero() {
					if balance.2 > 0 {
						<pallet_balances::Pallet<T>>::unreserve_named(&(reserve_identifier_prefix::FOUNDATION, T::AccountId::default()).into(), &account, balance.3);
						Self::deposit_event(Event::PreLockedFundUnlocked(account.clone(), balance.3));
						let b = (balance.0, balance.1, balance.2 - 1, balance.3);
						Foundation::<T>::insert(account, b);
						weight = weight.saturating_add(T::DbWeight::get().writes(1 as Weight));
					}
				}
            }
            weight
        }
    }
}
