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

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::DispatchResultWithPostInfo;
    use frame_support::{pallet_prelude::*, traits::Get, transactional};
    use frame_system::ensure_signed;
    use frame_system::pallet_prelude::*;
    use fuso_support::traits::{Rewarding, Token};
    use sp_runtime::{
        traits::{CheckedAdd, Zero},
        DispatchResult, Perquintill,
    };

    pub type Balance<T> =
        <<T as Config>::Asset as Token<<T as frame_system::Config>::AccountId>>::Balance;

    pub type Era<T> = <T as frame_system::Config>::BlockNumber;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type Asset: Token<Self::AccountId>;

        #[pallet::constant]
        type EraDuration: Get<Self::BlockNumber>;

        #[pallet::constant]
        type RewardsPerEra: Get<Balance<Self>>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {
        Overflow,
        DivideByZero,
        RewardNotFound,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, Default)]
    pub struct Reward<Balance, Era> {
        pub confirmed: Balance,
        pub pending_vol: Balance,
        pub last_modify: Era,
    }

    #[pallet::storage]
    #[pallet::getter(fn rewards)]
    pub type Rewards<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Reward<Balance<T>, Era<T>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn volumes)]
    pub type Volumes<T: Config> = StorageMap<_, Blake2_128Concat, Era<T>, Balance<T>, ValueQuery>;

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T>
    where
        Balance<T>: Into<u128> + From<u128>,
    {
        #[pallet::weight(10000000)]
        pub fn take_rewarding(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::claim_rewarding(&who)?;
            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T>
    where
        Balance<T>: Into<u128> + From<u128>,
    {
        fn save_rewarding(
            at: T::BlockNumber,
            amount: Balance<T>,
            account: &T::AccountId,
        ) -> DispatchResult {
            Ok(Rewards::<T>::try_mutate(account, |r| -> DispatchResult {
                if at == r.last_modify {
                    Ok(r.pending_vol = r
                        .pending_vol
                        .checked_add(&amount)
                        .ok_or(Error::<T>::Overflow)?)
                } else {
                    if r.pending_vol == Zero::zero() {
                        r.pending_vol = amount;
                        r.last_modify = at;
                    } else {
                        let pending_vol: u128 = r.pending_vol.into();
                        let total_vol: u128 = Volumes::<T>::get(at).into();
                        ensure!(total_vol > 0, Error::<T>::DivideByZero);
                        let p: Perquintill = Perquintill::from_rational(pending_vol, total_vol);
                        let era_reward: u128 = T::RewardsPerEra::get().into();
                        let a = p * era_reward;
                        r.confirmed = r
                            .confirmed
                            .checked_add(&a.into())
                            .ok_or(Error::<T>::Overflow)?;
                        r.pending_vol = amount;
                        r.last_modify = at;
                    }
                    Ok(())
                }
            })?)
        }
    }

    impl<T: Config> Rewarding<T::AccountId, Balance<T>, T::BlockNumber> for Pallet<T>
    where
        Balance<T>: Into<u128> + From<u128>,
    {
        type Volume = Balance<T>;

        fn era_duration() -> T::BlockNumber {
            T::EraDuration::get()
        }

        fn total_volume(at: T::BlockNumber) -> Self::Volume {
            Self::volumes(at - at % Self::era_duration())
        }

        fn acked_reward(who: &T::AccountId) -> Balance<T> {
            Self::rewards(who).confirmed
        }

        #[transactional]
        fn save_trading(
            taker: &T::AccountId,
            maker: &T::AccountId,
            amount: Self::Volume,
            at: T::BlockNumber,
        ) -> DispatchResult
        where
            Balance<T>: Into<u128> + From<u128>,
        {
            if amount == Zero::zero() {
                return Ok(());
            }
            let at = at - at % Self::era_duration();
            Volumes::<T>::try_mutate(&at, |v| v.checked_add(&amount).ok_or(Error::<T>::Overflow))?;
            Self::save_rewarding(at, amount, maker)?;
            Self::save_rewarding(at, amount, taker)?;
            Ok(())
        }

        #[transactional]
        fn claim_rewarding(
            who: &T::AccountId,
        ) -> sp_std::result::Result<Balance<T>, DispatchError> {
            Rewards::<T>::try_mutate_exists(who, |r| -> DispatchResult {
                ensure!(r.is_some(), Error::<T>::RewardNotFound);
                let mut reward: Reward<Balance<T>, Era<T>> = r.take().unwrap();
                let confirmed = reward.confirmed;
                reward.confirmed = 0.into();
                r.replace(reward);
                T::Asset::try_mutate_account(&0u32.into(), &who, |b| Ok(b.0 += confirmed))?;
                Ok(())
            })?;
            Ok(Zero::zero())
        }
    }
}
