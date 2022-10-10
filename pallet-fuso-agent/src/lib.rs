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

/// This pallet is under construction.
/// InterChainAccount (a.k. ICA) is an application layer protocol over IBC protocol family,
/// since Fusotao is based on Octopus Network which doesn't implement IBC to interact with mainchain,
/// this pallet only mainteins the controller-chain + controller-addr <-> host-addr bindings to call
/// some functions without sr25519 signatures.
/// In the future, we should migrate the bindings to compatiable with IBC port/connection/routing.
#[frame_support::pallet]
pub mod pallet {
    use codec::{Codec, EncodeLike};
    use frame_support::{pallet_prelude::*, traits::Get, weights::GetDispatchInfo};
    use frame_system::{ensure_signed, pallet_prelude::*};
    use sp_runtime::{
        traits::{CheckedAdd, Dispatchable, TrailingZeroInput, Zero},
        DispatchError, DispatchResult,
    };
	use pallet_chainbridge_support::traits::Agent;
    use sp_std::{boxed::Box, vec::Vec};

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type Controller: Parameter + Member + Codec;

        type Function: Parameter
            + Dispatchable<Origin = Self::Origin>
            + EncodeLike
            + GetDispatchInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn agents)]
    pub type Agents<T: Config> =
        StorageMap<_, Blake2_128Concat, T::Controller, T::AccountId, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        ControllerTxCompleted,
    }

    #[pallet::error]
    pub enum Error<T> {
        RegisterAgentFailed,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::pallet]
    #[pallet::without_storage_info]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    /// IBC reference
    impl<T: Config> Agent<T::AccountId> for Pallet<T> {
        type Message = T::Function;
        type Origin = T::Controller;

        /// bind the origin to an appchain account without private key
        /// function RegisterInterchainAccount(counterpartyPortId: Identifier, connectionID: Identifier) returns (nil)
        fn register_agent(origin: Self::Origin) -> Result<T::AccountId, DispatchError> {
            let deterministic =
                (b"fusotao#", origin.clone()).using_encoded(sp_io::hashing::blake2_256);
            let host_addr: T::AccountId =
                Decode::decode(&mut TrailingZeroInput::new(deterministic.as_ref()))
                    .map_err(|_| Error::<T>::RegisterAgentFailed)?;
            // FIXME migration friendly
            Agents::<T>::insert(origin, host_addr.clone());
            Ok(host_addr)
        }

        /// function AuthenticateTx(msgs []Any, connectionId string, portId string) returns (error)
        fn authenticate_tx(origin: Self::Origin, msg: Self::Message) -> Result<(), DispatchError> {
            Ok(())
        }

        /// function ExecuteTx(sourcePort: Identifier, channel Channel, msgs []Any) returns (resultString, error)
        fn execute_tx(origin: Self::Origin, msg: Self::Message) -> DispatchResult {
            let agent = match Self::agents(origin.clone()) {
                Some(agent) => agent,
                None => Self::register_agent(origin)?,
            };
            msg.dispatch(frame_system::RawOrigin::Signed(agent).into())
                .map(|_| {
                    Self::deposit_event(Event::<T>::ControllerTxCompleted);
                })
                .map_err(|e| e.error)
        }
    }

  /*  pub trait Agent<AccountId> {
        type Origin;
        type Message;

        /// bind the origin to an appchain account without private key
        /// function RegisterInterchainAccount(counterpartyPortId: Identifier, connectionID: Identifier) returns (nil)
        fn register_agent(origin: Self::Origin) -> Result<AccountId, DispatchError>;

        /// function AuthenticateTx(msgs []Any, connectionId string, portId string) returns (error)
        fn authenticate_tx(origin: Self::Origin, msg: Self::Message) -> Result<(), DispatchError>;

        /// function ExecuteTx(sourcePort: Identifier, channel Channel, msgs []Any) returns (resultString, error)
        fn execute_tx(origin: Self::Origin, msg: Self::Message) -> DispatchResult;
    }*/
}
