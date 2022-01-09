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
    use frame_support::pallet_prelude::*;
    use frame_support::{
        traits::{Get, NamedReservableCurrency},
        weights::Weight,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{Saturating, Zero};

    pub type BalanceOf<T> = <T as pallet_balances::Config>::Balance;
    pub type IdentifierOf<T> = <T as pallet_balances::Config>::ReserveIdentifier;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_balances::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type UnlockDelay: Get<Self::BlockNumber>;

        type UnlockPeriod: Get<Self::BlockNumber>;
    }

    pub const RESERVABLE_IDENTIFIER: [u8; 8] = *b"foundati";

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub fund: Vec<(T::AccountId, (u16, BalanceOf<T>))>,
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
        IdentifierOf<T>: From<[u8; 8]>,
    {
        fn build(&self) {
            for (account, balance) in &self.fund {
                Foundation::<T>::insert(account, balance);
            }
            for (account, balance) in &self.fund_total {
                <pallet_balances::Pallet<T>>::reserve_named(
                    &(RESERVABLE_IDENTIFIER.into()),
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
        IdentifierOf<T>: From<[u8; 8]>,
    {
        fn on_initialize(now: T::BlockNumber) -> Weight {
            if now < T::UnlockDelay::get() {
                0
            } else {
                Self::initialize(now)
            }
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn foundation)]
    pub type Foundation<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (u16, BalanceOf<T>)>;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    impl<T: Config> Pallet<T>
    where
        IdentifierOf<T>: From<[u8; 8]>,
    {
        fn initialize(now: T::BlockNumber) -> Weight {
            let unlock_delay: T::BlockNumber = T::UnlockDelay::get();
            let unlock_period: T::BlockNumber = T::UnlockPeriod::get();
            if (now.saturating_sub(unlock_delay) % unlock_period) == Zero::zero() {
                return Self::unlock_fund();
            }
            0
        }

        fn unlock_fund() -> Weight {
            let mut weight: Weight = 0u64;
            for item in Foundation::<T>::iter() {
                let account = item.0;
                let balance: (u16, BalanceOf<T>) = item.1;
                if balance.0 > 0 {
                    <pallet_balances::Pallet<T>>::unreserve_named(
                        &(RESERVABLE_IDENTIFIER.into()),
                        &account,
                        balance.1,
                    );
                    Self::deposit_event(Event::PreLockedFundUnlocked(account.clone(), balance.1));
                    let b = (balance.0 - 1, balance.1);
                    Foundation::<T>::insert(account, b);
                    weight = weight + 100_000;
                }
            }
            weight
        }
    }
}
