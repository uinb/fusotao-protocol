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

pub use pallet::*;
#[cfg(test)]
pub mod mock;
#[cfg(test)]
pub mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        pallet_prelude::*,
        // storage::types::OptionQuery,
        traits::{Get, ReservableCurrency},
        weights::Weight,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::Zero;

    pub type BalanceOf<T> = <T as pallet_balances::Config>::Balance;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_balances::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type Duration: Get<Self::BlockNumber>;
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub fund: Vec<(
            T::AccountId,
            // delay duration, interval_duration, times, amount for each time, first unlock amount
            u32,
            u32,
            u32,
            BalanceOf<T>,
            BalanceOf<T>,
        )>,
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, Default, TypeInfo, Debug)]
    pub struct FoundationData<Balance> {
        pub delay_durations: u32,
        pub interval_durations: u32,
        pub times: u32,
        pub amount: Balance,
        pub first_amount: Balance,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self { fund: Vec::new() }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for data in &self.fund {
                Foundation::<T>::insert(
                    data.0.clone(),
                    FoundationData {
                        delay_durations: data.1,
                        interval_durations: data.2,
                        times: data.3,
                        amount: data.4,
                        first_amount: data.5,
                    },
                );
                pallet_balances::Pallet::<T>::mutate_account(&data.0, |account_data| {
                    account_data.reserved = data.4 * data.3.into() + data.5;
                })
                .unwrap();
            }
        }
    }

    #[cfg(feature = "std")]
    impl<T: Config> GenesisConfig<T> {
        /// Direct implementation of `GenesisBuild::build_storage`.
        ///
        /// Kept in order not to break dependency.
        pub fn build_storage(&self) -> Result<sp_runtime::Storage, String> {
            <Self as GenesisBuild<T>>::build_storage(self)
        }

        /// Direct implementation of `GenesisBuild::assimilate_storage`.
        ///
        /// Kept in order not to break dependency.
        pub fn assimilate_storage(&self, storage: &mut sp_runtime::Storage) -> Result<(), String> {
            <Self as GenesisBuild<T>>::assimilate_storage(self, storage)
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
        T::BlockNumber: Into<u32>,
    {
        fn on_initialize(now: T::BlockNumber) -> Weight {
            Self::initialize(now)
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn foundation)]
    pub type Foundation<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, FoundationData<BalanceOf<T>>, OptionQuery>;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    impl<T: Config> Pallet<T>
    where
        T::BlockNumber: Into<u32>,
    {
        fn initialize(now: T::BlockNumber) -> Weight {
            let duration: T::BlockNumber = T::Duration::get();
            if now % duration != Zero::zero() {
                0
            } else {
                Self::unlock_fund(now.into() / duration.into())
            }
        }

        fn unlock_fund(now: u32) -> Weight {
            let mut weight: Weight = 100_000_000u64;
            for item in Foundation::<T>::iter() {
                weight = weight.saturating_add(T::DbWeight::get().reads(1 as Weight));
                let account = item.0;
                let mut balance: FoundationData<BalanceOf<T>> = item.1;

                if (now > balance.delay_durations)
                    && (now.saturating_sub(balance.delay_durations) % balance.interval_durations
                        == 0u32)
                {
                    <pallet_balances::Pallet<T>>::unreserve(&account, balance.amount);
                    Self::deposit_event(Event::PreLockedFundUnlocked(
                        account.clone(),
                        balance.amount,
                    ));
                    balance.times = balance.times - 1;
                    if balance.times == 0 {
                        Foundation::<T>::remove(account);
                    } else {
                        Foundation::<T>::insert(account, balance);
                    }
                    weight = weight.saturating_add(T::DbWeight::get().writes(1 as Weight));
                } else if (now == balance.delay_durations)
                    && (now.saturating_sub(balance.delay_durations) % balance.interval_durations
                        == 0u32)
                {
                    //initial unlock
                    <pallet_balances::Pallet<T>>::unreserve(&account, balance.first_amount);
                    Self::deposit_event(Event::PreLockedFundUnlocked(
                        account.clone(),
                        balance.first_amount,
                    ));
                    weight = weight.saturating_add(T::DbWeight::get().writes(1 as Weight));
                }
            }
            weight
        }
    }
}
