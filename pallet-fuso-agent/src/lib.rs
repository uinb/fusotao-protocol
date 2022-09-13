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

#[cfg(test)]
pub mod mock;

/// InterChainAccount (a.k. ICA) is an application layer protocol over IBC protocol family,
/// since Fusotao is based on Octopus Network which doesn't implement IBC to interact with mainchain,
/// this pallet only mainteins the controller-chain + controller-addr <-> host-addr bindings to call
/// some functions without sr25519 signatures.
/// In the future, we should migrate the bindings to compatiable with IBC port/connection/routing.
#[frame_support::pallet]
pub mod pallet {
    use codec::EncodeLike;
    use frame_support::{pallet_prelude::*, traits::Get, transactional, weights::GetDispatchInfo};
    use frame_system::{ensure_signed, pallet_prelude::*};
    use sp_runtime::{
        traits::{CheckedAdd, Dispatchable, TrailingZeroInput, Zero},
        DispatchError, DispatchResult, Perquintill,
    };

    pub type ControllerChain = sp_std::vec::Vec<u8>;
    pub type ControllerAddr = sp_std::vec::Vec<u8>;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type Function: Parameter
            + Dispatchable<Origin = Self::Origin>
            + EncodeLike
            + GetDispatchInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn agents)]
    pub type Agents<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        ControllerChain,
        Blake2_128Concat,
        ControllerAddr,
        T::AccountId,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {
        CouldntRegisterAgent,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::pallet]
    #[pallet::without_storage_info]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    /// IBC reference
    impl<T: Config> Pallet<T> {
        /// function RegisterInterchainAccount(counterpartyPortId: Identifier, connectionID: Identifier) returns (nil)
        pub fn register_agent(
            controller_chain: ControllerChain,
            controller_addr: ControllerAddr,
        ) -> Result<T::AccountId, DispatchError> {
            let deterministic = (
                b"fuso/agents",
                controller_chain.clone(),
                controller_addr.clone(),
            )
                .using_encoded(sp_io::hashing::blake2_256);
            let host_addr: T::AccountId =
                Decode::decode(&mut TrailingZeroInput::new(deterministic.as_ref()))
                    .map_err(|_| Error::<T>::CouldntRegisterAgent)?;
            Agents::<T>::insert(controller_chain, controller_addr, host_addr.clone());
            Ok(host_addr)
        }

        /// function AuthenticateTx(msgs []Any, connectionId string, portId string) returns (error)
        pub fn authenticate_tx(
            controller_chain: ControllerChain,
            controller_addr: ControllerAddr,
        ) -> DispatchResult {
            Ok(())
        }

        /// function ExecuteTx(sourcePort: Identifier, channel Channel, msgs []Any) returns (resultString, error)
        /// the octopus has already validated the transaction happend on mainchain
        pub fn execute_tx(
            controller_chain: ControllerChain,
            controller_addr: ControllerAddr,
            call: T::Function,
        ) -> DispatchResult {
            let agent = match Self::agents(controller_chain.clone(), controller_addr.clone()) {
                Some(agent) => agent,
                None => Self::register_agent(controller_chain, controller_addr)?,
            };
            // TODO
            call.dispatch(frame_system::RawOrigin::Signed(agent).into())
                .map(|_| ())
                .map_err(|e| e.error)
        }
    }
}
