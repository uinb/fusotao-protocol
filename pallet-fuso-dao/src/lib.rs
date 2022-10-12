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

#[frame_support::pallet]
pub mod pallet {
    use codec::{Codec, EncodeLike};
    use frame_support::{pallet_prelude::*, traits::Get, weights::GetDispatchInfo};
    use frame_system::{ensure_signed, pallet_prelude::*};
    use fuso_support::{
        constants::*,
        traits::{ReservableToken, Rewarding, Token},
        XToken,
    };
    use sp_runtime::{
        traits::{CheckedAdd, Dispatchable, TrailingZeroInput, Zero},
        DispatchError, DispatchResult, Percent,
    };
    use sp_std::{boxed::Box, vec::Vec};

    pub type TokenId<T> =
        <<T as Config>::Asset as Token<<T as frame_system::Config>::AccountId>>::TokenId;

    pub type Balance<T> =
        <<T as Config>::Asset as Token<<T as frame_system::Config>::AccountId>>::Balance;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type Asset: Token<Self::AccountId>;

        type Function: Parameter
            + Dispatchable<Origin = Self::Origin, PostInfo = PostDispatchInfo>
            + GetDispatchInfo
            + From<frame_system::Call<Self>>;
    }

    pub type ProposalIndex = u32;

    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
    pub struct DAO<AccountId, TokenId, Balance, BlockNumber> {
        pub name: Vec<u8>,
        pub logo: Vec<u8>,
        pub lang: Vec<u8>,
        pub gov_token: TokenId,
        pub swapable: bool,
        pub swap_by: TokenId,
        pub treasury: AccountId,
        pub originator: AccountId,
        pub rule: DAORule<BlockNumber>,
        pub valid: bool,
        pub proposal_index: ProposalIndex,
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
    pub enum Proposal<AccountId, BlockNumber, Function, Balance> {
        Onchain {
            issuer: AccountId,
            expire_at: BlockNumber,
            status: u8,
            url: Vec<u8>,
            call: Function,
            voting: (Balance, Balance, Balance),
        },
        Offchain {
            issuer: AccountId,
            expire_at: BlockNumber,
            status: u8,
            url: Vec<u8>,
            voting: (Balance, Balance, Balance),
        },
    }

    pub type ProposalOf<T> = Proposal<T::AccountId, T::BlockNumber, T::Function, Balance<T>>;

    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
    pub enum DAORule<Term> {
        Unanimity,
        Majority(Percent),
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
    pub enum GovernanceToken<TokenId, Balance> {
        Existing(TokenId),
        New {
            token_symbol: Vec<u8>,
            swap_by: TokenId,
            max_supply: Balance,
        },
    }

    #[pallet::storage]
    #[pallet::getter(fn orgs)]
    pub type Orgs<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        DAO<T::AccountId, TokenId<T>, Balance<T>, T::BlockNumber>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn members)]
    pub type Members<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        T::AccountId,
        Balance<T>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn proposals)]
    pub type Proposals<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        ProposalIndex,
        ProposalOf<T>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {
        DaoNameAlreadyExisted,
        AccountImplyError,
        DaoNotExists,
        GovTokenIsNotMintable,
        InsufficientBalance,
        MinimalRequired,
        GovTokenBeyondMaximum,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::pallet]
    #[pallet::without_storage_info]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // TODO
        #[pallet::weight(0)]
        pub fn proposal(
            origin: OriginFor<T>,
            org: T::AccountId,
            proposal: ProposalOf<T>,
        ) -> DispatchResultWithPostInfo {
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn approve(
            origin: OriginFor<T>,
            org: T::AccountId,
            index: ProposalIndex,
        ) -> DispatchResultWithPostInfo {
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn reject(
            origin: OriginFor<T>,
            org: T::AccountId,
            index: ProposalIndex,
        ) -> DispatchResultWithPostInfo {
            Ok(().into())
        }

        #[tranactional]
        #[pallet::weight(1_000_000_000_000)]
        pub fn purchase(
            origin: OriginFor<T>,
            treasury: T::AccountId,
            amount: Balance<T>,
        ) -> DispatchResultWithPostInfo {
            let origin = ensure_signed(origin)?;
            Orgs::<T>::try_mutate_exists(&treasury, |dao| -> DispatchResult {
                ensure!(dao.is_some(), Error::<T>::DaoNotExists);
                let dao = dao.unwrap();
                ensure!(dao.swapable, Error::<T>::GovTokenIsNotMintable);
                ensure!(amount >= dao.entry_threshold, Error::<T>::MinimalRequired);
                ensure!(
                    T::Asset::free_balance(&dao.swap_by, &origin) >= amount,
                    Error::<T>::InsufficientBalance
                );
                ensure!(
                    T::Asset::free_balance(&dao.gov_token, &treasury) >= amount,
                    Error::<T>::GovTokenBeyondMaximum,
                );
                T::Asset::transfer(&dao.swap_by, origin, treasury, amount)?;
                T::Asset::transfer(&dao.gov_token, treasury, origin, amount)?;
            })?;
            Ok(().into())
        }

        /// invoke by the originators
        #[pallet::weight(1_000_000_000_000_000_000)]
        pub fn create(
            origin: OriginFor<T>,
            name: Vec<u8>,
            logo: Vec<u8>,
            lang: Vec<u8>,
            gov_token: GovernanceToken<TokenId<T>, Balance<T>>,
            rule: DAORule<T::BlockNumber>,
            entry_threshold: Balance<T>,
        ) -> DispatchResultWithPostInfo {
            let originator = ensure_signed(origin)?;
            let treasury = Self::imply_account(name.clone())?;
            ensure!(
                !Orgs::<T>::contains_key(&treasury),
                Error::<T>::DaoNameAlreadyExisted
            );
            let (gov_token, swapable, mint_by) = match gov_token {
                Existing(token_id) => (token_id, false, token_id),
                New {
                    token_symbol,
                    swap_by,
                    max_supply,
                } => {
                    let token_info = XToken::FND10(token_symbol.clone(), max_supply);
                    let token_id = T::Token::create(token_info)?;
                    (token_id, true, swap_by)
                }
            };
            // TODO params check
            Orgs::<T>::insert(
                treasury,
                DAO {
                    name,
                    logo,
                    lang,
                    gov_token,
                    swapable,
                    mint_by,
                    treasury,
                    originator,
                    rule,
                    valid: false,
                    proposal_index: ProposalIndex::default(),
                },
            );
            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn imply_account(name: Vec<u8>) -> Result<T::AccountId, Error<T>> {
            let deterministic = (b"#_fuso_dao_#", name).using_encoded(sp_io::hashing::blake2_256);
            Decode::decode(&mut TrailingZeroInput::new(deterministic.as_ref()))
                .map_err(|_| Error::<T>::AccountImplyError)
        }
    }
}
