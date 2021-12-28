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

//mod tests;

#[frame_support::pallet]
pub mod pallet {
    use codec::{Compact, Decode, Encode};
    use frame_support::{
        pallet_prelude::*,
        traits::{BalanceStatus, Currency, ReservableCurrency},
    };
    use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use sp_io::hashing::sha2_256;
    use sp_runtime::{Permill, Perquintill, RuntimeDebug, traits::StaticLookup};
    use sp_std::{convert::*, prelude::*, result::Result, vec::Vec};

    use fuso_support::traits::{ReservableToken, Token};

    pub type AmountOfCoin<T> = <T as pallet_balances::Config>::Balance;

    pub type AmountOfToken<T> = <T as pallet_fuso_token::Config>::Balance;

    pub type Amount = u128;

    pub type Price = (u128, Perquintill);

    pub type TokenId<T> = <T as pallet_fuso_token::Config>::TokenId;

    // pub type PositiveImbalanceOf<T> = <<T as Config>::Coin as Currency<
    //     <T as frame_system::Config>::AccountId,
    // >>::PositiveImbalance;

    // pub type NegativeImbalanceOf<T> = <<T as Config>::Coin as Currency<
    //     <T as frame_system::Config>::AccountId,
    // >>::NegativeImbalance;

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
    pub struct Dominator<Balance, BlockNumber> {
        pub merkle_root: [u8; 32],
        pub pledged: Balance,
        pub sequence: (u64, BlockNumber),
    }

    #[pallet::config]
    pub trait Config:
    frame_system::Config + pallet_balances::Config + pallet_fuso_token::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        // type Coin: ReservableCurrency<Self::AccountId>;

        // type Token: ReservableToken<Self::AccountId>;
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
        Dominator<AmountOfCoin<T>, T::BlockNumber>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        DominatorClaimed(T::AccountId, AmountOfCoin<T>),
        CoinHosted(T::AccountId, T::AccountId, AmountOfCoin<T>),
        TokenHosted(T::AccountId, T::AccountId, TokenId<T>, AmountOfToken<T>),
        CoinRevoked(T::AccountId, T::AccountId, AmountOfCoin<T>),
        TokenRevoked(T::AccountId, T::AccountId, TokenId<T>, AmountOfToken<T>),
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
        DominatorBanned,
        PledgeUnsatisfied,
        DominatorClosing,
        InsufficientBalance,
        InsufficientStashAccount,
        InvalidStatus,
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    // TODO
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T>
        where
            AmountOfCoin<T>: Copy + From<u128>,
            AmountOfToken<T>: Copy + From<u128>,
            u128: From<AmountOfToken<T>> + From<AmountOfCoin<T>>,
            u32: From<TokenId<T>>
    {
        // TODO pledge amount config?
        /// Initialize an empty sparse merkle tree with sequence 0 for a new dominator.
        #[pallet::weight(1_000_000_000_000)]
        pub fn claim_dominator(
            origin: OriginFor<T>,
            pledged: AmountOfCoin<T>,
        ) -> DispatchResultWithPostInfo {
            let dominator = ensure_signed(origin)?;
            ensure!(
                !Dominators::<T>::contains_key(&dominator),
                Error::<T>::DominatorAlreadyExists
            );
            <pallet_balances::Pallet<T>>::reserve(&dominator, pledged)?;
            <Dominators<T>>::insert(
                &dominator,
                Dominator {
                    pledged: pledged,
                    sequence: (0, frame_system::Pallet::<T>::block_number()),
                    merkle_root: Default::default(),
                },
            );
            Self::deposit_event(Event::DominatorClaimed(dominator, pledged));
            Ok(().into())
        }

        // TODO 0 gas if OK, non-zero gas otherwise
        #[pallet::weight(100_000)]
        pub fn verify(
            origin: OriginFor<T>,
            proofs: Vec<Proof<T::AccountId>>,
        ) -> DispatchResultWithPostInfo {
            let dominator = ensure_signed(origin)?;
            let ex =
                Dominators::<T>::try_get(&dominator).map_err(|_| Error::<T>::DominatorNotFound)?;
            for proof in proofs.into_iter() {
                Self::verify_and_update(&dominator, &ex, proof)?;
            }
            Ok(().into())
        }

        #[pallet::weight(1_000_000_000_000)]
        pub fn authorize_coin(
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
            ensure!(
                pallet_balances::Pallet::<T>::can_reserve(&fund_owner, amount),
                Error::<T>::InsufficientBalance
            );
            let block_number = frame_system::Pallet::<T>::block_number();
            pallet_balances::Pallet::<T>::reserve(&fund_owner, amount)?;
            Receipts::<T>::insert(
                dominator.clone(),
                fund_owner.clone(),
                Receipt::Authorize(UniBalance::Coin(amount.into()), block_number),
            );
            Self::deposit_event(Event::CoinHosted(fund_owner, dominator, amount));
            Ok(().into())
        }

        #[pallet::weight(1_000_000_000_000)]
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

        #[pallet::weight(1_000_000_000_000)]
        pub fn authorize_token(
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
            ensure!(
                pallet_fuso_token::Pallet::<T>::can_reserve(&token_id, &fund_owner, amount),
                Error::<T>::InsufficientBalance
            );
            let block_number = frame_system::Pallet::<T>::block_number();
            pallet_fuso_token::Pallet::<T>::reserve(&token_id, &fund_owner, amount)?;
            Receipts::<T>::insert(
                dominator.clone(),
                fund_owner.clone(),
                Receipt::Authorize(UniBalance::Token(token_id.into(), amount.into()), block_number),
            );
            Self::deposit_event(Event::TokenHosted(fund_owner, dominator, token_id, amount));
            Ok(().into())
        }

        #[pallet::weight(1_000_000_000_000)]
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

    impl TryFrom<(u32, u128)> for UniBalance {
        type Error = DispatchError;
        fn try_from((token, value): (u32, u128)) -> Result<Self, Self::Error> {
            match token.clone() {
                0 => {
                    Ok(UniBalance::Coin(value))
                }
                id => {
                    Ok(UniBalance::Token(token, value))
                }
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

    impl<T: Config> Pallet<T> where AmountOfCoin<T>: Copy + From<u128>,
                                    AmountOfToken<T>: Copy + From<u128>,
                                    TokenId<T>: From<u32> {
        fn verify_and_update(
            dominator: &T::AccountId,
            ex: &Dominator<AmountOfCoin<T>, T::BlockNumber>,
            proof: Proof<T::AccountId>,
        ) -> DispatchResultWithPostInfo
            where AmountOfCoin<T>: Copy + From<u128>,
                  AmountOfToken<T>: Copy + From<u128>,
                  TokenId<T>: From<u32> {
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
                    Self::unreserve(&proof.user_id, balance);
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
                        Receipt::Revoke(v, index) => balance == v,
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
            price: Price,
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
            let mut delta = vec![];
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
            price: Price,
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
            // FIXME ceil
            // let frozen_vol = price
            //     .0
            //     .checked_mul(amount)
            //     .ok_or_else(|| Error::<T>::ProofsUnsatisfied)?
            //     .checked_add(price.1.mul_ceil(amount))
            //     .ok_or_else(|| Error::<T>::ProofsUnsatisfied)?;
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
            let mut delta = vec![];
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
            let (a0, f0) = leaves[0].split_old_to_u128();
            // let (a1, f1) = leaves[0].split_new_to_u128();
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
                    pallet_fuso_token::Pallet::<T>::reserved_balance(&(*id).into(), who) >= (*value).into()
                }
            }
        }

        fn mutate_to(who: &T::AccountId, balance: &UniBalance) {
            match balance {
                UniBalance::Coin(value) => {
                    pallet_balances::Pallet::<T>::mutate_account(who, |a| a.reserved = (*value).into());
                }
                UniBalance::Token(id, value) => {
                    pallet_fuso_token::Pallet::<T>::mutate_account(&(*id).into(), who, |a| {
                        a.reserved = (*value).into()
                    });
                }
            }
        }

        fn unreserve(who: &T::AccountId, balance: UniBalance) {
            match balance {
                UniBalance::Coin(value) => {
                    pallet_balances::Pallet::<T>::unreserve(who, value.into());
                }
                UniBalance::Token(id, value) => {
                    pallet_fuso_token::Pallet::<T>::unreserve(&id.into(), who, value.into());
                }
            }
        }

        fn charge(who: &T::AccountId, balance: &UniBalance) where AmountOfCoin<T>: Copy + From<u128>,
                                                                  TokenId<T>: From<u32>,
                                                                  AmountOfToken<T>: Copy + From<u128> {
            match balance {
                UniBalance::Coin(value) => {
                    pallet_balances::Pallet::<T>::mutate_account(who, |a| a.reserved += (*value).into());
                }
                UniBalance::Token(id, value) => {
                    pallet_fuso_token::Pallet::<T>::mutate_account(&(*id).into(), who, |a| {
                        a.reserved += (*value).into()
                    });
                }
            }
        }
    }
}
