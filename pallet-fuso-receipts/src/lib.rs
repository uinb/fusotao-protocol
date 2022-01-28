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
    use crate::weights::WeightInfo;
    use codec::alloc::collections::BTreeMap;
    use codec::{Compact, Decode, Encode};
    use frame_support::{
        weights::constants::RocksDbWeight,
        {pallet_prelude::*, transactional},
    };
    use frame_system::pallet_prelude::*;
    use fuso_support::{
        constants,
        traits::{ReservableToken, Token},
    };
    use scale_info::TypeInfo;
    use sp_io::hashing::sha2_256;
    use sp_runtime::{
        traits::{CheckedSub, StaticLookup, Zero},
        PerThing, Permill, Perquintill, RuntimeDebug,
    };
    use sp_std::{convert::*, prelude::*, result::Result, vec::Vec};

    pub type TokenId<T> =
        <<T as Config>::Asset as Token<<T as frame_system::Config>::AccountId>>::TokenId;
    pub type Balance<T> =
        <<T as Config>::Asset as Token<<T as frame_system::Config>::AccountId>>::Balance;
    pub type UniBalanceOf<T> = (TokenId<T>, Balance<T>);
    pub type Symbol<T> = (TokenId<T>, TokenId<T>);
    pub type Season = u32;
    pub type Amount = u128;
    pub type Price = (u128, Perquintill);

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
        // price, amount, maker_fee, taker_fee, base, quote
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
    pub enum Receipt<TokenId, Balance, BlockNumber> {
        Authorize(TokenId, Balance, BlockNumber),
        Revoke(TokenId, Balance, BlockNumber),
    }

    #[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
    pub struct Dominator<TokenId, Balance, BlockNumber> {
        pub staked: Balance,
        pub stablecoins: Vec<TokenId>,
        pub merkle_root: [u8; 32],
        pub start_from: BlockNumber,
        pub sequence: (u64, BlockNumber),
        pub active: bool,
    }

    #[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, Default)]
    pub struct Staking<Balance> {
        from_season: Season,
        amount: Balance,
    }

    #[derive(Clone, RuntimeDebug)]
    struct Distribution<AccountId, Balance> {
        dominator: AccountId,
        from_season: Season,
        to_season: Season,
        staking: Balance,
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type Asset: ReservableToken<Self::AccountId>;

        type DominatorOnlineThreshold: Get<Balance<Self>>;

        type SeasonDuration: Get<Self::BlockNumber>;

        type SelfWeightInfo: WeightInfo;

        type MinimalStakingAmount: Get<Balance<Self>>;

        type MaxDominators: Get<u32>;

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
        Receipt<TokenId<T>, Balance<T>, T::BlockNumber>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn dominators)]
    pub type Dominators<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Dominator<TokenId<T>, Balance<T>, T::BlockNumber>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn reserves)]
    pub type Reserves<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        (u8, T::AccountId, TokenId<T>),
        Blake2_128Concat,
        T::AccountId,
        Balance<T>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn bonuses)]
    pub type Bonuses<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        Season,
        (Balance<T>, BoundedVec<UniBalanceOf<T>, T::BonusesVecLimit>),
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn stakings)]
    pub type Stakings<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        T::AccountId,
        Staking<Balance<T>>,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        DominatorClaimed(T::AccountId),
        CoinHosted(T::AccountId, T::AccountId, Balance<T>),
        TokenHosted(T::AccountId, T::AccountId, TokenId<T>, Balance<T>),
        CoinRevoked(T::AccountId, T::AccountId, Balance<T>),
        TokenRevoked(T::AccountId, T::AccountId, TokenId<T>, Balance<T>),
        ProofAccepted(T::AccountId, u32),
        ProofRejected(T::AccountId, u32),
        TaoStaked(T::AccountId, T::AccountId, Balance<T>),
        TaoUnstaked(T::AccountId, T::AccountId, Balance<T>),
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
        LittleStakingAmount,
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
    where
        Balance<T>: Into<u128>,
        T::BlockNumber: Into<u32>,
    {
        fn on_initialize(now: T::BlockNumber) -> Weight {
            let mut weight: Weight = 0u64 as Weight;

            // FIXME dominator offline
            for dominator in Dominators::<T>::iter() {
                let start = dominator.1.start_from;
                weight = weight.saturating_add(RocksDbWeight::get().reads(1 as Weight));
                if (now - start) % T::SeasonDuration::get() == 0u32.into() {
                    let current_season: u32 =
                        ((now - start) / T::SeasonDuration::get()).into() as u32;
                    let b = (dominator.1.staked, BoundedVec::default());
                    Bonuses::<T>::insert(dominator.0, current_season, b);
                    weight = weight.saturating_add(RocksDbWeight::get().writes(1 as Weight))
                }
            }
            weight
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T>
    where
        TokenId<T>: Copy + From<u32> + Into<u32>,
        Balance<T>: Copy + From<u128> + Into<u128>,
        T::BlockNumber: Into<u32> + From<u32>,
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
                (Balance::<T>::zero(), BoundedVec::default()),
            );
            Self::deposit_event(Event::DominatorClaimed(dominator));
            Ok(().into())
        }

        #[pallet::weight(T::SelfWeightInfo::add_stablecoin())]
        pub fn add_stablecoin(
            origin: OriginFor<T>,
            stablecoin: TokenId<T>,
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
            })?;
            Ok(().into())
        }

        #[pallet::weight(T::SelfWeightInfo::remove_stablecoin())]
        pub fn remove_stablecoin(
            origin: OriginFor<T>,
            stablecoin: TokenId<T>,
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
            })?;
            Ok(().into())
        }

        #[pallet::weight(T::SelfWeightInfo::verify())]
        pub fn verify(
            origin: OriginFor<T>,
            proofs: Vec<Proof<T::AccountId>>,
        ) -> DispatchResultWithPostInfo {
            let dominator_id = ensure_signed(origin)?;
            let dominator = Dominators::<T>::try_get(&dominator_id)
                .map_err(|_| Error::<T>::DominatorNotFound)?;
            ensure!(dominator.active, Error::<T>::DominatorInactive);
            for proof in proofs.into_iter() {
                Self::verify_and_update(&dominator_id, &dominator, proof)?;
            }
            Ok(Some(0).into())
        }

        #[transactional]
        #[pallet::weight(T::SelfWeightInfo::stake())]
        pub fn stake(
            origin: OriginFor<T>,
            dominator: <T::Lookup as StaticLookup>::Source,
            amount: Balance<T>,
        ) -> DispatchResultWithPostInfo {
            let staker = ensure_signed(origin)?;
            let dominator = T::Lookup::lookup(dominator)?;
            Self::stake_on(&staker, &dominator, amount)?;
            Ok(().into())
        }

        #[transactional]
        #[pallet::weight(T::SelfWeightInfo::unstake())]
        pub fn unstake(
            origin: OriginFor<T>,
            dominator: <T::Lookup as StaticLookup>::Source,
            amount: Balance<T>,
        ) -> DispatchResultWithPostInfo {
            let staker = ensure_signed(origin)?;
            let dominator = T::Lookup::lookup(dominator)?;
            Self::unstake_from(&staker, &dominator, amount)?;
            Ok(().into())
        }

        // #[transactional]
        // #[pallet::weight(1_000_000_000_000)]
        // pub fn claim_shares(
        //     origin: OriginFor<T>,
        //     dominator: <T::Lookup as StaticLookup>::Source,
        //     token: TokenId<T>,
        // ) -> DispatchResultWithPostInfo {
        //     let signer = ensure_signed(origin)?;
        //     let dex = T::Lookup::lookup(dominator)?;
        //     let dominator =
        //         Dominators::<T>::try_get(&dex).map_err(|_| Error::<T>::DominatorNotFound)?;
        //     let staking =
        //         Stakings::<T>::try_get(&dex, &signer).map_err(|_| Error::<T>::InvalidStaking)?;
        //     let current_block = frame_system::Pallet::<T>::block_number();
        //     let current_season = (current_block - dominator.start_from) / T::SeasonDuration::get();
        //     let distribution = Distribution {
        //         dominator: dex.clone(),
        //         from_season: staking.from_season,
        //         to_season: current_season.into(),
        //         staking: staking.amount,
        //     };
        //     let step_into =
        //         Self::take_shares(&signer, &distribution).map_err(|_| Error::<T>::InvalidStatus)?;
        //     Stakings::<T>::try_mutate(&dex, &signer, |s| -> DispatchResult {
        //         let mut mutation = s.take().unwrap();
        //         mutation.start_season = step_into;
        //         s.replace(mutation);
        //         Ok(())
        //     })?;
        //     Ok(().into())
        // }

        #[pallet::weight(T::SelfWeightInfo::authorize_token())]
        pub fn authorize(
            origin: OriginFor<T>,
            dominator: <T::Lookup as StaticLookup>::Source,
            token_id: TokenId<T>,
            amount: Balance<T>,
        ) -> DispatchResultWithPostInfo {
            let fund_owner = ensure_signed(origin)?;
            let dex = T::Lookup::lookup(dominator)?;
            let dominator =
                Dominators::<T>::try_get(&dex).map_err(|_| Error::<T>::DominatorNotFound)?;
            ensure!(dominator.active, Error::<T>::DominatorInactive);
            ensure!(
                T::Asset::can_reserve(&token_id, &fund_owner, amount),
                Error::<T>::InsufficientBalance
            );
            let block_number = frame_system::Pallet::<T>::block_number();
            Self::reserve(
                constants::RESERVE_FOR_AUTHORIZING,
                fund_owner.clone(),
                token_id,
                amount,
                &fund_owner,
            )?;
            Receipts::<T>::insert(
                dex.clone(),
                fund_owner.clone(),
                Receipt::Authorize(token_id, amount, block_number),
            );
            Self::deposit_event(Event::TokenHosted(fund_owner, dex, token_id, amount));
            Ok(().into())
        }

        #[pallet::weight(T::SelfWeightInfo::revoke_token())]
        pub fn revoke(
            origin: OriginFor<T>,
            dominator: <T::Lookup as StaticLookup>::Source,
            token_id: TokenId<T>,
            amount: Balance<T>,
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
            ensure!(
                Self::has_reserved_on(fund_owner.clone(), token_id, amount, &dominator),
                Error::<T>::InsufficientBalance
            );
            let block_number = frame_system::Pallet::<T>::block_number();
            Receipts::<T>::insert(
                dominator.clone(),
                fund_owner.clone(),
                Receipt::Revoke(token_id, amount, block_number),
            );
            Self::deposit_event(Event::TokenRevoked(fund_owner, dominator, token_id, amount));
            Ok(().into())
        }
    }

    #[derive(Clone)]
    struct ClearingResult<T: Config> {
        pub users_mutation: Vec<TokenMutation<T::AccountId, Balance<T>>>,
        pub base_fee: Balance<T>,
        pub quote_fee: Balance<T>,
    }

    #[derive(Clone)]
    struct TokenMutation<AccountId, Balance> {
        pub who: AccountId,
        pub base_value: Balance,
        pub quote_value: Balance,
    }

    impl<T: Config> Pallet<T>
    where
        Balance<T>: Copy + From<u128> + Into<u128>,
        TokenId<T>: Copy + From<u32> + Into<u32>,
        T::BlockNumber: From<u32> + Into<u32>,
    {
        #[transactional]
        fn verify_and_update(
            dominator_id: &T::AccountId,
            dominator: &Dominator<TokenId<T>, Balance<T>, T::BlockNumber>,
            proof: Proof<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            let p0 = smt::CompiledMerkleProof(proof.proof_of_exists.clone());
            let old = proof
                .leaves
                .iter()
                .map(|v| (sha2_256(&v.key).into(), v.old_v.into()))
                .collect::<Vec<_>>();
            let r = p0
                .verify::<smt::sha256::Sha256Hasher>(&dominator.merkle_root.into(), old)
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
                    let cr = Self::verify_ask_limit(
                        price,
                        amount,
                        maker_fee,
                        taker_fee,
                        base,
                        quote,
                        dominator_id,
                        &proof.leaves,
                    )?;
                    for d in cr.users_mutation {
                        Self::clear(&d.who, dominator_id, base.into(), d.base_value)?;
                        Self::clear(&d.who, dominator_id, quote.into(), d.quote_value)?;
                    }
                    // TODO fee
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
                    let cr = Self::verify_bid_limit(
                        price,
                        amount,
                        maker_fee,
                        taker_fee,
                        base,
                        quote,
                        dominator_id,
                        &proof.leaves,
                    )?;
                    for d in cr.users_mutation {
                        Self::clear(&d.who, dominator_id, base.into(), d.base_value)?;
                        Self::clear(&d.who, dominator_id, quote.into(), d.quote_value)?;
                    }
                    // TODO fee
                }
                Command::Cancel(base, quote) => {
                    Self::verify_cancel(base.into(), quote.into(), &proof.user_id, &proof.leaves)?;
                }
                Command::TransferOut(currency, amount) => {
                    let (currency, amount) = (currency.into(), amount.into());
                    let r = Receipts::<T>::get(dominator_id, &proof.user_id)
                        .ok_or(Error::<T>::ReceiptNotExists)?;
                    let exists = match r {
                        Receipt::Revoke(id, value, _) => {
                            id.into() == currency && value.into() == amount
                        }
                        _ => false,
                    };
                    ensure!(exists, Error::<T>::ReceiptNotExists);
                    Self::verify_transfer_out(currency, amount, &proof.user_id, &proof.leaves)?;
                    Self::unreserve(
                        constants::RESERVE_FOR_AUTHORIZING,
                        proof.user_id.clone(),
                        currency.into(),
                        amount.into(),
                        &dominator_id,
                    )?;
                    Receipts::<T>::remove(dominator_id, &proof.user_id);
                }
                Command::TransferIn(currency, amount) => {
                    let (currency, amount) = (currency.into(), amount.into());
                    let r = Receipts::<T>::get(dominator_id, &proof.user_id)
                        .ok_or(Error::<T>::ReceiptNotExists)?;
                    let exists = match r {
                        Receipt::Authorize(id, value, _) => {
                            id.into() == currency && value.into() == amount
                        }
                        _ => false,
                    };
                    ensure!(exists, Error::<T>::ReceiptNotExists);
                    Self::verify_transfer_in(currency, amount, &proof.user_id, &proof.leaves)?;
                    Receipts::<T>::remove(dominator_id, &proof.user_id);
                }
                Command::RejectTransferOut(currency, amount) => {
                    let (currency, amount): (u32, u128) = (currency.into(), amount.into());
                    let r = Receipts::<T>::get(&dominator_id, &proof.user_id)
                        .ok_or(Error::<T>::ReceiptNotExists)?;
                    let exists = match r {
                        Receipt::Revoke(id, value, _) => {
                            currency == id.into() && value.into() == amount
                        }
                        _ => false,
                    };
                    ensure!(exists, Error::<T>::ReceiptNotExists);
                    Self::verify_reject_transfer_out(
                        currency,
                        amount,
                        &proof.user_id,
                        &proof.leaves,
                    )?;
                    Receipts::<T>::remove(&dominator_id, &proof.user_id);
                    // needn't step forward
                    return Ok(().into());
                }
            }
            Dominators::<T>::mutate(&dominator_id, |d| {
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
            dominator: &T::AccountId,
            leaves: &[MerkleLeaf],
        ) -> Result<ClearingResult<T>, DispatchError> {
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

            // let taker_paid = (base, tb_delta).into();
            ensure!(
                Self::has_reserved_on(taker_b_id.clone(), base.into(), tb_delta.into(), &dominator),
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
                    Self::has_reserved_on(
                        maker_q_id.clone(),
                        quote.into(),
                        quote_decr.into(),
                        &dominator
                    ),
                    Error::<T>::ProofsUnsatisfied
                );
                delta.push(TokenMutation {
                    who: maker_q_id,
                    base_value: mb1.into(),
                    quote_value: mq1.into(),
                });
                // delta.push((maker_b_id, (base, mb1).into(), (quote, mq1).into()));
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
            delta.push(TokenMutation {
                who: taker_b_id,
                base_value: (tba1 + tbf1).into(),
                quote_value: (tqa1 + tqf1).into(),
            });
            Ok(ClearingResult {
                users_mutation: delta,
                base_fee: base_charged.into(),
                quote_fee: quote_charged.into(),
            })
        }

        fn verify_bid_limit(
            _price: Price,
            amount: u128,
            maker_fee: Permill,
            taker_fee: Permill,
            base: u32,
            quote: u32,
            dominator: &T::AccountId,
            leaves: &[MerkleLeaf],
        ) -> Result<ClearingResult<T>, DispatchError> {
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
                Self::has_reserved_on(
                    taker_q_id.clone(),
                    quote.into(),
                    tq_delta.into(),
                    &dominator
                ),
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
                    Self::has_reserved_on(
                        maker_b_id.clone(),
                        base.into(),
                        base_decr.into(),
                        &dominator
                    ),
                    Error::<T>::ProofsUnsatisfied
                );
                delta.push(TokenMutation {
                    who: maker_b_id,
                    base_value: mb1.into(),
                    quote_value: mq1.into(),
                });
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
            delta.push(TokenMutation {
                who: taker_b_id,
                base_value: (tba1 + tbf1).into(),
                quote_value: (tqa1 + tqf1).into(),
            });
            Ok(ClearingResult {
                users_mutation: delta,
                base_fee: base_charged.into(),
                quote_fee: quote_charged.into(),
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

        fn has_reserved_on(
            who: T::AccountId,
            token_id: TokenId<T>,
            amount: Balance<T>,
            dominator: &T::AccountId,
        ) -> bool {
            Reserves::<T>::get(
                &(constants::RESERVE_FOR_AUTHORIZING, who, token_id),
                dominator,
            ) >= amount
        }

        #[transactional]
        fn clear(
            who: &T::AccountId,
            dominator: &T::AccountId,
            token_id: TokenId<T>,
            balance: Balance<T>,
        ) -> DispatchResult {
            Reserves::<T>::try_mutate(
                &(constants::RESERVE_FOR_AUTHORIZING, who.clone(), token_id),
                dominator,
                |reserved| -> DispatchResult {
                    T::Asset::try_mutate_account(&token_id, who, |b| -> DispatchResult {
                        b.1 = balance;
                        Ok(())
                    })?;
                    *reserved = balance;
                    Ok(())
                },
            )
        }

        #[transactional]
        fn take_shares(
            staker: &T::AccountId,
            distributions: &Distribution<T::AccountId, Balance<T>>,
        ) -> Result<Season, DispatchError> {
            if distributions.to_season == distributions.from_season {
                return Ok(distributions.from_season);
            }
            let mut shares: BTreeMap<TokenId<T>, u128> = BTreeMap::new();
            for season in distributions.from_season..distributions.to_season {
                let bonus = Bonuses::<T>::get(&distributions.dominator, season);
                if bonus.0.is_zero() {
                    continue;
                }
                let staking: u128 = distributions.staking.into();
                let total_staking: u128 = bonus.0.into();
                for (token_id, profit) in bonus.1.into_iter() {
                    let profit = profit.into();
                    let r: Perquintill = PerThing::from_rational(staking, total_staking);
                    shares
                        .entry(token_id)
                        .and_modify(|share| *share += r * profit)
                        .or_insert(r * profit);
                }
            }
            for (token_id, profit) in shares {
                T::Asset::try_mutate_account(&token_id, staker, |b| Ok(b.0 += profit.into()))?;
            }
            Ok(distributions.to_season)
        }

        #[transactional]
        fn reserve(
            reserve_id: u8,
            fund_owner: T::AccountId,
            token: TokenId<T>,
            value: Balance<T>,
            to: &T::AccountId,
        ) -> DispatchResult {
            if value.is_zero() {
                return Ok(());
            }
            Reserves::<T>::try_mutate(
                &(reserve_id, fund_owner.clone(), token),
                to,
                |ov| -> DispatchResult {
                    T::Asset::reserve(&token, &fund_owner, value)?;
                    *ov += value;
                    Ok(())
                },
            )
        }

        #[transactional]
        fn unreserve(
            reserve_id: u8,
            fund_owner: T::AccountId,
            token: TokenId<T>,
            value: Balance<T>,
            from: &T::AccountId,
        ) -> DispatchResult {
            if value.is_zero() {
                return Ok(());
            }
            Reserves::<T>::try_mutate(
                &(reserve_id, fund_owner.clone(), token),
                from,
                |ov| -> DispatchResult {
                    *ov = ov
                        .checked_sub(&value)
                        .ok_or(Error::<T>::InsufficientBalance)?;
                    T::Asset::unreserve(&token, &fund_owner, value)?;
                    Ok(())
                },
            )
        }

        #[transactional]
        fn stake_on(
            staker: &T::AccountId,
            dominator_id: &T::AccountId,
            amount: Balance<T>,
        ) -> DispatchResult {
            ensure!(
                amount >= T::MinimalStakingAmount::get(),
                Error::<T>::LittleStakingAmount
            );
            Dominators::<T>::try_mutate_exists(dominator_id, |exists| -> DispatchResult {
                ensure!(exists.is_some(), Error::<T>::DominatorNotFound);
                let mut dominator = exists.take().unwrap();
                Stakings::<T>::try_mutate(&dominator_id, &staker, |staking| -> DispatchResult {
                    Self::reserve(
                        constants::RESERVE_FOR_STAKING,
                        staker.clone(),
                        T::Asset::native_token_id(),
                        amount,
                        &dominator_id,
                    )?;
                    let current_season = Self::current_season(dominator.start_from);
                    let season_step_into = if staking.amount.is_zero() {
                        current_season + 1
                    } else {
                        // TODO put into pending distributions
                        current_season
                    };
                    staking.amount += amount;
                    staking.from_season = season_step_into;
                    Ok(())
                })?;
                dominator.staked += amount;
                dominator.active = dominator.staked >= T::DominatorOnlineThreshold::get();
                Self::deposit_event(Event::TaoStaked(
                    staker.clone(),
                    dominator_id.clone(),
                    amount,
                ));
                if dominator.active {
                    Self::deposit_event(Event::DominatorOnline(dominator_id.clone()));
                }
                exists.replace(dominator);
                Ok(())
            })
        }

        #[transactional]
        fn unstake_from(
            staker: &T::AccountId,
            dominator_id: &T::AccountId,
            amount: Balance<T>,
        ) -> DispatchResult {
            Dominators::<T>::try_mutate_exists(dominator_id, |exists| -> DispatchResult {
                ensure!(exists.is_some(), Error::<T>::DominatorNotFound);
                let mut dominator = exists.take().unwrap();
                let dominator_total_staking = dominator
                    .staked
                    .checked_sub(&amount)
                    .ok_or(Error::<T>::InsufficientBalance)?;
                Stakings::<T>::try_mutate(&dominator_id, &staker, |staking| -> DispatchResult {
                    let remain = staking
                        .amount
                        .checked_sub(&amount)
                        .ok_or(Error::<T>::InsufficientBalance)?;
                    ensure!(
                        remain.is_zero() || remain >= T::MinimalStakingAmount::get(),
                        Error::<T>::LittleStakingAmount
                    );
                    Self::unreserve(
                        constants::RESERVE_FOR_STAKING,
                        staker.clone(),
                        T::Asset::native_token_id(),
                        amount,
                        &dominator_id,
                    )?;
                    let current_season = Self::current_season(dominator.start_from);
                    let season_step_into = if remain.is_zero() {
                        current_season + 1
                    } else {
                        // TODO put into pending distributions
                        current_season
                    };
                    staking.amount = remain;
                    staking.from_season = season_step_into;
                    Ok(())
                })?;
                dominator.staked = dominator_total_staking;
                dominator.active = dominator.staked >= T::DominatorOnlineThreshold::get();
                Self::deposit_event(Event::TaoUnstaked(
                    staker.clone(),
                    dominator_id.clone(),
                    amount,
                ));
                if !dominator.active {
                    Self::deposit_event(Event::DominatorOffline(dominator_id.clone()));
                }
                exists.replace(dominator);
                Ok(())
            })
        }

        fn current_season(claim_at: T::BlockNumber) -> Season {
            let current_block = frame_system::Pallet::<T>::block_number();
            if current_block <= claim_at {
                return 0;
            }
            ((current_block - claim_at) / T::SeasonDuration::get()).into()
        }
    }
}
