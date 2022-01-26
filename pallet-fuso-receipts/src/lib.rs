// Copyright 2021 UINB Technologies Pte. Ltd.

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
#![recursion_limit = "256"]

pub use pallet::*;

pub mod weights;

#[frame_support::pallet]
pub mod pallet {
    use codec::alloc::collections::BTreeMap;
    use codec::{Compact, Decode, Encode};
    use frame_support::traits::NamedReservableCurrency;
    use frame_support::weights::constants::RocksDbWeight;
    use frame_support::{pallet_prelude::*, traits::ReservableCurrency, transactional};
    use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use sp_io::hashing::sha2_256;
    use sp_runtime::{
        traits::{StaticLookup, Zero},
        PerThing, Percent, Permill, Perquintill, RuntimeDebug,
    };
    use sp_std::{convert::*, prelude::*, result::Result, vec::Vec};

    use fuso_support::reserve_identifier_prefix;
    use fuso_support::traits::{NamedReservableToken, ReservableToken, Token};

    use crate::weights::WeightInfo;

    pub type AmountOfCoin<T> = <T as pallet_balances::Config>::Balance;
    pub type AmountOfToken<T> = <T as pallet_fuso_token::Config>::Balance;
    pub type TokenId<T> = <T as pallet_fuso_token::Config>::TokenId;
    pub type Symbol<T> = (TokenId<T>, TokenId<T>);
    pub type Season = u32;
    pub type Amount = u128;
    pub type Price = (u128, Perquintill);

    pub type IdentifierOfCoin<T> = <T as pallet_balances::Config>::ReserveIdentifier;
    pub type IdentifierOfToken<T> = <T as pallet_fuso_token::Config>::ReserveIdentifier;

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
    pub struct MerkleLeaf {
        pub key: Vec<u8>,
        pub old_v: [u8; 32],
        pub new_v: [u8; 32],
    }

    impl MerkleLeaf {
        const ACCOUNT_KEY: u8 = 0x00;
        const ORDERBOOK_KEY: u8 = 0x01;

        fn try_get_account<T: Config>(&self) -> Result<(u32, T::AccountId), Error<T>> {
            if self.key.len() != 37 {
                return Err(Error::<T>::ProofsUnsatisfied);
            }
            match self.key[0] {
                Self::ACCOUNT_KEY => Ok((
                    u32::from_le_bytes(
                        self.key[33..]
                            .try_into()
                            .map_err(|_| Error::<T>::ProofsUnsatisfied)?,
                    ),
                    T::AccountId::decode(&mut &self.key[1..33])
                        .map_err(|_| Error::<T>::ProofsUnsatisfied)?,
                )),
                _ => Err(Error::<T>::ProofsUnsatisfied),
            }
        }

        fn try_get_symbol<T: Config>(&self) -> Result<(u32, u32), Error<T>> {
            if self.key.len() != 9 {
                return Err(Error::<T>::ProofsUnsatisfied);
            }
            match self.key[0] {
                Self::ORDERBOOK_KEY => Ok((
                    u32::from_le_bytes(
                        self.key[1..5]
                            .try_into()
                            .map_err(|_| Error::<T>::ProofsUnsatisfied)?,
                    ),
                    u32::from_le_bytes(
                        self.key[5..]
                            .try_into()
                            .map_err(|_| Error::<T>::ProofsUnsatisfied)?,
                    ),
                )),
                _ => Err(Error::<T>::ProofsUnsatisfied),
            }
        }

        fn split_value(v: &[u8; 32]) -> ([u8; 16], [u8; 16]) {
            (v[..16].try_into().unwrap(), v[16..].try_into().unwrap())
        }

        fn split_old_to_u128(&self) -> (u128, u128) {
            let (l, r) = Self::split_value(&self.old_v);
            (u128::from_le_bytes(l), u128::from_le_bytes(r))
        }

        fn split_old_to_u128sum(&self) -> u128 {
            let (l, r) = self.split_old_to_u128();
            l + r
        }

        fn split_new_to_u128(&self) -> (u128, u128) {
            let (l, r) = Self::split_value(&self.new_v);
            (u128::from_le_bytes(l), u128::from_le_bytes(r))
        }

        fn split_new_to_u128sum(&self) -> u128 {
            let (l, r) = self.split_new_to_u128();
            l + r
        }
    }

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
    pub enum Command {
        // price, amounnt, maker_fee, taker_fee, base, quote
        AskLimit(
            (Compact<u64>, Compact<u64>),
            Compact<u128>,
            Compact<u32>,
            Compact<u32>,
            Compact<u32>,
            Compact<u32>,
        ),
        BidLimit(
            (Compact<u64>, Compact<u64>),
            Compact<u128>,
            Compact<u32>,
            Compact<u32>,
            Compact<u32>,
            Compact<u32>,
        ),
        Cancel(Compact<u32>, Compact<u32>),
        TransferOut(Compact<u32>, Compact<u128>),
        TransferIn(Compact<u32>, Compact<u128>),
        // BlockNumber, Currency, Amount
        RejectTransferOut(Compact<u32>, Compact<u128>),
    }

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
    pub struct Proof<AccountId> {
        pub event_id: u64,
        pub user_id: AccountId,
        pub nonce: u32,
        pub signature: Vec<u8>,
        pub cmd: Command,
        pub leaves: Vec<MerkleLeaf>,
        pub proof_of_exists: Vec<u8>,
        pub proof_of_cmd: Vec<u8>,
        pub root: [u8; 32],
    }

    #[derive(Clone, Encode, Decode, RuntimeDebug, Eq, PartialEq, TypeInfo)]
    pub enum Receipt<Balance, BlockNumber> {
        Authorize(Balance, BlockNumber),
        Revoke(Balance, BlockNumber),
    }

    #[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
    pub struct Dominator<Coin, TokenId, BlockNumber> {
        pub staked: Coin,
        pub stablecoins: Vec<TokenId>,
        pub merkle_root: [u8; 32],
        pub start_from: BlockNumber,
        pub sequence: (u64, BlockNumber),
        pub active: bool,
    }

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
    pub struct Bonus<T: Config> {
        total_staking: UniBalance,
        profits: BoundedVec<UniBalance, T::BonusesVecLimit>,
    }

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
    pub struct Staking<Coin> {
        start_season: Season,
        amount: Coin,
    }

    pub enum StakeOption {
        STAKING,
        UNSTAKING,
    }
    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
    pub struct Share<Coin> {
        stable_coin: bool,
        amount: Coin,
    }

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
    pub struct PendingDistribution<AccountId, Coin> {
        dominator: AccountId,
        from_season: Season,
        to_season: Season,
        amount: Coin,
    }

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_balances::Config + pallet_fuso_token::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type DominatorOnlineThreshold: Get<AmountOfCoin<Self>>;

        type SeasonDuration: Get<Self::BlockNumber>;
        type SelfWeightInfo: WeightInfo;
        // type Coin: ReservableCurrency<Self::AccountId>;

        type MinimalStakingAmount: Get<AmountOfCoin<Self>>;

        type SymbolLimit: Get<usize>;

        type DominatorStablecoinLimit: Get<usize>;

        type BonusesVecLimit: Get<u32>;
    }

    #[pallet::storage]
    #[pallet::getter(fn receipts)]
    pub type Receipts<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        T::AccountId,
        Receipt<UniBalance, T::BlockNumber>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn dominators)]
    pub type Dominators<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Dominator<AmountOfCoin<T>, T::TokenId, T::BlockNumber>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn bonuses)]
    pub type Bonuses<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        Season,
        (UniBalance, BoundedVec<UniBalance, T::BonusesVecLimit>),
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn stakings)]
    pub type Stakings<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        T::AccountId,
        Staking<AmountOfCoin<T>>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        DominatorClaimed(T::AccountId),
        CoinHosted(T::AccountId, T::AccountId, AmountOfCoin<T>),
        TokenHosted(T::AccountId, T::AccountId, TokenId<T>, AmountOfToken<T>),
        CoinRevoked(T::AccountId, T::AccountId, AmountOfCoin<T>),
        TokenRevoked(T::AccountId, T::AccountId, TokenId<T>, AmountOfToken<T>),
        ProofAccepted(T::AccountId, u32),
        ProofRejected(T::AccountId, u32),
        TaoStaked(T::AccountId, T::AccountId, AmountOfCoin<T>),
        TaoUnstaked(T::AccountId, T::AccountId, AmountOfCoin<T>),
        DominatorOnline(T::AccountId),
        DominatorOffline(T::AccountId),
        DominatorSlashed(T::AccountId),
        DomintorEvicted(T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        DominatorNotFound,
        ProofsUnsatisfied,
        IllegalParameters,
        ReceiptNotExists,
        ChainNotSupport,
        ReceiptAlreadyExists,
        DominatorAlreadyExists,
        DominatorInactive,
        PledgeUnsatisfied,
        InsufficientBalance,
        InsufficientStashAccount,
        InsufficientStakingAmount,
        InvalidStatus,
        InvalidStaking,
        StakingNotExists,
        DistributionOngoing,
        OutOfStablecoinLimit,
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
    where
        AmountOfCoin<T>: Into<u128>,
        T::BlockNumber: Into<u32>,
    {
        fn on_initialize(now: T::BlockNumber) -> Weight {
            let mut weight: Weight = 0u64 as Weight;

            for dominator in Dominators::<T>::iter() {
                let start = dominator.1.start_from;
                weight = weight.saturating_add(RocksDbWeight::get().reads(1 as Weight));
                if (now - start) % T::SeasonDuration::get() == 0u32.into() {
                    let current_session: u32 =
                        ((now - start) / T::SeasonDuration::get()).into() as u32;
                    let b = (
                        UniBalance::Coin(dominator.1.staked.into()),
                        BoundedVec::default(),
                    );
                    Bonuses::<T>::insert(dominator.0, current_session, b);
                    weight = weight.saturating_add(RocksDbWeight::get().writes(1 as Weight))
                }
            }
            weight
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T>
    where
        AmountOfCoin<T>: Copy + From<u128> + Into<u128>,
        AmountOfToken<T>: Copy + From<u128> + Into<u128>,
        TokenId<T>: From<u32> + Into<u32>,
        <T as frame_system::Config>::BlockNumber: Into<u32>,
        IdentifierOfCoin<T>: From<(u8, [u8; 32])>,
        IdentifierOfToken<T>: From<(u8, [u8; 32])>,
        <T as frame_system::Config>::AccountId: Into<[u8; 32]>,
    {
        /// Initialize an empty sparse merkle tree with sequence 0 for a new dominator.
        #[pallet::weight(T::SelfWeightInfo::claim_dominator())]
        pub fn claim_dominator(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let dominator = ensure_signed(origin)?;
            ensure!(
                !Dominators::<T>::contains_key(&dominator),
                Error::<T>::DominatorAlreadyExists
            );
            let current_block = frame_system::Pallet::<T>::block_number();
            Dominators::<T>::insert(
                &dominator,
                Dominator {
                    staked: Zero::zero(),
                    stablecoins: Vec::new(),
                    start_from: current_block,
                    sequence: (0, current_block),
                    merkle_root: Default::default(),
                    active: false,
                },
            );
            Bonuses::<T>::insert(
                &dominator,
                0u32,
                (UniBalance::Coin(0), BoundedVec::default()),
            );
            Self::deposit_event(Event::DominatorClaimed(dominator));
            Ok(().into())
        }

        #[pallet::weight(T::SelfWeightInfo::add_stablecoin())]
        pub fn add_stablecoin(
            origin: OriginFor<T>,
            stablecoin: T::TokenId,
        ) -> DispatchResultWithPostInfo {
            let dex = ensure_signed(origin)?;
            ensure!(
                Dominators::<T>::contains_key(&dex),
                Error::<T>::DominatorNotFound
            );

            Dominators::<T>::try_mutate_exists(&dex, |dominator| -> DispatchResult {
                ensure!(dominator.is_some(), Error::<T>::DominatorNotFound);
                let mut dex = dominator.take().unwrap();
                let idx = dex.stablecoins.binary_search(&stablecoin);
                if !idx.is_ok() {
                    ensure!(
                        dex.stablecoins.len() < T::DominatorStablecoinLimit::get(),
                        Error::<T>::OutOfStablecoinLimit
                    );
                    dex.stablecoins
                        .insert(idx.unwrap_or_else(|x| x), stablecoin);
                }
                dominator.replace(dex);
                return Ok(());
            });
            Ok(().into())
        }

        #[pallet::weight(T::SelfWeightInfo::remove_stablecoin())]
        pub fn remove_stablecoin(
            origin: OriginFor<T>,
            stablecoin: T::TokenId,
        ) -> DispatchResultWithPostInfo {
            let dex = ensure_signed(origin)?;
            ensure!(
                Dominators::<T>::contains_key(&dex),
                Error::<T>::DominatorNotFound
            );
            Dominators::<T>::try_mutate_exists(&dex, |dominator| -> DispatchResult {
                ensure!(dominator.is_some(), Error::<T>::DominatorNotFound);
                let mut dex = dominator.take().unwrap();
                let idx = dex.stablecoins.binary_search(&stablecoin);
                if idx.is_ok() {
                    dex.stablecoins.remove(idx.unwrap());
                }
                dominator.replace(dex);
                return Ok(());
            });
            Ok(().into())
        }

        // TODO 0 gas if OK, non-zero gas otherwise
        #[pallet::weight(T::SelfWeightInfo::verify())]
        pub fn verify(
            origin: OriginFor<T>,
            proofs: Vec<Proof<T::AccountId>>,
        ) -> DispatchResultWithPostInfo {
            let dex = ensure_signed(origin)?;
            let dominator =
                Dominators::<T>::try_get(&dex).map_err(|_| Error::<T>::DominatorNotFound)?;
            ensure!(dominator.active, Error::<T>::DominatorInactive);
            for proof in proofs.into_iter() {
                Self::verify_and_update(&dex, &dominator, proof)?;
            }
            Ok(().into())
        }

        #[transactional]
        #[pallet::weight(T::SelfWeightInfo::stake())]
        pub fn stake(
            origin: OriginFor<T>,
            dominator: <T::Lookup as StaticLookup>::Source,
            amount: AmountOfCoin<T>,
        ) -> DispatchResultWithPostInfo {
            ensure!(
                amount >= T::MinimalStakingAmount::get(),
                Error::<T>::InvalidStaking
            );
            let fund_owner = ensure_signed(origin)?;
            let dex = T::Lookup::lookup(dominator)?;
            let dominator =
                Dominators::<T>::try_get(&dex).map_err(|_| Error::<T>::DominatorNotFound)?;
            let current_block = frame_system::Pallet::<T>::block_number();
            let current_season = (current_block - dominator.start_from) / T::SeasonDuration::get();
            Stakings::<T>::try_mutate(&dex, &fund_owner, |staking| -> DispatchResult {
                if staking.is_none() {
                    // TODO reserve_named
                    pallet_balances::Pallet::<T>::reserve_named(
                        &(reserve_identifier_prefix::STAKING, dex.clone().into()).into(),
                        &fund_owner,
                        amount,
                    )?;
                    staking.replace(Staking {
                        start_season: current_season.into() + 1,
                        amount,
                    });
                } else {
                    pallet_balances::Pallet::<T>::reserve_named(
                        &(reserve_identifier_prefix::STAKING, dex.clone().into()).into(),
                        &fund_owner,
                        amount,
                    )?;
                    let exists = staking.take().unwrap();
                    staking.replace(Staking {
                        start_season: current_season.into() + 1,
                        amount: exists.amount + amount,
                    });
                    let pending_distribution = PendingDistribution {
                        dominator: dex.clone(),
                        from_season: exists.start_season,
                        to_season: current_season.into(),
                        amount: exists.amount,
                    };
                    Self::take_shares(&dex.clone(), &fund_owner, &pending_distribution);
                }

                //check and update active
                let new_staking = dominator.staked + amount;
                Self::update_bonus_staking(&dex.clone(), current_season.into(), new_staking)?;
                Self::update_dominator_staked_and_active(&dex.clone(), new_staking)?;
                Self::deposit_event(Event::TaoStaked(fund_owner.clone(), dex.clone(), amount));
                Ok(())
            })?;
            // TODO
            Ok(().into())
        }

        #[transactional]
        #[pallet::weight(T::SelfWeightInfo::unstake())]
        pub fn unstake(
            origin: OriginFor<T>,
            dominator: <T::Lookup as StaticLookup>::Source,
            amount: AmountOfCoin<T>,
        ) -> DispatchResultWithPostInfo {
            let fund_owner = ensure_signed(origin)?;
            ensure!(
                amount >= T::MinimalStakingAmount::get(),
                Error::<T>::InvalidStaking
            );
            let dex = T::Lookup::lookup(dominator)?;
            let dominator =
                Dominators::<T>::try_get(&dex).map_err(|_| Error::<T>::DominatorNotFound)?;
            let current_block = frame_system::Pallet::<T>::block_number();
            let current_season = (current_block - dominator.start_from) / T::SeasonDuration::get();
            Stakings::<T>::try_mutate_exists(&dex, &fund_owner, |staking| -> DispatchResult {
                ensure!(staking.is_some(), Error::<T>::StakingNotExists);
                ensure!(
                    staking.as_ref().filter(|s| s.amount >= amount).is_some(),
                    Error::<T>::StakingNotExists
                );
                let exists = staking.take().unwrap();
                // TODO unreserve_named
                pallet_balances::Pallet::<T>::unreserve_named(
                    &(reserve_identifier_prefix::STAKING, dex.clone().into()).into(),
                    &fund_owner,
                    amount,
                );
                if exists.amount - amount >= T::MinimalStakingAmount::get() {
                    staking.replace(Staking {
                        start_season: current_season.into() + 1,
                        amount: exists.amount - amount,
                    });
                }
                let pending_distribution = PendingDistribution {
                    dominator: dex.clone(),
                    from_season: exists.start_season,
                    to_season: current_season.into(),
                    amount: exists.amount,
                };
                Self::take_shares(&dex.clone(), &fund_owner, &pending_distribution);

                //check and update active
                let new_staking = dominator.staked - amount;
                Self::update_bonus_staking(&dex.clone(), current_season.into(), new_staking)?;
                Self::update_dominator_staked_and_active(&dex.clone(), new_staking)?;
                Self::deposit_event(Event::TaoUnstaked(fund_owner.clone(), dex.clone(), amount));
                Ok(())
            })?;
            Ok(().into())
        }

        /*   #[transactional]
                #[pallet::weight(1_000_000_000_000)]
                pub fn claim_shares(
                    origin: OriginFor<T>,
                    dominator: <T::Lookup as StaticLookup>::Source,
                    token: TokenId<T>,
                ) -> DispatchResultWithPostInfo {
                    let signer = ensure_signed(origin)?;
                    let dex = T::Lookup::lookup(dominator)?;
                    let dominator =
                        Dominators::<T>::try_get(&dex).map_err(|_| Error::<T>::DominatorNotFound)?;
                    let key = BonusKey {
                        dominator: dex.clone(),
                        token: token,
                    };
                    let staking =
                        Stakings::<T>::try_get(&dex, &signer).map_err(|_| Error::<T>::InvalidStaking)?;
                    let current_block = frame_system::Pallet::<T>::block_number();
                    let current_season = (current_block - dominator.start_from) / T::SeasonDuration::get();
                    let step_into = Self::take_shares(&key, &signer, &staking, current_season.into())
                        .map_err(|_| Error::<T>::InvalidStatus)?;
                    Stakings::<T>::try_mutate(&dex, &signer, |s| -> DispatchResult {
                        let mut mutation = s.take().unwrap();
                        mutation.start_season = step_into;
                        s.replace(mutation);
                        Ok(())
                    })?;
                    Ok(().into())
                }
        */

        #[pallet::weight(T::SelfWeightInfo::authorize_coin())]
        pub fn authorize_coin(
            origin: OriginFor<T>,
            dominator: <T::Lookup as StaticLookup>::Source,
            amount: AmountOfCoin<T>,
        ) -> DispatchResultWithPostInfo {
            let fund_owner = ensure_signed(origin)?;
            let dex = T::Lookup::lookup(dominator)?;
            ensure!(
                Dominators::<T>::contains_key(&dex),
                Error::<T>::DominatorNotFound
            );
            // TODO when can dominator accept hosting
            ensure!(
                !Receipts::<T>::contains_key(&dex, &fund_owner),
                Error::<T>::ReceiptAlreadyExists,
            );
            ensure!(
                pallet_balances::Pallet::<T>::can_reserve(&fund_owner, amount),
                Error::<T>::InsufficientBalance
            );
            let block_number = frame_system::Pallet::<T>::block_number();
            pallet_balances::Pallet::<T>::reserve_named(
                &(reserve_identifier_prefix::AUTHORIZING, dex.clone().into()).into(),
                &fund_owner,
                amount,
            )?;
            Receipts::<T>::insert(
                dex.clone(),
                fund_owner.clone(),
                Receipt::Authorize(UniBalance::Coin(amount.into()), block_number),
            );
            Self::deposit_event(Event::CoinHosted(fund_owner, dex, amount));
            Ok(().into())
        }

        #[pallet::weight(T::SelfWeightInfo::revoke_coin())]
        pub fn revoke_coin(
            origin: OriginFor<T>,
            dominator: <T::Lookup as StaticLookup>::Source,
            amount: AmountOfCoin<T>,
        ) -> DispatchResultWithPostInfo {
            let fund_owner = ensure_signed(origin)?;
            let dominator = T::Lookup::lookup(dominator)?;
            ensure!(
                Dominators::<T>::contains_key(&dominator),
                Error::<T>::DominatorNotFound
            );
            // TODO when can dominator accept hosting
            ensure!(
                !Receipts::<T>::contains_key(&dominator, &fund_owner),
                Error::<T>::ReceiptAlreadyExists,
            );
            let balance = UniBalance::Coin(amount.into());
            ensure!(
                Self::has_enough_reserved(&fund_owner, &balance),
                Error::<T>::InsufficientBalance
            );
            let block_number = frame_system::Pallet::<T>::block_number();
            Receipts::<T>::insert(
                dominator.clone(),
                fund_owner.clone(),
                Receipt::Revoke(balance, block_number),
            );
            Self::deposit_event(Event::CoinRevoked(fund_owner, dominator, amount));
            Ok(().into())
        }

        #[pallet::weight(T::SelfWeightInfo::authorize_token())]
        pub fn authorize_token(
            origin: OriginFor<T>,
            dominator: <T::Lookup as StaticLookup>::Source,
            token_id: TokenId<T>,
            amount: AmountOfToken<T>,
        ) -> DispatchResultWithPostInfo {
            let fund_owner = ensure_signed(origin)?;
            let dex = T::Lookup::lookup(dominator)?;
            ensure!(
                Dominators::<T>::contains_key(&dex),
                Error::<T>::DominatorNotFound
            );
            ensure!(
                !Receipts::<T>::contains_key(&dex, &fund_owner),
                Error::<T>::ReceiptAlreadyExists,
            );
            ensure!(
                pallet_fuso_token::Pallet::<T>::can_reserve(&token_id, &fund_owner, amount),
                Error::<T>::InsufficientBalance
            );
            let block_number = frame_system::Pallet::<T>::block_number();
            pallet_fuso_token::Pallet::<T>::reserve_named(
                &(reserve_identifier_prefix::AUTHORIZING, dex.clone().into()).into(),
                &token_id,
                &fund_owner,
                amount,
            )?;
            Receipts::<T>::insert(
                dex.clone(),
                fund_owner.clone(),
                Receipt::Authorize(
                    UniBalance::Token(token_id.into(), amount.into()),
                    block_number,
                ),
            );
            Self::deposit_event(Event::TokenHosted(fund_owner, dex, token_id, amount));
            Ok(().into())
        }

        #[pallet::weight(T::SelfWeightInfo::revoke_token())]
        pub fn revoke_token(
            origin: OriginFor<T>,
            dominator: <T::Lookup as StaticLookup>::Source,
            token_id: TokenId<T>,
            amount: AmountOfToken<T>,
        ) -> DispatchResultWithPostInfo {
            let fund_owner = ensure_signed(origin)?;
            let dominator = T::Lookup::lookup(dominator)?;
            ensure!(
                Dominators::<T>::contains_key(&dominator),
                Error::<T>::DominatorNotFound
            );
            ensure!(
                !Receipts::<T>::contains_key(&dominator, &fund_owner),
                Error::<T>::ReceiptAlreadyExists,
            );
            let balance = UniBalance::Token(token_id.into(), amount.into());
            ensure!(
                // fuso_pallet_token::Pallet::<T>::can_unreserve(&token_id, &fund_owner, amount),
                Self::has_enough_reserved(&fund_owner, &balance),
                Error::<T>::InsufficientBalance
            );
            let block_number = frame_system::Pallet::<T>::block_number();
            Receipts::<T>::insert(
                dominator.clone(),
                fund_owner.clone(),
                Receipt::Revoke(balance, block_number),
            );
            Self::deposit_event(Event::TokenRevoked(fund_owner, dominator, token_id, amount));
            Ok(().into())
        }
    }

    #[derive(Encode, Decode, Eq, PartialEq, Clone, RuntimeDebug, TypeInfo)]
    pub enum UniBalance {
        Token(u32, u128),
        Coin(u128),
    }

    impl Into<u128> for UniBalance {
        fn into(self) -> u128 {
            match self {
                Self::Token(a, b) => b,
                Self::Coin(a) => a,
            }
        }
    }

    impl TryFrom<(u32, u128)> for UniBalance {
        type Error = DispatchError;

        fn try_from((token, value): (u32, u128)) -> Result<Self, Self::Error> {
            match token.clone() {
                0 => Ok(UniBalance::Coin(value)),
                id => Ok(UniBalance::Token(id, value)),
            }
        }
    }

    #[derive(Clone)]
    struct AssetsAlternate<T: Config> {
        // account, base, quote
        pub alternates: Vec<(T::AccountId, UniBalance, UniBalance)>,
        pub base_fee: UniBalance,
        pub quote_fee: UniBalance,
    }

    impl<T: Config> Pallet<T>
    where
        AmountOfCoin<T>: Copy + From<u128> + Into<u128>,
        AmountOfToken<T>: Copy + From<u128> + Into<u128>,
        TokenId<T>: From<u32>,
        IdentifierOfCoin<T>: From<(u8, [u8; 32])>,
        IdentifierOfToken<T>: From<(u8, [u8; 32])>,
        <T as frame_system::Config>::AccountId: Into<[u8; 32]>,
    {
        fn verify_and_update(
            dominator: &T::AccountId,
            ex: &Dominator<AmountOfCoin<T>, T::TokenId, T::BlockNumber>,
            proof: Proof<T::AccountId>,
        ) -> DispatchResultWithPostInfo
        where
            AmountOfCoin<T>: Copy + From<u128>,
            AmountOfToken<T>: Copy + From<u128>,
            TokenId<T>: From<u32>,
        {
            let p0 = smt::CompiledMerkleProof(proof.proof_of_exists.clone());
            let old = proof
                .leaves
                .iter()
                .map(|v| (sha2_256(&v.key).into(), v.old_v.into()))
                .collect::<Vec<_>>();
            let r = p0
                .verify::<smt::sha256::Sha256Hasher>(&ex.merkle_root.into(), old)
                .map_err(|_| Error::<T>::ProofsUnsatisfied)?;
            ensure!(r, Error::<T>::ProofsUnsatisfied);
            let p1 = smt::CompiledMerkleProof(proof.proof_of_cmd.clone());
            let new = proof
                .leaves
                .iter()
                .map(|v| (sha2_256(&v.key).into(), v.new_v.into()))
                .collect::<Vec<_>>();
            let r = p1
                .verify::<smt::sha256::Sha256Hasher>(&proof.root.into(), new)
                .map_err(|_| Error::<T>::ProofsUnsatisfied)?;
            ensure!(r, Error::<T>::ProofsUnsatisfied);
            //debug::debug!("{:?}", proof.cmd);
            match proof.cmd {
                Command::AskLimit(price, amount, maker_fee, taker_fee, base, quote) => {
                    let (n, f): (u64, u64) = (price.0.into(), price.1.into());
                    let (price, amount, maker_fee, taker_fee, base, quote) = (
                        (n.into(), Perquintill::from_parts(f.into())),
                        amount.into(),
                        Permill::from_parts(maker_fee.into()),
                        Permill::from_parts(taker_fee.into()),
                        base.into(),
                        quote.into(),
                    );
                    let delta = Self::verify_ask_limit(
                        price,
                        amount,
                        maker_fee,
                        taker_fee,
                        base,
                        quote,
                        &proof.leaves,
                    )?;
                    for d in delta.alternates {
                        Self::mutate_to(&d.0, &d.1);
                        Self::mutate_to(&d.0, &d.2);
                    }
                    Self::charge(&dominator, &delta.base_fee);
                    Self::charge(&dominator, &delta.quote_fee);
                }
                Command::BidLimit(price, amount, maker_fee, taker_fee, base, quote) => {
                    let (n, f): (u64, u64) = (price.0.into(), price.1.into());
                    let (price, amount, maker_fee, taker_fee, base, quote) = (
                        (n.into(), Perquintill::from_parts(f.into())),
                        amount.into(),
                        Permill::from_parts(maker_fee.into()),
                        Permill::from_parts(taker_fee.into()),
                        base.into(),
                        quote.into(),
                    );
                    let delta = Self::verify_bid_limit(
                        price,
                        amount,
                        maker_fee,
                        taker_fee,
                        base,
                        quote,
                        &proof.leaves,
                    )?;
                    for d in delta.alternates {
                        Self::mutate_to(&d.0, &d.1);
                        Self::mutate_to(&d.0, &d.2);
                    }
                    Self::charge(&dominator, &delta.base_fee);
                    Self::charge(&dominator, &delta.quote_fee);
                }
                Command::Cancel(base, quote) => {
                    Self::verify_cancel(base.into(), quote.into(), &proof.user_id, &proof.leaves)?;
                }
                Command::TransferOut(currency, amount) => {
                    let (currency, amount) = (currency.into(), amount.into());
                    let balance: UniBalance = (currency, amount).try_into()?;
                    let r = Receipts::<T>::get(&dominator, &proof.user_id)
                        .ok_or(Error::<T>::ReceiptNotExists)?;
                    let exists = match r {
                        Receipt::Revoke(v, _) => balance == v,
                        _ => false,
                    };
                    ensure!(exists, Error::<T>::ReceiptNotExists);
                    Self::verify_transfer_out(currency, amount, &proof.user_id, &proof.leaves)?;
                    Self::unreserve_named(&dominator, &proof.user_id, balance);
                    Receipts::<T>::remove(&dominator, &proof.user_id);
                }
                Command::TransferIn(currency, amount) => {
                    let (currency, amount) = (currency.into(), amount.into());
                    let balance: UniBalance = (currency, amount).try_into()?;
                    let r = Receipts::<T>::get(&dominator, &proof.user_id)
                        .ok_or(Error::<T>::ReceiptNotExists)?;
                    let exists = match r {
                        Receipt::Authorize(v, _) => balance == v,
                        _ => false,
                    };
                    ensure!(exists, Error::<T>::ReceiptNotExists);
                    Self::verify_transfer_in(currency, amount, &proof.user_id, &proof.leaves)?;
                    Receipts::<T>::remove(&dominator, &proof.user_id);
                }
                Command::RejectTransferOut(currency, amount) => {
                    let (currency, amount) = (currency.into(), amount.into());
                    let balance: UniBalance = (currency, amount).try_into()?;
                    let r = Receipts::<T>::get(&dominator, &proof.user_id)
                        .ok_or(Error::<T>::ReceiptNotExists)?;
                    let exists = match r {
                        Receipt::Revoke(v, _) => balance == v,
                        _ => false,
                    };
                    ensure!(exists, Error::<T>::ReceiptNotExists);
                    Self::verify_reject_transfer_out(
                        currency,
                        amount,
                        &proof.user_id,
                        &proof.leaves,
                    )?;
                    Receipts::<T>::remove(&dominator, &proof.user_id);
                    // needn't step forward
                    return Ok(().into());
                }
            }
            Dominators::<T>::mutate(&dominator, |d| {
                // TODO unwrap?
                let update = d.as_mut().unwrap();
                update.merkle_root = proof.root;
                update.sequence = (proof.event_id, frame_system::Pallet::<T>::block_number());
            });
            Ok(().into())
        }

        fn verify_ask_limit(
            _price: Price,
            amount: u128,
            maker_fee: Permill,
            taker_fee: Permill,
            base: u32,
            quote: u32,
            leaves: &[MerkleLeaf],
        ) -> Result<AssetsAlternate<T>, DispatchError> {
            ensure!(leaves.len() >= 3, Error::<T>::ProofsUnsatisfied);
            let maker_count = leaves.len() - 3;
            ensure!(maker_count % 2 == 0, Error::<T>::ProofsUnsatisfied);
            //debug::debug!("ask-limit with number of maker accounts is odd");
            let (ask0, bid0) = leaves[0].split_old_to_u128();
            let (ask1, bid1) = leaves[0].split_new_to_u128();
            // 0 or remain
            let ask_delta = ask1 - ask0;
            // equals to traded base
            let bid_delta = bid0 - bid1;

            let taker_base = &leaves[leaves.len() - 2];
            let (bk, taker_b_id) = taker_base.try_get_account::<T>()?;
            let (tba0, tbf0) = taker_base.split_old_to_u128();
            let (tba1, tbf1) = taker_base.split_new_to_u128();
            // equals to traded base
            let tb_delta = (tba0 + tbf0) - (tba1 + tbf1);

            let taker_quote = leaves.last().ok_or_else(|| Error::<T>::ProofsUnsatisfied)?;
            let (tqa0, tqf0) = taker_quote.split_old_to_u128();
            let (tqa1, tqf1) = taker_quote.split_new_to_u128();
            let (qk, taker_q_id) = taker_quote.try_get_account::<T>()?;
            let tq_delta = (tqa1 + tqf1) - (tqa0 + tqf0);
            ensure!(bk == base && qk == quote, Error::<T>::ProofsUnsatisfied);
            ensure!(taker_b_id == taker_q_id, Error::<T>::ProofsUnsatisfied);

            let taker_paid = (base, tb_delta).try_into()?;
            ensure!(
                Self::has_enough_reserved(&taker_b_id, &taker_paid),
                Error::<T>::ProofsUnsatisfied
            );
            // the delta of taker base available account(a.k.a base freezed of taker), equals to the amount of cmd
            // let taker_ba_delta = taker_ba0 - taker_ba1;

            if ask_delta != 0 {
                ensure!(amount == tba0 - tba1, Error::<T>::ProofsUnsatisfied);
            } else {
                ensure!(tbf0 == tbf1, Error::<T>::ProofsUnsatisfied);
            }
            //debug::debug!("ask-limit taker base frozen account == cmd");
            ensure!(bid_delta == tb_delta, Error::<T>::ProofsUnsatisfied);
            let mut mb_delta = 0u128;
            let mut mq_delta = 0u128;
            let mut delta = Vec::new();
            for i in 0..maker_count / 2 {
                // base first
                let maker_base = &leaves[i * 2 + 1];
                let (bk, maker_b_id) = maker_base.try_get_account::<T>()?;
                let mb0 = maker_base.split_old_to_u128sum();
                let mb1 = maker_base.split_new_to_u128sum();
                let base_incr = mb1 - mb0;
                mb_delta += base_incr;
                // then quote account
                let maker_quote = &leaves[i * 2 + 2];
                let (qk, maker_q_id) = maker_quote.try_get_account::<T>()?;
                ensure!(base == bk && quote == qk, Error::<T>::ProofsUnsatisfied);
                let mq0 = maker_quote.split_old_to_u128sum();
                let mq1 = maker_quote.split_new_to_u128sum();
                let quote_decr = mq0 - mq1;
                mq_delta += quote_decr;
                // the accounts should be owned by same user
                ensure!(maker_b_id == maker_q_id, Error::<T>::ProofsUnsatisfied);
                ensure!(
                    Self::has_enough_reserved(&maker_q_id, &(quote, quote_decr).try_into()?),
                    Error::<T>::ProofsUnsatisfied
                );
                delta.push((
                    maker_b_id,
                    (base, mb1).try_into()?,
                    (quote, mq1).try_into()?,
                ));
            }
            //debug::debug!("ask-limit all makers ok");
            // FIXME ceil
            let base_charged = maker_fee.mul_ceil(tb_delta);
            ensure!(
                mb_delta + base_charged == tb_delta,
                Error::<T>::ProofsUnsatisfied
            );
            //debug::debug!("ask-limit traded_base == base_fee + sum_of_maker_base_delta");
            // FIXME ceil
            let quote_charged = taker_fee.mul_ceil(mq_delta);
            ensure!(
                mq_delta == tq_delta + quote_charged,
                Error::<T>::ProofsUnsatisfied
            );
            delta.push((
                taker_b_id,
                (base, tba1 + tbf1).try_into()?,
                (quote, tqa1 + tqf1).try_into()?,
            ));
            //debug::debug!("ask-limit taker_quote_available_delta + quote_fee == sum_of_maker_quote_frozen_delta");
            Ok(AssetsAlternate {
                alternates: delta,
                base_fee: (base, base_charged).try_into()?,
                quote_fee: (quote, quote_charged).try_into()?,
            })
        }

        fn verify_bid_limit(
            _price: Price,
            amount: u128,
            maker_fee: Permill,
            taker_fee: Permill,
            base: u32,
            quote: u32,
            leaves: &[MerkleLeaf],
        ) -> Result<AssetsAlternate<T>, DispatchError> {
            ensure!(leaves.len() >= 3, Error::<T>::ProofsUnsatisfied);
            let maker_count = leaves.len() - 3;
            ensure!(maker_count % 2 == 0, Error::<T>::ProofsUnsatisfied);
            //debug::debug!("bid-limit with number of maker accounts is odd");
            let (ask0, bid0) = leaves[0].split_old_to_u128();
            let (ask1, bid1) = leaves[0].split_new_to_u128();
            let ask_delta = ask0 - ask1;
            let bid_delta = bid1 - bid0;

            let taker_base = &leaves[leaves.len() - 2];
            let (tba0, tbf0) = taker_base.split_old_to_u128();
            let (tba1, tbf1) = taker_base.split_new_to_u128();
            let tb_delta = (tba1 + tbf1) - (tba0 + tbf0);
            let (bk, taker_b_id) = taker_base.try_get_account::<T>()?;
            let taker_quote = leaves.last().ok_or_else(|| Error::<T>::ProofsUnsatisfied)?;
            let (tqa0, tqf0) = taker_quote.split_old_to_u128();
            let (tqa1, tqf1) = taker_quote.split_new_to_u128();
            let (qk, taker_q_id) = taker_quote.try_get_account::<T>()?;
            let tq_delta = (tqa0 + tqf0) - (tqa1 + tqf1);
            ensure!(bk == base && qk == quote, Error::<T>::ProofsUnsatisfied);
            ensure!(taker_b_id == taker_q_id, Error::<T>::ProofsUnsatisfied);

            // unsatisfied:
            // if bid_delta != 0 {
            //     ensure!(frozen_vol == tqa0 - tqa1, Error::<T>::ProofsUnsatisfied);
            // } else {
            //     ensure!(tqf0 == tqf1, Error::<T>::ProofsUnsatisfied);
            // }

            ensure!(
                Self::has_enough_reserved(&taker_q_id, &(quote, tq_delta).try_into()?),
                Error::<T>::ProofsUnsatisfied
            );
            let mut mb_delta = 0u128;
            let mut mq_delta = 0u128;
            let mut delta = Vec::new();
            for i in 0..maker_count / 2 {
                // base first
                let maker_base = &leaves[i * 2 + 1];
                let (bk, maker_b_id) = maker_base.try_get_account::<T>()?;
                let mb0 = maker_base.split_old_to_u128sum();
                let mb1 = maker_base.split_new_to_u128sum();
                let base_decr = mb0 - mb1;
                mb_delta += base_decr;
                // then quote
                let maker_quote = &leaves[i * 2 + 2];
                let (qk, maker_q_id) = maker_quote.try_get_account::<T>()?;
                ensure!(quote == qk && base == bk, Error::<T>::ProofsUnsatisfied);
                ensure!(maker_b_id == maker_q_id, Error::<T>::ProofsUnsatisfied);
                let mq0 = maker_quote.split_old_to_u128sum();
                let mq1 = maker_quote.split_new_to_u128sum();
                let quote_incr = mq1 - mq0;
                mq_delta += quote_incr;
                ensure!(
                    Self::has_enough_reserved(&maker_b_id, &(base, base_decr).try_into()?),
                    Error::<T>::ProofsUnsatisfied
                );
                delta.push((
                    maker_b_id,
                    (base, mb1).try_into()?,
                    (quote, mq1).try_into()?,
                ));
            }
            //debug::debug!("bid-limit makers ok");
            // FIXME ceil
            let quote_charged = maker_fee.mul_ceil(tq_delta);
            ensure!(
                mq_delta + quote_charged == tq_delta,
                Error::<T>::ProofsUnsatisfied
            );
            //debug::debug!("bid-limit maker_quote_delta + quote_charged == traded_quote");
            // FIXME ceil
            let base_charged = taker_fee.mul_ceil(mb_delta);
            ensure!(
                tb_delta + base_charged == mb_delta,
                Error::<T>::ProofsUnsatisfied
            );
            //debug::debug!("bid-limit taker_base_available_delta + base_charged == traded_base");
            ensure!(ask_delta == mb_delta, Error::<T>::ProofsUnsatisfied);
            //debug::debug!("bid-limit orderbook_ask_size_delta == traded_base");
            if bid_delta != 0 {
                ensure!(
                    bid_delta == amount - mb_delta,
                    Error::<T>::ProofsUnsatisfied
                );
            } else {
                // TODO to avoid divide
            }
            //debug::debug!("bid-limit orderbook_bid_size_delta == untraded_base");
            delta.push((
                taker_b_id,
                (base, tba1 + tbf1).try_into()?,
                (quote, tqa1 + tqf1).try_into()?,
            ));
            Ok(AssetsAlternate {
                alternates: delta,
                base_fee: (base, base_charged).try_into()?,
                quote_fee: (quote, quote_charged).try_into()?,
            })
        }

        fn verify_transfer_in(
            currency: u32,
            amount: u128,
            account: &T::AccountId,
            leaves: &[MerkleLeaf],
        ) -> Result<(), DispatchError> {
            ensure!(leaves.len() == 1, Error::<T>::ProofsUnsatisfied);
            let (a0, f0) = leaves[0].split_old_to_u128();
            let (a1, f1) = leaves[0].split_new_to_u128();
            ensure!(a1 - a0 == amount, Error::<T>::ProofsUnsatisfied);
            ensure!(f1 == f0, Error::<T>::ProofsUnsatisfied);
            let (c, id) = leaves[0].try_get_account::<T>()?;
            ensure!(
                currency == c && account == &id,
                Error::<T>::ProofsUnsatisfied
            );
            Ok(())
        }

        fn verify_transfer_out(
            currency: u32,
            amount: u128,
            account: &T::AccountId,
            leaves: &[MerkleLeaf],
        ) -> Result<(), DispatchError> {
            ensure!(leaves.len() == 1, Error::<T>::ProofsUnsatisfied);
            let (a0, f0) = leaves[0].split_old_to_u128();
            let (a1, f1) = leaves[0].split_new_to_u128();
            ensure!(a0 - a1 == amount, Error::<T>::ProofsUnsatisfied);
            ensure!(f1 == f0, Error::<T>::ProofsUnsatisfied);
            let (c, id) = leaves[0].try_get_account::<T>()?;
            ensure!(
                currency == c && account == &id,
                Error::<T>::ProofsUnsatisfied
            );
            Ok(())
        }

        fn verify_reject_transfer_out(
            currency: u32,
            amount: u128,
            account: &T::AccountId,
            leaves: &[MerkleLeaf],
        ) -> Result<(), DispatchError> {
            ensure!(leaves.len() == 1, Error::<T>::ProofsUnsatisfied);
            let (a0, _) = leaves[0].split_old_to_u128();
            ensure!(a0 < amount, Error::<T>::ProofsUnsatisfied);
            let (c, id) = leaves[0].try_get_account::<T>()?;
            ensure!(
                currency == c && account == &id,
                Error::<T>::ProofsUnsatisfied
            );
            Ok(())
        }

        fn verify_cancel(
            base: u32,
            quote: u32,
            account: &T::AccountId,
            leaves: &[MerkleLeaf],
        ) -> Result<(), DispatchError> {
            ensure!(leaves.len() == 3, Error::<T>::ProofsUnsatisfied);
            let (b, q) = leaves[0].try_get_symbol::<T>()?;
            ensure!(b == base && q == quote, Error::<T>::ProofsUnsatisfied);
            let (ask0, bid0) = leaves[0].split_old_to_u128();
            let (ask1, bid1) = leaves[0].split_new_to_u128();
            let ask_delta = ask0 - ask1;
            let bid_delta = bid0 - bid1;
            ensure!(ask_delta + bid_delta != 0, Error::<T>::ProofsUnsatisfied);
            ensure!(ask_delta & bid_delta == 0, Error::<T>::ProofsUnsatisfied);

            let (b, id) = leaves[1].try_get_account::<T>()?;
            ensure!(b == base, Error::<T>::ProofsUnsatisfied);
            ensure!(account == &id, Error::<T>::ProofsUnsatisfied);
            let (ba0, bf0) = leaves[1].split_old_to_u128();
            let (ba1, bf1) = leaves[1].split_new_to_u128();
            ensure!(ba0 + bf0 == ba1 + bf1, Error::<T>::ProofsUnsatisfied);

            let (q, id) = leaves[2].try_get_account::<T>()?;
            ensure!(q == quote, Error::<T>::ProofsUnsatisfied);
            ensure!(account == &id, Error::<T>::ProofsUnsatisfied);
            let (qa0, qf0) = leaves[2].split_old_to_u128();
            let (qa1, qf1) = leaves[2].split_new_to_u128();
            ensure!(qa0 + qf0 == qa1 + qf1, Error::<T>::ProofsUnsatisfied);
            Ok(())
        }

        fn has_enough_reserved(who: &T::AccountId, balance: &UniBalance) -> bool {
            match balance {
                UniBalance::Coin(value) => {
                    pallet_balances::Pallet::<T>::reserved_balance(who) >= (*value).into()
                }
                UniBalance::Token(id, value) => {
                    pallet_fuso_token::Pallet::<T>::reserved_balance(&(*id).into(), who)
                        >= (*value).into()
                }
            }
        }

        fn mutate_to(who: &T::AccountId, balance: &UniBalance) {
            match balance {
                UniBalance::Coin(value) => {
                    pallet_balances::Pallet::<T>::mutate_account(who, |a| {
                        a.reserved = (*value).into()
                    });
                }
                UniBalance::Token(id, value) => {
                    pallet_fuso_token::Pallet::<T>::mutate_account(&(*id).into(), who, |a| {
                        a.reserved = (*value).into()
                    });
                }
            }
        }

        fn unreserve_named(dominator: &T::AccountId, who: &T::AccountId, balance: UniBalance) {
            match balance {
                UniBalance::Coin(value) => {
                    pallet_balances::Pallet::<T>::unreserve_named(
                        &(
                            reserve_identifier_prefix::AUTHORIZING,
                            dominator.clone().into(),
                        )
                            .into(),
                        who,
                        value.into(),
                    );
                }
                UniBalance::Token(id, value) => {
                    pallet_fuso_token::Pallet::<T>::unreserve_named(
                        &(
                            reserve_identifier_prefix::AUTHORIZING,
                            dominator.clone().into(),
                        )
                            .into(),
                        &id.into(),
                        who,
                        value.into(),
                    );
                }
            }
        }

        #[transactional]
        fn charge(who: &T::AccountId, balance: &UniBalance) -> Result<(), ()>
        where
            AmountOfCoin<T>: Copy + From<u128>,
            TokenId<T>: From<u32>,
            AmountOfToken<T>: Copy + From<u128>,
        {
            match balance {
                UniBalance::Coin(value) => pallet_balances::Pallet::<T>::mutate_account(who, |a| {
                    a.reserved += (*value).into();
                })
                .map(|_| ())
                .map_err(|_| ()),
                UniBalance::Token(id, value) => {
                    pallet_fuso_token::Pallet::<T>::mutate_account(&(*id).into(), who, |a| {
                        a.reserved += (*value).into();
                    })
                    .map(|_| ())
                    .map_err(|_| ())
                }
            }
        }

        #[transactional]
        fn update_bonus_staking(
            dex: &T::AccountId,
            season: Season,
            new_staking: AmountOfCoin<T>,
        ) -> Result<(), Error<T>> {
            match Bonuses::<T>::get(dex, season) {
                Some(bonus) => {
                    let b = (UniBalance::Coin(new_staking.into()), bonus.1);
                    Bonuses::<T>::insert(dex, season, b);
                }
                None => {
                    let b = (UniBalance::Coin(new_staking.into()), BoundedVec::default());
                    Bonuses::<T>::insert(dex, season, b);
                }
            };
            Ok(())
        }

        fn update_dominator_staked_and_active(
            dex: &T::AccountId,
            new_staking: AmountOfCoin<T>,
        ) -> Result<(), Error<T>> {
            Dominators::<T>::try_mutate_exists(&dex, |dominator| -> DispatchResult {
                ensure!(dominator.is_some(), Error::<T>::DominatorNotFound);
                let dex = &mut dominator.take().unwrap();
                dominator.replace(Dominator {
                    staked: new_staking,
                    stablecoins: dex.clone().stablecoins,
                    start_from: dex.start_from,
                    sequence: dex.sequence,
                    merkle_root: dex.merkle_root,
                    active: new_staking.into() >= T::DominatorOnlineThreshold::get().into(),
                });
                Ok(())
            });
            Ok(())
        }

        #[transactional]
        fn take_shares(
            dex: &T::AccountId,
            staker: &T::AccountId,
            pending_distribution: &PendingDistribution<T::AccountId, AmountOfCoin<T>>,
        ) -> Result<Season, ()>
        where
            <T as pallet_fuso_token::Config>::TokenId: From<u32>,
        {
            if pending_distribution.to_season == pending_distribution.from_season {
                return Ok(pending_distribution.from_season);
            }
            let mut shares_map: BTreeMap<u32, u128> = BTreeMap::new();
            for season in pending_distribution.from_season..pending_distribution.to_season {
                match Bonuses::<T>::get(dex, season) {
                    Some(bonus) => {
                        for b in bonus.clone().1 {
                            let s: u128 = pending_distribution.amount.into();
                            match b {
                                UniBalance::Token(a, c) => {
                                    let mut share = *shares_map.get(&a).unwrap_or(&0u128);
                                    let t: u128 = bonus.0.clone().into();
                                    let p: Perquintill = PerThing::from_rational(s, t);
                                    share += p * c;
                                    shares_map.insert(a, share);
                                }
                                _ => {}
                            }
                        }
                    }
                    None => {}
                };
            }
            for share in shares_map {
                if share.0 == 0 {
                    pallet_balances::Pallet::<T>::mutate_account(staker, |a| {
                        a.free += share.1.into()
                    })
                    .map_err(|_| ())
                    .map(|_| pending_distribution.to_season);
                } else {
                    pallet_fuso_token::Pallet::<T>::mutate_account(&share.0.into(), staker, |a| {
                        a.free += share.1.into()
                    })
                    .map_err(|_| ())
                    .map(|_| pending_distribution.to_season);
                }
            }
            Ok(pending_distribution.to_season)
        }
    }
}
