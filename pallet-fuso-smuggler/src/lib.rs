// Copyright 2021-2022 UINB Technologies Pte. Ltd.

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
        traits::{Currency, ExistenceRequirement},
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::StaticLookup;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type EnsureAdmin: EnsureOrigin<Self::Origin>;

        #[pallet::constant]
        type Admin: Get<Self::AccountId>;

        type Currency: Currency<Self::AccountId>;
    }

    #[pallet::storage]
    #[pallet::getter(fn black_list)]
    pub type BlackList<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u128, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {}

    #[pallet::pallet]
    #[pallet::without_storage_info]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(0)]
        pub fn add_account_to_list(
            origin: OriginFor<T>,
            target: <T::Lookup as StaticLookup>::Source,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_admin(origin)?;
            let t = T::Lookup::lookup(target)?;
            BlackList::<T>::insert(t, 0);
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn remove_account_from_list(
            origin: OriginFor<T>,
            target: <T::Lookup as StaticLookup>::Source,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_admin(origin)?;
            let t = T::Lookup::lookup(target)?;
            BlackList::<T>::remove(t);
            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        fn ensure_admin(o: OriginFor<T>) -> DispatchResult {
            T::EnsureAdmin::ensure_origin(o)?;
            Ok(().into())
        }
    }

    impl<T: Config> fuso_support::traits::Smuggler<T::AccountId> for Pallet<T> {
        fn is_wanted(t: &T::AccountId) -> bool {
            BlackList::<T>::contains_key(t)
        }

        fn repatriate_if_wanted(who: &T::AccountId) -> bool {
            if BlackList::<T>::contains_key(who) {
                let v = T::Currency::free_balance(&who);
                let t = T::Admin::get();
                let _ = T::Currency::transfer(&who, &t, v, ExistenceRequirement::AllowDeath);
                true
            } else {
                false
            }
        }
    }
}

pub struct EnsureAdmin<T>(sp_std::marker::PhantomData<T>);

use frame_support::traits::Get;

impl<T: Config> frame_support::pallet_prelude::EnsureOrigin<T::Origin> for EnsureAdmin<T> {
    type Success = T::AccountId;

    fn try_origin(o: T::Origin) -> Result<Self::Success, T::Origin> {
        o.into().and_then(|o| match o {
            frame_system::RawOrigin::Signed(who) if who.clone() == T::Admin::get() => Ok(who),
            r => Err(T::Origin::from(r)),
        })
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn successful_origin() -> T::Origin {
        T::Origin::from(frame_system::RawOrigin::Signed(<Pallet<T>>::account_id()))
    }
}
