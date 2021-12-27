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

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use ascii::AsciiStr;
	use codec::Codec;
	use codec::{Decode, Encode};
	use frame_support::pallet_prelude::*;
	use frame_support::traits::{BalanceStatus, Currency};
	use frame_system::pallet_prelude::*;
	use sp_std::{fmt::Debug, vec::Vec};

	use sp_runtime::traits::{
		AtLeast32BitUnsigned, CheckedAdd, CheckedSub, MaybeSerializeDeserialize, Member, One,
		StaticLookup, Zero,
	};
	use sp_runtime::DispatchResult;

	use fuso_support::traits::{ReservableToken, Token};

	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	// pub type UniBalance<T> = <T as pallet_balances::Config>::Balance;

	#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, Debug)]
	pub struct TokenAccountData<Balance> {
		pub free: Balance,
		pub reserved: Balance,
	}

	#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, Debug)]
	pub struct TokenInfo<Balance> {
		pub total: Balance,
		pub symbol: Vec<u8>,
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type TokenId: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ Codec
			+ Debug
			+ MaybeSerializeDeserialize;

		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Codec
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ Debug;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::error]
	pub enum Error<T> {
		AmountZero,
		BalanceLow,
		BalanceZero,
		InvalidTokenName,
		InvalidToken,
		InsufficientBalance,
		Overflow,
	}

	#[pallet::storage]
	#[pallet::getter(fn get_token_balance)]
	pub type Balances<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		(T::TokenId, T::AccountId),
		TokenAccountData<T::Balance>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn get_token_info)]
	pub type Tokens<T: Config> =
		StorageMap<_, Twox64Concat, T::TokenId, TokenInfo<T::Balance>, OptionQuery>;

	#[pallet::type_value]
	pub fn DefaultNextTokenId<T: Config>() -> T::TokenId {
		One::one()
	}

	#[pallet::storage]
	#[pallet::getter(fn next_token_id)]
	pub type NextTokenId<T: Config> =
		StorageValue<_, T::TokenId, ValueQuery, DefaultNextTokenId<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		TokenIssued(T::TokenId, T::AccountId, T::Balance),
		TokenTransfered(T::TokenId, T::AccountId, T::AccountId, T::Balance),
		TokenReserved(T::TokenId, T::AccountId, T::Balance),
		TokenUnreserved(T::TokenId, T::AccountId, T::Balance),
		TokenBurned(T::TokenId, T::AccountId, T::Balance),
		TokenRepatriated(T::TokenId, T::AccountId, T::AccountId, T::Balance),
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn issue(
			origin: OriginFor<T>,
			total: T::Balance,
			symbol: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			let origin = ensure_signed(origin)?;
			ensure!(!total.is_zero(), Error::<T>::AmountZero);
			let name = AsciiStr::from_ascii(&symbol);
			ensure!(name.is_ok(), Error::<T>::InvalidTokenName);
			let name = name.unwrap();
			ensure!(
				name.len() >= 2 && name.len() <= 5,
				Error::<T>::InvalidTokenName
			);
			let id = Self::next_token_id();
			NextTokenId::<T>::mutate(|id| *id += One::one());
			Balances::<T>::insert(
				(id, &origin),
				TokenAccountData {
					free: total,
					reserved: Zero::zero(),
				},
			);
			Tokens::<T>::insert(
				id,
				TokenInfo {
					total: total,
					symbol: symbol,
				},
			);
			Self::deposit_event(Event::TokenIssued(id, origin, total));
			Ok(().into())
		}

		#[pallet::weight(0)]
		pub fn transfer(
			origin: OriginFor<T>,
			token: T::TokenId,
			target: <T::Lookup as StaticLookup>::Source,
			amount: T::Balance,
		) -> DispatchResultWithPostInfo {
			let origin = ensure_signed(origin)?;
			ensure!(!amount.is_zero(), Error::<T>::AmountZero);
			let target = T::Lookup::lookup(target)?;
			<Balances<T>>::try_mutate_exists((&token, &origin), |from| -> DispatchResult {
				ensure!(from.is_some(), Error::<T>::BalanceZero);
				let mut account = from.take().unwrap();
				account.free = account
					.free
					.checked_sub(&amount)
					.ok_or(Error::<T>::InsufficientBalance)?;
				match account.free == Zero::zero() && account.reserved == Zero::zero() {
					true => {}
					false => {
						from.replace(account);
					}
				}
				<Balances<T>>::try_mutate_exists((&token, &target), |to| -> DispatchResult {
					let mut account = to.take().unwrap_or(TokenAccountData {
						free: Zero::zero(),
						reserved: Zero::zero(),
					});
					account.free = account
						.free
						.checked_add(&amount)
						.ok_or(Error::<T>::Overflow)?;
					to.replace(account);
					Ok(())
				})?;
				Ok(())
			})?;
			Self::deposit_event(Event::TokenTransfered(token, origin, target, amount));
			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn mutate_account<R>(
			token: &T::TokenId,
			who: &T::AccountId,
			f: impl FnOnce(&mut TokenAccountData<T::Balance>) -> R,
		) -> Result<R, ()> {
			Balances::<T>::try_mutate((token, who), |account| -> Result<R, ()> { Ok(f(account)) })
		}
	}

	impl<T: Config> Token<T::AccountId> for Pallet<T> {
		type Balance = T::Balance;
		type TokenId = T::TokenId;

		fn free_balance(token: &T::TokenId, who: &T::AccountId) -> T::Balance {
			Self::get_token_balance((token, who)).free
		}

		fn total_issuance(token: &T::TokenId) -> T::Balance {
			Self::get_token_info(token).unwrap_or_default().total
		}
	}

	impl<T: Config> ReservableToken<T::AccountId> for Pallet<T> {
		fn can_reserve(token: &T::TokenId, who: &T::AccountId, value: T::Balance) -> bool {
			if value.is_zero() {
				return true;
			}
			if !<Balances<T>>::contains_key((token, who)) {
				return false;
			}
			Self::free_balance(token, who).checked_sub(&value).is_some()
		}

		fn reserve(
			token: &T::TokenId,
			who: &T::AccountId,
			value: T::Balance,
		) -> sp_std::result::Result<T::Balance, DispatchError> {
			if value.is_zero() {
				return Ok(value);
			}
			<Balances<T>>::try_mutate_exists(
				(token, who),
				|account| -> sp_std::result::Result<T::Balance, DispatchError> {
					ensure!(account.is_some(), Error::<T>::BalanceZero);
					let account = account.as_mut().ok_or(Error::<T>::BalanceZero)?;
					account.free = account
						.free
						.checked_sub(&value)
						.ok_or(Error::<T>::InsufficientBalance)?;
					account.reserved = account
						.reserved
						.checked_add(&value)
						.ok_or(Error::<T>::Overflow)?;
					Self::deposit_event(Event::TokenReserved(token.clone(), who.clone(), value));
					Ok(value)
				},
			)
		}

		fn unreserve(
			token: &T::TokenId,
			who: &T::AccountId,
			value: T::Balance,
		) -> sp_std::result::Result<T::Balance, DispatchError> {
			if value.is_zero() {
				return Ok(value);
			}
			<Balances<T>>::try_mutate_exists(
				(token, who),
				|account| -> sp_std::result::Result<T::Balance, DispatchError> {
					ensure!(account.is_some(), Error::<T>::BalanceZero);
					let account = account.as_mut().ok_or(Error::<T>::BalanceZero)?;
					account.reserved = account
						.reserved
						.checked_sub(&value)
						.ok_or(Error::<T>::InsufficientBalance)?;
					account.free = account
						.free
						.checked_add(&value)
						.ok_or(Error::<T>::Overflow)?;
					Self::deposit_event(Event::TokenUnreserved(token.clone(), who.clone(), value));
					Ok(value)
				},
			)
			// Self::deposit_event(Event::TokenUnreserved(token.clone(), who.clone(), value));
			// r
		}

		fn reserved_balance(token: &Self::TokenId, who: &T::AccountId) -> Self::Balance {
			<Balances<T>>::get((token, who)).reserved
		}

		fn repatriate_reserved(
			token: &T::TokenId,
			slashed: &T::AccountId,
			beneficiary: &T::AccountId,
			value: T::Balance,
			status: BalanceStatus,
		) -> sp_std::result::Result<Self::Balance, DispatchError> {
			if slashed == beneficiary {
				return match status {
					BalanceStatus::Free => Self::unreserve(token, slashed, value),
					BalanceStatus::Reserved => Self::reserve(token, slashed, value),
				};
			}
			<Balances<T>>::try_mutate_exists((token, slashed), |from| -> DispatchResult {
				ensure!(from.is_some(), Error::<T>::BalanceZero);
				let mut account = from.take().unwrap();
				account.reserved = account
					.reserved
					.checked_sub(&value)
					.ok_or(Error::<T>::InsufficientBalance)?;
				// drop the `from` if dead
				match account.reserved == Zero::zero() && account.free == Zero::zero() {
					true => {}
					false => {
						from.replace(account);
					}
				}
				<Balances<T>>::try_mutate_exists((token, beneficiary), |to| -> DispatchResult {
					let mut account = to.take().unwrap_or(TokenAccountData {
						free: Zero::zero(),
						reserved: Zero::zero(),
					});
					match status {
						BalanceStatus::Free => {
							account.free = account
								.free
								.checked_add(&value)
								.ok_or(Error::<T>::Overflow)?;
						}
						BalanceStatus::Reserved => {
							account.reserved = account
								.reserved
								.checked_add(&value)
								.ok_or(Error::<T>::Overflow)?;
						}
					}
					to.replace(account);
					Ok(())
				})?;
				Ok(())
			})?;
			Self::deposit_event(Event::TokenRepatriated(
				token.clone(),
				slashed.clone(),
				beneficiary.clone(),
				value,
			));
			Ok(value)
		}
	}
}
