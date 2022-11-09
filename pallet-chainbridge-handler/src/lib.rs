#![recursion_limit = "128"]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use crate::pallet;
    use codec::EncodeLike;
    use frame_support::{
        pallet_prelude::*,
        traits::{fungibles::Mutate, tokens::BalanceConversion, EnsureOrigin, Get, StorageVersion},
        weights::GetDispatchInfo,
    };
    use frame_system::{ensure_signed, pallet_prelude::*};
    use fuso_support::{
        chainbridge::*,
        traits::{Agent, Token},
    };
    use pallet_chainbridge as bridge;
    use pallet_fuso_verifier as verifier;
    use sp_core::U256;
    use sp_runtime::traits::{Dispatchable, SaturatedConversion, TrailingZeroInput};
    use sp_std::{convert::From, prelude::*};

    type Depositer = EthAddress;

    type AssetId<T> =
        <<T as Config>::Fungibles as Token<<T as frame_system::Config>::AccountId>>::TokenId;

    type BalanceOf<T> =
        <<T as Config>::Fungibles as Token<<T as frame_system::Config>::AccountId>>::Balance;

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    #[pallet::without_storage_info]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + bridge::Config + verifier::Config {
        /// The overarching event type.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Specifies the origin check provided by the bridge for calls that can only be called by
        /// the bridge pallet
        type BridgeOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;

        /// Origin used to administer the pallet
        type AdminOrigin: EnsureOrigin<Self::Origin>;

        type BalanceConversion: BalanceConversion<BalanceOf<Self>, AssetId<Self>, BalanceOf<Self>>;

        /// dispatchable call
        type Redirect: Parameter
            + Dispatchable<Origin = Self::Origin>
            + EncodeLike
            + GetDispatchInfo;

        /// Expose customizable associated type of asset transfer, lock and unlock
        type Fungibles: Mutate<Self::AccountId, AssetId = AssetId<Self>, Balance = BalanceOf<Self>>
            + Token<Self::AccountId>;

        /// Map of cross-chain asset ID & name
        type AssetIdByName: AssetIdResourceIdProvider<AssetId<Self>>;

        /// Max native token value
        type NativeTokenMaxValue: Get<BalanceOf<Self>>;

        type NativeResourceId: Get<ResourceId>;

        type DonorAccount: Get<Self::AccountId>;

        type DonationForAgent: Get<BalanceOf<Self>>;
    }

    #[pallet::storage]
    #[pallet::getter(fn native_check)]
    pub type NativeCheck<T> = StorageValue<_, bool, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn assets_stored)]
    pub type AssetsStored<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, bool>;

    #[pallet::storage]
    #[pallet::getter(fn associated_dominator)]
    pub type AssociatedDominator<T: Config> =
        StorageMap<_, Blake2_128Concat, u8, T::AccountId, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn agents)]
    pub type Agents<T: Config> = StorageMap<_, Blake2_128Concat, Depositer, (T::AccountId, u32)>;

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
        InvalidResourceId,
        WrongAssetId,
        InvalidTokenName,
        OverTransferLimit,
        AssetAlreadyExists,
        InvalidCallMessage,
        RegisterAgentFailed,
        DepositerNotFound,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T>
    where
        <<T as verifier::Config>::Asset as Token<<T as frame_system::Config>::AccountId>>::Balance:
            From<u128> + Into<u128>,
        <<T as verifier::Config>::Asset as Token<<T as frame_system::Config>::AccountId>>::TokenId:
            Into<u32>,
        <T::Fungibles as Token<<T as frame_system::Config>::AccountId>>::Balance:
            From<u128> + Into<u128>,
        <T::Fungibles as Token<<T as frame_system::Config>::AccountId>>::TokenId: Into<u32>,
        <T as frame_system::Config>::BlockNumber: Into<u32>,
    {
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
            match Self::is_native_resource(r_id) {
                true => Self::do_lock(source, amount, r_id, recipient, dest_id)?,
                false => Self::do_burn_assets(source, amount, r_id, recipient, dest_id)?,
            }
            Ok(())
        }

        /// Executes a simple currency transfer using the bridge account as the source
        /// Triggered by a initial transfer on source chain, executed by relayer when proposal was
        /// resolved. this function by bridge triggered transfer
        /// TODO add callback function
        #[pallet::weight(195_000_0000)]
        pub fn transfer_in(
            origin: OriginFor<T>,
            to: T::AccountId,
            amount: BalanceOf<T>,
            r_id: ResourceId,
        ) -> DispatchResult {
            let source = T::BridgeOrigin::ensure_origin(origin)?;
            match Self::is_native_resource(r_id) {
                true => {
                    Self::do_unlock(source, to.clone(), amount)?;
                    let (_, associated, _) = decode_resource_id(r_id);
                    match Self::associated_dominator(associated) {
                        Some(dominator) => {
                            let b: u128 = T::BalanceConversion::to_asset_balance(
                                amount,
                                T::Fungibles::native_token_id(),
                            )
                            .map_err(|_| Error::<T>::InvalidResourceId)?
                            .into();
                            if let Err(e) = verifier::Pallet::<T>::authorize_to(
                                to.clone(),
                                dominator,
                                <T as verifier::Config>::Asset::native_token_id(),
                                b.into(),
                            ) {
                                log::error!("failed to invoke authorize_to from {:?}, {:?}", to, e);
                            }
                        }
                        None => {}
                    }
                }
                false => {
                    Self::do_mint_assets(to.clone(), amount, r_id)?;
                    let (chain_id, associated, maybe_contract) = decode_resource_id(r_id);
                    match Self::associated_dominator(associated) {
                        Some(dominator) => {
                            let token =
                                T::AssetIdByName::try_get_asset_id(chain_id, maybe_contract)
                                    .map_err(|_| Error::<T>::InvalidResourceId)?;
                            let b: u128 = T::BalanceConversion::to_asset_balance(amount, token)
                                .map_err(|_| Error::<T>::InvalidResourceId)?
                                .into();
                            let t: u32 = token.into();
                            if let Err(e) = verifier::Pallet::<T>::authorize_to(
                                to.clone(),
                                dominator,
                                t.into(),
                                b.into(),
                            ) {
                                log::error!("failed to invoke authorize_to from {:?}, {:?}", to, e);
                            }
                        }
                        None => {}
                    }
                }
            }
            Ok(())
        }

        #[pallet::weight(195_000_0000)]
        pub fn associate_dominator(
            origin: OriginFor<T>,
            associate_id: u8,
            dominator_account: T::AccountId,
        ) -> DispatchResult {
            let _ = Self::ensure_admin(origin)?;
            AssociatedDominator::<T>::insert(associate_id, dominator_account);
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
            let message_length = message.len();
            ensure!(message_length > 4, Error::<T>::InvalidCallMessage);
            let nonce: u32 = Decode::decode(&mut &message[0..4]).unwrap();
            let c = <T as Config>::Redirect::decode(&mut &message[4..])
                .map_err(|_| <Error<T>>::InvalidCallMessage)?;
            let controller = (b"ETH".to_vec(), depositer);
            Self::execute_tx(controller, c)?;
            Agents::<T>::try_mutate_exists(depositer, |v| -> Result<(), DispatchError> {
                ensure!(v.is_some(), Error::<T>::DepositerNotFound);
                let mut map = v.take().unwrap();
                map.1 = nonce;
                v.replace(map);
                Ok(())
            })?;
            Ok(())
        }
    }

    /// IBC reference
    impl<T: Config> Agent<T::AccountId> for Pallet<T> {
        type Message = T::Redirect;
        type Origin = (Vec<u8>, Depositer);

        /// bind the origin to an appchain account without private key
        /// function RegisterInterchainAccount(counterpartyPortId: Identifier, connectionID: Identifier) returns (nil)
        fn register_agent(origin: Self::Origin) -> Result<T::AccountId, DispatchError> {
            let hash =
                (b"-*-#fusotao#-*-", origin.clone()).using_encoded(sp_io::hashing::blake2_256);
            let host_addr = Decode::decode(&mut TrailingZeroInput::new(hash.as_ref()))
                .map_err(|_| Error::<T>::RegisterAgentFailed)?;
            if !Agents::<T>::contains_key(&origin.1) {
                T::Fungibles::transfer_token(
                    &T::DonorAccount::get(),
                    T::Fungibles::native_token_id(),
                    T::DonationForAgent::get(),
                    &host_addr,
                )?;
                Agents::<T>::insert(origin.1.clone(), (host_addr.clone(), 0u32));
            }
            Ok(host_addr)
        }

        /// function AuthenticateTx(msgs []Any, connectionId string, portId string) returns (error)
        fn authenticate_tx(
            _origin: Self::Origin,
            _msg: Self::Message,
        ) -> Result<(), DispatchError> {
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

    impl<T: Config> Pallet<T> {
        fn is_native_resource(mut r_id: ResourceId) -> bool {
            let native = T::NativeResourceId::get();
            r_id[30] = 0;
            native == r_id
        }

        pub fn ensure_admin(o: T::Origin) -> DispatchResult {
            <T as pallet::Config>::AdminOrigin::ensure_origin(o)?;
            Ok(().into())
        }

        pub(crate) fn set_associated_dominator(idx: u8, dominator: T::AccountId) {
            AssociatedDominator::<T>::insert(idx, dominator);
        }

        pub(crate) fn do_lock(
            sender: T::AccountId,
            amount: BalanceOf<T>,
            r_id: ResourceId,
            recipient: Vec<u8>,
            dest_id: ChainId,
        ) -> DispatchResult {
            log::info!("transfer native token");
            let bridge_id = bridge::Pallet::<T>::account_id();
            let native_token_id = T::Fungibles::native_token_id();
            if NativeCheck::<T>::get() {
                let free_balance = T::Fungibles::free_balance(&native_token_id, &bridge_id);
                let total_balance = free_balance + amount;

                let right_balance = T::NativeTokenMaxValue::get() / 3u8.into();
                if total_balance > right_balance {
                    return Err(Error::<T>::OverTransferLimit)?;
                }
            }

            T::Fungibles::transfer_token(&sender, native_token_id, amount, &bridge_id)?;

            log::info!("transfer native token successful");
            bridge::Pallet::<T>::transfer_fungible(
                dest_id,
                r_id,
                recipient.clone(),
                U256::from(amount.saturated_into::<u128>()),
            )?;

            Ok(())
        }

        pub(crate) fn do_unlock(
            sender: T::AccountId,
            to: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let native_token_id = T::Fungibles::native_token_id();
            T::Fungibles::transfer_token(&sender, native_token_id, amount, &to)?;
            Ok(())
        }

        pub(crate) fn do_burn_assets(
            who: T::AccountId,
            amount: BalanceOf<T>,
            r_id: ResourceId,
            recipient: Vec<u8>,
            dest_id: ChainId,
        ) -> DispatchResult {
            let (chain_id, _, maybe_contract) = decode_resource_id(r_id);
            let token_id = T::AssetIdByName::try_get_asset_id(chain_id, maybe_contract)
                .map_err(|_| Error::<T>::InvalidResourceId)?;
            T::Fungibles::burn_from(token_id, &who, amount)?;
            bridge::Pallet::<T>::transfer_fungible(
                dest_id,
                r_id,
                recipient.clone(),
                U256::from(amount.saturated_into::<u128>()),
            )?;
            Ok(())
        }

        pub(crate) fn do_mint_assets(
            who: T::AccountId,
            amount: BalanceOf<T>,
            r_id: ResourceId,
        ) -> DispatchResult {
            let (chain_id, _, maybe_contract) = decode_resource_id(r_id);
            let token_id = T::AssetIdByName::try_get_asset_id(chain_id, maybe_contract)
                .map_err(|_| Error::<T>::InvalidResourceId)?;
            T::Fungibles::mint_into(token_id, &who, amount)?;
            Ok(())
        }
    }
}
