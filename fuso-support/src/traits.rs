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

use codec::{Codec, FullCodec, MaxEncodedLen};
use frame_support::{traits::BalanceStatus, Parameter};
use sp_runtime::traits::{AtLeast32BitUnsigned, MaybeDisplay, MaybeSerializeDeserialize, Member};
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::fmt::Debug;
use sp_std::vec::Vec;

pub use crate::external_chain::ExternalChainAddress;

pub trait Token<AccountId> {
    type Balance: Member
        + Parameter
        + AtLeast32BitUnsigned
        + Default
        + Copy
        + Codec
        + Debug
        + MaybeSerializeDeserialize;

    type TokenId: Member
        + Parameter
        + AtLeast32BitUnsigned
        + Default
        + Copy
        + Codec
        + Debug
        + MaybeSerializeDeserialize;

    fn free_balance(token: &Self::TokenId, who: &AccountId) -> Self::Balance;

    fn total_issuance(token: &Self::TokenId) -> Self::Balance;
}

pub trait ReservableToken<AccountId>: Token<AccountId> {
    /// Same result as `reserve(who, value)` (but without the side-effects) assuming there
    /// are no balance changes in the meantime.
    fn can_reserve(token: &Self::TokenId, who: &AccountId, value: Self::Balance) -> bool;

    /// The amount of the balance of a given account that is externally reserved; this can still get
    /// slashed, but gets slashed last of all.
    ///
    /// This balance is a 'reserve' balance that other subsystems use in order to set aside tokens
    /// that are still 'owned' by the account holder, but which are suspendable.
    ///
    /// When this balance falls below the value of `ExistentialDeposit`, then this 'reserve account'
    /// is deleted: specifically, `ReservedBalance`.
    ///
    /// `system::AccountNonce` is also deleted if `FreeBalance` is also zero (it also gets
    /// collapsed to zero if it ever becomes less than `ExistentialDeposit`.
    fn reserved_balance(token: &Self::TokenId, who: &AccountId) -> Self::Balance;

    /// Moves `value` from balance to reserved balance.
    ///
    /// If the free balance is lower than `value`, then no funds will be moved and an `Err` will
    /// be returned to notify of this.
    fn reserve(
        token: &Self::TokenId,
        who: &AccountId,
        value: Self::Balance,
    ) -> sp_std::result::Result<Self::Balance, DispatchError>;

    /// Moves up to `value` from reserved balance to free balance.
    fn unreserve(
        token: &Self::TokenId,
        who: &AccountId,
        value: Self::Balance,
    ) -> sp_std::result::Result<Self::Balance, DispatchError>;

    /// Moves up to `value` from reserved balance of account `slashed` to balance of account
    /// `beneficiary`. `beneficiary` must exist for this to succeed. If it does not, `Err` will be
    /// returned. Funds will be placed in either the `free` balance or the `reserved` balance,
    /// depending on the `status`.
    ///
    /// As much funds up to `value` will be deducted as possible. If this is less than `value`,
    /// then `Ok(non_zero)` will be returned.
    fn repatriate_reserved(
        token: &Self::TokenId,
        slashed: &AccountId,
        beneficiary: &AccountId,
        value: Self::Balance,
        status: BalanceStatus,
    ) -> sp_std::result::Result<Self::Balance, DispatchError>;
}

pub trait NamedReservableToken<AccountId>: Token<AccountId> {
    type ReserveIdentifier;

    /// Same result as `reserve(who, value)` (but without the side-effects) assuming there
    /// are no balance changes in the meantime.
    fn can_reserve_named(
        id: &Self::ReserveIdentifier,
        token: &Self::TokenId,
        who: &AccountId,
        value: Self::Balance,
    ) -> bool;

    /// The amount of the balance of a given account that is externally reserved; this can still get
    /// slashed, but gets slashed last of all.
    ///
    /// This balance is a 'reserve' balance that other subsystems use in order to set aside tokens
    /// that are still 'owned' by the account holder, but which are suspendable.
    ///
    /// When this balance falls below the value of `ExistentialDeposit`, then this 'reserve account'
    /// is deleted: specifically, `ReservedBalance`.
    ///
    /// `system::AccountNonce` is also deleted if `FreeBalance` is also zero (it also gets
    /// collapsed to zero if it ever becomes less than `ExistentialDeposit`.
    fn reserved_balance_named(
        id: &Self::ReserveIdentifier,
        token: &Self::TokenId,
        who: &AccountId,
    ) -> Self::Balance;

    /// Moves `value` from balance to reserved balance.
    ///
    /// If the free balance is lower than `value`, then no funds will be moved and an `Err` will
    /// be returned to notify of this.
    fn reserve_named(
        id: &Self::ReserveIdentifier,
        token: &Self::TokenId,
        who: &AccountId,
        value: Self::Balance,
    ) -> sp_std::result::Result<Self::Balance, DispatchError>;

    /// Moves up to `value` from reserved balance to free balance.
    fn unreserve_named(
        id: &Self::ReserveIdentifier,
        token: &Self::TokenId,
        who: &AccountId,
        value: Self::Balance,
    ) -> sp_std::result::Result<Self::Balance, DispatchError>;

    /// Moves up to `value` from reserved balance of account `slashed` to balance of account
    /// `beneficiary`. `beneficiary` must exist for this to succeed. If it does not, `Err` will be
    /// returned. Funds will be placed in either the `free` balance or the `reserved` balance,
    /// depending on the `status`.
    ///
    /// As much funds up to `value` will be deducted as possible. If this is less than `value`,
    /// then `Ok(non_zero)` will be returned.
    fn repatriate_reserved_named(
        id: &Self::ReserveIdentifier,
        token: &Self::TokenId,
        slashed: &AccountId,
        beneficiary: &AccountId,
        value: Self::Balance,
        status: BalanceStatus,
    ) -> sp_std::result::Result<Self::Balance, DispatchError>;
}

pub trait ProofOfSecurity<AccountId> {
    type ExternalChainAddress: Parameter
        + Member
        + MaybeSerializeDeserialize
        + Debug
        + MaybeDisplay
        + Ord
        + Default;

    fn pos_enabled() -> bool;
}

pub trait Referendum<BlockNumber, Index, Members> {
    fn proposal(start_include: BlockNumber) -> Index;

    fn get_round() -> Index;

    fn get_result(index: Index) -> Option<Vec<Members>>;
}

pub type ExternalTransactionId = u64;

pub trait Inspector<T: frame_system::Config> {
    type ExternalChainBalance: AtLeast32BitUnsigned
        + FullCodec
        + Parameter
        + Member
        + Copy
        + MaybeDisplay
        + MaybeSerializeDeserialize
        + Default
        + Debug;

    type ExternalChainTxHash: Clone
        + Parameter
        + Member
        + MaybeSerializeDeserialize
        + Debug
        + MaybeDisplay
        + Ord
        + Default;

    fn expect_transaction(
        to: ExternalChainAddress,
        memo: Vec<u8>,
        amount: Self::ExternalChainBalance,
    );

    fn approve(
        from: ExternalChainAddress,
        to: ExternalChainAddress,
        memo: Vec<u8>,
        amount: Self::ExternalChainBalance,
        external_transaction_hash: Self::ExternalChainTxHash,
    );
}
