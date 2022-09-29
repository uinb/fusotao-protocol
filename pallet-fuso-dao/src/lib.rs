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

        type Call: Parameter
            + Dispatchable<Origin = Self::Origin, PostInfo = PostDispatchInfo>
            + GetDispatchInfo
            + From<frame_system::Call<Self>>;
    }

    // TODO
    pub type ProposalIndex = u32;

    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
    pub struct DAO<AccountId, TokenId, Balance, BlockNumber> {
        pub name: Vec<u8>,
        pub logo: Vec<u8>,
        pub lang: Vec<u8>,
        pub gov_token: TokenId,
        pub mintable: bool,
        pub mint_by: TokenId,
        pub treasury: AccountId,
        pub originator: AccountId,
        pub max_members: u32,
        pub rule: DAORule<BlockNumber>,
        pub valid: bool,
        pub proposal_index: ProposalIndex,
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, Debug)]
    pub enum Proposal<AccountId, BlockNumber, Call, TokenId, Balance> {
        OnchainNeedCharge {
            pub issuer: AccountId,
            pub expire_at: BlockNumber,
            pub status: u8,
            pub url: Vec<u8>,
            pub call: Call,
            pub token_id: TokenId,
            pub amount: Balance,
            pub voting: (Balance, Balance, Balance),
        },
        OffchainNeedCharge {
            pub issuer: AccountId,
            pub expire_at: BlockNumber,
            pub status: u8,
            pub url: Vec<u8>,
            pub token_id: TokenId,
            pub amount: Balance,
            pub voting: (Balance, Balance, Balance),
        },
        Onchain {
            pub issuer: AccountId,
            pub expire_at: BlockNumber,
            pub status: u8,
            pub url: Vec<u8>,
            pub call: Call,
            pub voting: (Balance, Balance, Balance),
        },
        Offchain {
            pub issuer: AccountId,
            pub expire_at: BlockNumber,
            pub status: u8,
            pub url: Vec<u8>,
            pub voting: (Balance, Balance, Balance),
        },
    }

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
            mint_by: TokenId,
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
        Proposal<T::AccountId, T::BlockNumber, T::Call, TokenId<T>, Balance<T>>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {
        DaoNameAlreadyExisted,
        AccountImplyError,
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
            proposal: Box<T::Call>,
            expire_at: T::BlockNumber,
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

        /// invoke by the originators
        #[pallet::weight(1_000_000_000_000_000_000)]
        pub fn create(
            origin: OriginFor<T>,
            name: Vec<u8>,
            logo: Vec<u8>,
            lang: Vec<u8>,
            max_members: u32,
            gov_token: GovernanceToken<TokenId<T>, Balance<T>>,
            rule: DAORule<T::BlockNumber>,
            entry_threshold: Balance<T>,
        ) -> DispatchResultWithPostInfo {
            let originator = ensure_signed(origin)?;
            let treasury_account = Self::imply_account(name.clone(), ProposalIndex::default())?;
            ensure!(
                !Orgs::<T>::contains_key(&treasury_account),
                Error::<T>::DaoNameAlreadyExisted
            );
            // let gov_token = match gov_token {
            //     Existing(token_id) => token_id,
            //     New {
            //         token_symbol,
            //         mint_by,
            //     } => {

            //     }
            // }
            // TODO params check
            Orgs::<T>::insert(
                treasury_account,
                DAO {
                    name,
                    logo,
                    lang,
                    treasury: treasury_account,
                    originator,
                    max_members,
                    rule,
                    valid: false,
                    proposal_index: ProposalIndex::default(),
                },
            );
            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn imply_account(
            name: Vec<u8>,
            index: ProposalIndex,
        ) -> Result<T::AccountId, Error<T>> {
            let deterministic =
                (b"#_fuso_dao_#", name, index).using_encoded(sp_io::hashing::blake2_256);
            Decode::decode(&mut TrailingZeroInput::new(deterministic.as_ref()))
                .map_err(|_| Error::<T>::AccountImplyError)
        }

        fn join(joinee: T::AccountId, dao: T::AccountId) -> DispatchResult<()> {
            Ok(())
        }
    }
}
