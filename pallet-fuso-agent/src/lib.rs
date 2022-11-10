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

extern crate alloc;

use codec::{Codec, Encode};
use fuso_support::ExternalSignWrapper;
use sp_runtime::traits::Dispatchable;
use sp_std::{boxed::Box, vec::Vec};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EthInstance;

pub struct EthPersonalSignWrapper;

impl<T: frame_system::Config> ExternalSignWrapper<T> for EthPersonalSignWrapper {
    fn extend_payload<W: Dispatchable<Origin = T::Origin> + Codec>(
        nonce: T::Index,
        tx: Box<W>,
    ) -> Vec<u8> {
        let encoded_payload = (nonce, tx).using_encoded(|v| v.to_vec());
        [
            &[0x19u8][..],
            &alloc::format!("Ethereum Signed Message:\n{}", encoded_payload.len()).as_bytes()[..],
            &encoded_payload[..],
        ]
        .concat()
    }
}

#[frame_support::pallet]
pub mod pallet {
    use codec::EncodeLike;
    use frame_support::{
        dispatch::Dispatchable,
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, Get, WithdrawReasons},
        weights::{GetDispatchInfo, Weight, WeightToFeePolynomial},
    };
    use frame_system::pallet_prelude::*;
    pub use fuso_support::external_chain::{ChainId, ExternalSignWrapper};
    use sp_core::{ecdsa, ed25519};
    use sp_runtime::traits::{Saturating, TrailingZeroInput, Zero};
    use sp_std::boxed::Box;

    pub type BalanceOf<T, I = ()> =
        <<T as Config<I>>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
    pub enum ExternalVerifiable<Index, Call> {
        Ed25519 {
            public: ed25519::Public,
            tx: Box<Call>,
            nonce: Index,
            signature: ed25519::Signature,
        },
        Ecdsa {
            tx: Box<Call>,
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

        type WeightToFee: WeightToFeePolynomial<Balance = BalanceOf<Self, I>>;

        type TransactionByteFee: Get<BalanceOf<Self, I>>;

        type Currency: Currency<Self::AccountId>;

        type ExternalSignWrapper: ExternalSignWrapper<Self>;

        type ExternalChainId: Get<ChainId>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        ExternalTransactionExecuted(T::AccountId),
    }

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
            let account = Pallet::<T, I>::extract(&tx)?;
            match tx {
                ExternalVerifiable::Ed25519 { .. } => Err(Error::<T, I>::InvalidSignature.into()),
                ExternalVerifiable::Ecdsa { tx, .. } => tx
                    .dispatch(frame_system::RawOrigin::Signed(account.clone()).into())
                    .map(|_| {
                        Self::deposit_event(Event::<T, I>::ExternalTransactionExecuted(account));
                        ().into()
                    })
                    .map_err(|e| e.error.into()),
            }
        }
    }

    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        pub fn extract(
            sig: &ExternalVerifiable<T::Index, T::Transaction>,
        ) -> Result<T::AccountId, DispatchError> {
            match sig {
                ExternalVerifiable::Ed25519 { .. } => Err(Error::<T, I>::InvalidSignature.into()),
                ExternalVerifiable::Ecdsa {
                    ref tx,
                    ref nonce,
                    signature,
                } => {
                    let msg = T::ExternalSignWrapper::extend_payload(*nonce, tx.clone());
                    let pubkey = signature
                        .recover(&msg)
                        .map(|v| v.0.to_vec())
                        .ok_or(Error::<T, I>::InvalidSignature)?;
                    let address = sp_io::hashing::keccak_256(&pubkey)[12..].to_vec();
                    let h = (b"-*-#fusotao#-*-", T::ExternalChainId::get(), address)
                        .using_encoded(sp_io::hashing::blake2_256);
                    Decode::decode(&mut TrailingZeroInput::new(h.as_ref()))
                        .map_err(|_| Error::<T, I>::InvalidSignature.into())
                }
            }
        }

        fn withdraw_fee(
            who: &T::AccountId,
            fee: BalanceOf<T, I>,
        ) -> Result<(), TransactionValidityError> {
            if fee.is_zero() {
                return Ok(());
            }
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

        fn compute_fee(len: u32, weight: Weight, class: DispatchClass) -> BalanceOf<T, I> {
            let len = <BalanceOf<T, I>>::from(len);
            let per_byte = T::TransactionByteFee::get();
            let len_fee = per_byte.saturating_mul(len);
            let weight_fee = Self::weight_to_fee(weight);
            let base_fee = Self::weight_to_fee(T::BlockWeights::get().get(class).base_extrinsic);
            base_fee.saturating_add(len_fee).saturating_add(weight_fee)
        }

        fn weight_to_fee(weight: Weight) -> BalanceOf<T, I> {
            let capped_weight = weight.min(T::BlockWeights::get().max_block);
            T::WeightToFee::calc(&capped_weight)
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config<I>, I: 'static> ValidateUnsigned for Pallet<T, I> {
        type Call = Call<T, I>;

        /// Validate unsigned call to this module.
        /// TODO make it compatiable with Ed25519 signature
        fn validate_unsigned(_: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if let Call::submit_external_tx { ref tx } = call {
                let account =
                    Pallet::<T, I>::extract(tx).map_err(|_| InvalidTransaction::BadProof)?;
                frame_system::Pallet::<T>::inc_account_nonce(account.clone());
                let index = frame_system::Pallet::<T>::account_nonce(&account);
                let (nonce, call) = match tx {
                    ExternalVerifiable::Ed25519 {
                        public: _,
                        tx,
                        nonce,
                        signature: _,
                    } => (*nonce, tx),
                    ExternalVerifiable::Ecdsa {
                        tx,
                        nonce,
                        signature: _,
                    } => (*nonce, tx),
                };
                ensure!(index == nonce, InvalidTransaction::BadProof);
                let info = call.get_dispatch_info();
                let len = tx
                    .encoded_size()
                    .try_into()
                    .map_err(|_| InvalidTransaction::ExhaustsResources)?;
                ensure!(
                    len < *T::BlockLength::get().max.get(DispatchClass::Normal),
                    InvalidTransaction::ExhaustsResources
                );
                ensure!(
                    info.weight < T::BlockWeights::get().max_block.saturating_div(5),
                    InvalidTransaction::ExhaustsResources
                );
                let fee = Pallet::<T, I>::compute_fee(len, info.weight, info.class);
                let _ = Pallet::<T, I>::withdraw_fee(&account, fee)?;
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
