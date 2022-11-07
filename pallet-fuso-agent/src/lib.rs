// Copyright 2021-2022 UINB Technologies Pte. Ltd.

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
        traits::{Currency, Get, ReservableCurrency},
        weights::Weight,
    };
    use frame_system::pallet_prelude::*;
    use sp_application_crypto::RuntimePublic;
    use sp_runtime::traits::Zero;

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type Transaction: Parameter
            + Dispatchable<Origin = Self::Origin>
            + EncodeLike
            + GetDispatchInfo;

        type Signature: Parameter;

        type RuntimePublic: RuntimePublic;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {}

    #[pallet::error]
    pub enum Error<T, I = ()> {}

    #[pallet::pallet]
    #[pallet::without_storage_info]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        #[weight(10000000)]
        pub fn submit_external_signature(
            origin: OriginFor<T>,
            call: Box<T::Transaction>,
            nonce: T::Index,
            _: Signature,
        ) -> DispatchResultWithPostInfo {
            ensure_none!(origin)?;
            // TODO imply account
            frame_system::Pallet::<T>::inc_account_nonce();
            call.dispatch(frame_system::RawOrigin::Signed(agent).into())
                .map(|_| ().into())
                .map_err(|e| e.error)
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config<I>, I: 'static> ValidateUnsigned for Pallet<T, I> {
        type Call = T::Call;

        /// Validate unsigned call to this module.
        fn validate_unsigned(_: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if let Call::submit_external_signature {
                ref payload,
                ref nonce,
                ref signature,
            } = call
            {
                // TODO check nonce
                // TODO check signature
                // TODO imply account
                ValidTransaction::with_tag_prefix("external_signature")
                    // We set base priority to 2**21 and hope it's included before any other
                    // transactions in the pool.
                    .priority(T::UnsignedPriority::get())
                    // This transaction does not require anything else to go before into the pool.
                    // We set the `provides` tag to `account_id`. This makes
                    // sure only one transaction produced by current validator will ever
                    // get to the transaction pool and will end up in the block.
                    // We can still have multiple transactions compete for the same "spot",
                    // and the one with higher priority will replace other one in the pool.
                    .and_provides()
                    // The transaction is only valid for next 5 blocks. After that it's
                    // going to be revalidated by the pool.
                    .longevity(5)
                    // It's fine to propagate that transaction to other peers, which means it can be
                    // created even by nodes that don't produce blocks.
                    // Note that sometimes it's better to keep it for yourself (if you are the block
                    // producer), since for instance in some schemes others may copy your solution and
                    // claim a reward.
                    .propagate(true)
                    .build()
            } else {
                InvalidTransaction::Call.into()
            }
        }
    }
}
