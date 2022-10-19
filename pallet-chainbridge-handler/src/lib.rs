#![recursion_limit = "128"]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)]

extern crate core;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod fungible;
pub mod token;

use codec::{Codec, EncodeLike};
// use frame_support::{ dispatch::DispatchResult, ensure};
use frame_support::{
    pallet_prelude::*,
    sp_runtime::traits::AtLeast32BitUnsigned,
    sp_std::fmt::Debug,
    traits::{
        fungibles::Mutate, tokens::Balance as AssetBalance, Currency, EnsureOrigin,
        ExistenceRequirement, ExistenceRequirement::AllowDeath, Get, StorageVersion,
    },
    weights::GetDispatchInfo,
};
use frame_system::{ensure_signed, pallet_prelude::*};
use fuso_support::{
    chainbridge::*,
    traits::{Agent, AssetIdResourceIdProvider},
};
use pallet_chainbridge as bridge;
use sp_core::U256;
use sp_runtime::traits::{Dispatchable, SaturatedConversion, TrailingZeroInput};
use sp_std::{convert::From, prelude::*};

type Depositer = EthAddress;
type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub use pallet::*;

/// The current storage version.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    // use log::{info, log};

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    #[pallet::without_storage_info]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + bridge::Config {
        /// The overarching event type.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Specifies the origin check provided by the bridge for calls that can only be called by
        /// the bridge pallet
        type BridgeOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;

        /// The currency mechanism.
        type Currency: Currency<Self::AccountId>;

        /// Identifier for the class of asset.
        type AssetId: Member
            + Parameter
            + AtLeast32BitUnsigned
            + Codec
            + Copy
            + Debug
            + Default
            + MaybeSerializeDeserialize;

        /// The units in which we record balances.
        type AssetBalance: AssetBalance + From<u128> + Into<u128>;

        /// dispatchable call
        type Call: Parameter + Dispatchable<Origin = Self::Origin> + EncodeLike + GetDispatchInfo;

        /// Expose customizable associated type of asset transfer, lock and unlock
        type Fungibles: Mutate<
            Self::AccountId,
            AssetId = Self::AssetId,
            Balance = Self::AssetBalance,
        >;

        /// Map of cross-chain asset ID & name
        type AssetIdByName: AssetIdResourceIdProvider<Self::AssetId>;

        /// Max native token value
        type NativeTokenMaxValue: Get<BalanceOf<Self>>;

        type NativeResourceId: Get<ResourceId>;

        type DonorAccount: Get<Self::AccountId>;

        type DonationForAgent: Get<BalanceOf<Self>>;
    }

    #[pallet::storage]
    #[pallet::getter(fn native_check)]
    pub type NativeCheck<T> = StorageValue<_, bool, ValueQuery>;

    /// store generic hash
    #[pallet::storage]
    #[pallet::getter(fn assets_stored)]
    pub type AssetsStored<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, bool>;

    #[pallet::storage]
    #[pallet::getter(fn agents)]
    pub type Agents<T: Config> = StorageMap<_, Blake2_128Concat, Depositer, T::AccountId>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// deposit assets
        Deposit {
            sender: T::AccountId,
            recipient: T::AccountId,
            resource_id: ResourceId,
            amount: BalanceOf<T>,
        },
        /// Withdraw assets
        Withdraw {
            sender: T::AccountId,
            recipient: Vec<u8>,
            resource_id: ResourceId,
            amount: BalanceOf<T>,
        },
        Remark(T::Hash),
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidTransfer,
        InvalidTokenId,
        InValidResourceId,
        WrongAssetId,
        InvalidTokenName,
        OverTransferLimit,
        AssetAlreadyExists,
        InvalidCallMessage,
        RegisterAgentFailed,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(195_000_0000)]
        pub fn native_limit(origin: OriginFor<T>, value: bool) -> DispatchResult {
            ensure_root(origin)?;
            <NativeCheck<T>>::put(value);
            Ok(())
        }

        /// Transfers some amount of the native token to some recipient on a (whitelisted)
        /// destination chain.
        #[pallet::weight(195_000_0000)]
        pub fn transfer_out(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
            r_id: ResourceId,
            recipient: Vec<u8>,
            dest_id: ChainId,
        ) -> DispatchResult {
            let source = ensure_signed(origin)?;
            ensure!(
                <bridge::Pallet<T>>::chain_whitelisted(dest_id),
                <Error<T>>::InvalidTransfer
            );
            // TODO
            // check recipient address is verify
            match r_id == T::NativeResourceId::get() {
                true => Self::do_lock(source, amount, r_id, recipient, dest_id)?,
                false => Self::do_burn_assets(source, amount, r_id, recipient, dest_id)?,
            }
            Ok(())
        }

        /// Executes a simple currency transfer using the bridge account as the source
        /// Triggered by a initial transfer on source chain, executed by relayer when proposal was
        /// resolved. this function by bridge triggered transfer
        #[pallet::weight(195_000_0000)]
        pub fn transfer_in(
            origin: OriginFor<T>,
            to: T::AccountId,
            amount: BalanceOf<T>,
            r_id: ResourceId,
        ) -> DispatchResult {
            let source = T::BridgeOrigin::ensure_origin(origin)?;
            match r_id == T::NativeResourceId::get() {
                true => Self::do_unlock(source, to, amount.into())?,
                false => Self::do_mint_assets(to, amount, r_id)?,
            }
            Ok(())
        }

        /// This can be called by the bridge to demonstrate an arbitrary call from a proposal.
        #[pallet::weight(195_000_0000)]
        pub fn remark(
            origin: OriginFor<T>,
            message: Vec<u8>,
            depositer: Depositer,
            _r_id: ResourceId,
        ) -> DispatchResult {
            T::BridgeOrigin::ensure_origin(origin)?;
            let c = <T as Config>::Call::decode(&mut &message[..])
                .map_err(|_| <Error<T>>::InvalidCallMessage)?;
            let controller = (b"ETH".to_vec(), depositer);
            Self::execute_tx(controller, c)?;
            Ok(())
        }
    }
}

/// IBC reference
impl<T: Config> Agent<T::AccountId> for Pallet<T> {
    type Message = <T as pallet::Config>::Call;
    type Origin = (Vec<u8>, Depositer);

    /// bind the origin to an appchain account without private key
    /// function RegisterInterchainAccount(counterpartyPortId: Identifier, connectionID: Identifier) returns (nil)
    fn register_agent(origin: Self::Origin) -> Result<T::AccountId, DispatchError> {
        let hash = (b"-*-#fusotao#-*-", origin.clone()).using_encoded(sp_io::hashing::blake2_256);
        let host_addr = Decode::decode(&mut TrailingZeroInput::new(hash.as_ref()))
            .map_err(|_| Error::<T>::RegisterAgentFailed)?;
        if !Agents::<T>::contains_key(&origin.1) {
            T::Currency::transfer(
                &T::DonorAccount::get(),
                &host_addr,
                T::DonationForAgent::get(),
                ExistenceRequirement::KeepAlive,
            )?;
            Agents::<T>::insert(origin.1.clone(), host_addr.clone());
        }
        Ok(host_addr)
    }

    /// function AuthenticateTx(msgs []Any, connectionId string, portId string) returns (error)
    fn authenticate_tx(_origin: Self::Origin, _msg: Self::Message) -> Result<(), DispatchError> {
        Ok(())
    }

    /// function ExecuteTx(sourcePort: Identifier, channel Channel, msgs []Any) returns (resultString, error)
    fn execute_tx(origin: Self::Origin, msg: Self::Message) -> DispatchResult {
        let agent = Self::register_agent(origin)?;
        msg.dispatch(frame_system::RawOrigin::Signed(agent).into())
            .map(|_| ().into())
            .map_err(|e| e.error)
    }
}
