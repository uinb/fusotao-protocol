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
// #[cfg(test)]
// pub mod tests;

use codec::{Decode, Encode};
use frame_support::RuntimeDebug;
use fuso_support::ExternalSignWrapper;
use scale_info::TypeInfo;
use sp_runtime::traits::Dispatchable;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EthInstance;

pub struct EthPersonalSignWrapper;

impl<T: frame_system::Config> ExternalSignWrapper<T> for EthPersonalSignWrapper {
    fn extend_payload(nonce: T::Index, tx: &impl Dispatchable<Origin = T::Origin>) -> Vec<u8> {
        Vec::new()
        // [
        //     &[0x19u8][..],
        //     &format!("Ethereum Signed Message:\n{}", msg.as_ref().len()).as_bytes()[..],
        //     msg.as_ref(),
        // ]
        // .concat()
        // .to_vec()
    }
}

#[frame_support::pallet]
pub mod pallet {
    use codec::EncodeLike;
    use frame_support::{
        dispatch::Dispatchable,
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, Get, WithdrawReasons},
        weights::{DispatchInfo, GetDispatchInfo, Weight},
    };
    use frame_system::pallet_prelude::*;
    pub use fuso_support::external_chain::{ChainId, ExternalSignWrapper};
    use sp_core::{
        crypto::{self, Public},
        ecdsa, ed25519,
        hash::{H256, H512},
        sr25519,
    };
    use sp_runtime::traits::{DispatchInfoOf, TrailingZeroInput, Zero};

    pub type BalanceOf<T, I = ()> =
        <<T as Config<I>>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
    pub enum ExternalVerifiable<Index, Call> {
        Ed25519 {
            public: ed25519::Public,
            tx: Call,
            nonce: Index,
            signature: ed25519::Signature,
        },
        Ecdsa {
            tx: Call,
            nonce: Index,
            signature: ecdsa::Signature,
        },
    }

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        type Event: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::Event>;

        type Transaction: Parameter
            + Dispatchable<Origin = Self::Origin>
            + EncodeLike
            + GetDispatchInfo;

        type TransactionByteFee: Get<BalanceOf<Self, I>>;

        type Currency: Currency<Self::AccountId>;

        type ExternalSignWrapper: ExternalSignWrapper<Self>;

        type ExternalChainId: Get<ChainId>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {}

    #[pallet::error]
    pub enum Error<T, I = ()> {
        InvalidSignature,
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        // no need to set weight here, we would charge gas fee in `pre_dispatch`
        #[pallet::weight(0)]
        pub fn submit_external_tx(
            origin: OriginFor<T>,
            tx: ExternalVerifiable<T::Index, T::Transaction>,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;
            Ok(().into())
            // let msg = T::ExternalSignWrapper::extend_payload(&(nonce, tx.clone()).encode());
            // let addr = signature
            //     .address(&msg)
            //     .ok_or(Error::<T, I>::InvalidSignature)?;
            // let account = T::ExternalAccount::imply(T::ExternalChainId::get(), &addr);
            // // TODO move this to pre_dispatch
            // frame_system::Pallet::<T>::inc_account_nonce(account.clone());
            // tx.dispatch(frame_system::RawOrigin::Signed(account).into())
            //     .map(|_| ().into())
            //     .map_err(|e| e.error.into())
        }
    }

    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        pub fn extract(
            sig: &ExternalVerifiable<T::Index, T::Transaction>,
        ) -> Result<T::AccountId, DispatchError> {
            match sig {
                ExternalVerifiable::Ed25519 { public, .. } => {
                    // let h = (b"-*-#fusotao#-*-", T::ChainId::get(), public.0.to_vec())
                    //     .using_encoded(sp_io::hashing::blake2_256);
                    // let account = Decode::decode(&mut TrailingZeroInput::new(h.as_ref()))
                    //     .map_err(|_| Error::<T>::InvalidSignature)?;
                    Err(Error::<T, I>::InvalidSignature.into())
                }
                ExternalVerifiable::Ecdsa {
                    tx,
                    nonce,
                    signature,
                } => {
                    let msg = T::ExternalSignWrapper::extend_payload(*nonce, tx);
                    let pubkey = signature
                        .recover(&msg)
                        .map(|v| v.0.to_vec())
                        .ok_or(Error::<T, I>::InvalidSignature)?;
                    // TODO pubkey to address
                    let h = (b"-*-#fusotao#-*-", T::ExternalChainId::get(), pubkey)
                        .using_encoded(sp_io::hashing::blake2_256);
                    Decode::decode(&mut TrailingZeroInput::new(h.as_ref()))
                        .map_err(|_| Error::<T, I>::InvalidSignature.into())
                }
            }
        }

        pub fn withdraw_fee(
            who: &T::AccountId,
            _call: &T::Transaction,
            _info: &DispatchInfo,
            fee: BalanceOf<T, I>,
        ) -> Result<(), TransactionValidityError> {
            if fee.is_zero() {
                return Ok(());
            }
            // TODO
            match T::Currency::withdraw(
                who,
                fee,
                WithdrawReasons::TRANSACTION_PAYMENT,
                ExistenceRequirement::KeepAlive,
            ) {
                Ok(_) => Ok(()),
                Err(_) => Err(InvalidTransaction::Payment.into()),
            }
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config<I>, I: 'static> ValidateUnsigned for Pallet<T, I> {
        type Call = Call<T, I>;

        /// Validate unsigned call to this module.
        /// TODO make it compatiable with Ed25519 signature
        fn validate_unsigned(_: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if let Call::submit_external_tx { ref tx } = call {
                // TODO check weight and length
                let account =
                    Pallet::<T, I>::extract(tx).map_err(|_| InvalidTransaction::BadProof)?;
                let index = frame_system::Pallet::<T>::account_nonce(&account);
                let (nonce, call) = match tx {
                    ExternalVerifiable::Ed25519 {
                        public,
                        tx,
                        nonce,
                        signature,
                    } => (*nonce, tx),
                    ExternalVerifiable::Ecdsa {
                        tx,
                        nonce,
                        signature,
                    } => (*nonce, tx),
                };
                ensure!(index == nonce, InvalidTransaction::BadProof);
                let info = call.get_dispatch_info();
                // TODO
                let _ = Pallet::<T, I>::withdraw_fee(&account, call, &info, Zero::zero())?;
                ValidTransaction::with_tag_prefix("FusoAgent")
                    // .priority(call.get_dispatch_info().weight)
                    .and_provides(account)
                    .longevity(10)
                    .propagate(true)
                    .build()
            } else {
                InvalidTransaction::Call.into()
            }
        }
    }
}
