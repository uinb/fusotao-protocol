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
pub mod weights;

#[frame_support::pallet]
pub mod pallet {
    use ascii::AsciiStr;
    use codec::Codec;
    use codec::{Decode, Encode};
    use frame_support::pallet_prelude::*;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::tokens::{fungibles, DepositConsequence, WithdrawConsequence};
    use frame_support::traits::BalanceStatus;
    use frame_system::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use pallet_octopus_support::traits::AssetIdAndNameProvider;
    use scale_info::TypeInfo;
    use sp_runtime::traits::{
        AtLeast32BitUnsigned, CheckedAdd, CheckedSub, MaybeSerializeDeserialize, Member, One,
        StaticLookup, Zero,
    };
    use sp_runtime::DispatchResult;
    use sp_std::{fmt::Debug, vec::Vec};

    use fuso_support::traits::{ReservableToken, Token};

    #[derive(Encode, Decode, Clone, PartialEq, Eq, Default, TypeInfo, Debug)]
    pub struct TokenAccountData<Balance> {
        pub free: Balance,
        pub reserved: Balance,
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, Default, Debug, TypeInfo)]
    pub struct TokenInfo<Balance> {
        pub total: Balance,
        pub symbol: Vec<u8>,
    }

	/// Store named reserved balance.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct ReserveData<ReserveIdentifier, Balance> {
		/// The identifier for the named reserve.
		pub id: ReserveIdentifier,
		/// The amount of the named reserve.
		pub amount: Balance,
	}

	#[derive(Encode, Decode, Clone, PartialEq, Eq, Debug, TypeInfo)]
	pub enum XToken<TokenId, Balance> {
		//symbol, nep141 contract name
		NEP141(TokenId, Vec<u8>, Vec<u8>, Balance, u8),
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

		#[pallet::constant]
		type MaxReserves: Get<u32>;

		type ReserveIdentifier: Parameter + Member + MaxEncodedLen + Ord + Copy;

		type WeightInfo: WeightInfo;
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
		TooManyReserves,
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

    #[pallet::storage]
    #[pallet::getter(fn get_token_by_id)]
    pub type TokenById<T: Config> =
        StorageMap<_, Twox64Concat, T::TokenId, XToken<T::TokenId, T::Balance>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn get_token_by_name)]
    pub type TokenByName<T: Config> =
        StorageMap<_, Twox64Concat, Vec<u8>, XToken<T::TokenId, T::Balance>, OptionQuery>;

	/// Named reserves on some account balances.
	#[pallet::storage]
	#[pallet::getter(fn reserves)]
	pub type Reserves<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		(T::AccountId, T::TokenId),
		BoundedVec<ReserveData<T::ReserveIdentifier, T::Balance>, T::MaxReserves>,
		ValueQuery,
	>;

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

		#[pallet::weight(T::WeightInfo::transfer())]
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


	impl<T: Config> AssetIdAndNameProvider<u32> for Pallet<T>{
		type Err = ();

		fn try_get_asset_id(name: impl AsRef<[u8]>) -> Result<u32, Self::Err> {
			unimplemented!()
		}

		fn try_get_asset_name(asset_id: u32) -> Result<Vec<u8>, Self::Err> {
			unimplemented!()
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

        fn create_token(name: &[u8]) -> T::TokenId {
            let token_id = Self::next_token_id();
            let name = name.as_ref().to_vec();
            let token = XToken::<T::TokenId, T::Balance>::NEP141(
                token_id,
                name.clone(),
                name.clone(),
                Zero::zero(),
                18u8,
            );
            TokenByName::<T>::insert(name, token.clone());
            TokenById::<T>::insert(token_id, token.clone());
            token_id
        }

		pub fn do_mint(
			token: T::TokenId,
			beneficiary: &T::AccountId,
			amount: T::Balance,
			maybe_check_issuer: Option<T::AccountId>,
		) -> DispatchResult {
			<Balances<T>>::try_mutate_exists((&token, beneficiary), |to| -> DispatchResult {
				let mut account = to.take().unwrap_or(TokenAccountData {
					free: Zero::zero(),
					reserved: Zero::zero(),
				});
				account.free = account
					.free
					.checked_add(&amount)
					.ok_or(Error::<T>::Overflow)?;
				to.replace(account);
				<Tokens<T>>::try_mutate_exists(&token, |token_info| -> DispatchResult {
					ensure!(token_info.is_some(), Error::<T>::BalanceZero);
					let mut info  = token_info.take().unwrap();
					info.total = info.total.checked_add(&amount).ok_or(Error::<T>::InsufficientBalance)?;
					token_info.replace(info);
					Ok(())
				});

                Ok(())
            })?;
            Self::deposit_event(Event::TokenIssued(token, beneficiary.clone(), amount));
            Ok(())
        }

		pub fn do_burn(
			token: T::TokenId,
			target: &T::AccountId,
			amount: T::Balance,
			maybe_check_admin: Option<T::AccountId>,
		) -> Result<T::Balance, DispatchError> {
			ensure!(!amount.is_zero(), Error::<T>::AmountZero);
			<Balances<T>>::try_mutate_exists((&token, target), |from| -> DispatchResult {
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
				<Tokens<T>>::try_mutate_exists(&token, |token_info| -> DispatchResult {
					ensure!(token_info.is_some(), Error::<T>::BalanceZero);
					let mut info  = token_info.take().unwrap();
					info.total = info.total.checked_sub(&amount).ok_or(Error::<T>::InsufficientBalance)?;
					token_info.replace(info);
					Ok(())
				});
				Ok(())
			})?;
			Self::deposit_event(Event::TokenBurned(token, target.clone(), amount));
			Ok(T::Balance::default())
		}
	}

    impl<T: Config> fungibles::Inspect<T::AccountId> for Pallet<T> {
        type AssetId = T::TokenId;
        type Balance = T::Balance;

        fn total_issuance(asset: Self::AssetId) -> Self::Balance {
            Self::Balance::default()
        }

        fn minimum_balance(asset: Self::AssetId) -> Self::Balance {
            Self::Balance::default()
        }

        fn balance(asset: Self::AssetId, who: &T::AccountId) -> Self::Balance {
            Self::Balance::default()
        }

        fn reducible_balance(
            asset: Self::AssetId,
            who: &T::AccountId,
            keep_alive: bool,
        ) -> Self::Balance {
            Self::Balance::default()
        }

        fn can_deposit(
            asset: Self::AssetId,
            who: &T::AccountId,
            amount: Self::Balance,
        ) -> DepositConsequence {
            DepositConsequence::Success
        }

        fn can_withdraw(
            asset: Self::AssetId,
            who: &T::AccountId,
            amount: Self::Balance,
        ) -> WithdrawConsequence<Self::Balance> {
            WithdrawConsequence::Success
        }
    }

    impl<T: Config> fungibles::Mutate<T::AccountId> for Pallet<T> {
        fn mint_into(
            asset: Self::AssetId,
            who: &T::AccountId,
            amount: Self::Balance,
        ) -> DispatchResult {
            Self::do_mint(asset, who, amount, None)
        }

        fn burn_from(
            asset: Self::AssetId,
            who: &T::AccountId,
            amount: Self::Balance,
        ) -> Result<Self::Balance, DispatchError> {
            Self::do_burn(asset, who, amount, None)
        }

        fn slash(
            asset: Self::AssetId,
            who: &T::AccountId,
            amount: Self::Balance,
        ) -> Result<Self::Balance, DispatchError> {
            Self::do_burn(asset, who, amount, None)
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

	/*impl<T: Config> AssetIdAndNameProvider<T::TokenId> for Pallet<T> {
		type Err = ();

        fn try_get_asset_id(name: impl AsRef<[u8]>) -> Result<<T as Config>::TokenId, Self::Err> {
            let name = name.as_ref();
            let tokenResult = Self::get_token_by_name(name.clone().to_vec());
            let token_id = match tokenResult {
                Some(XToken::NEP141(token_id, _, _, _, _)) => token_id,
                _ => Self::create_token(name),
            };
            Ok(token_id)
        }

		fn try_get_asset_name(token_id: <T as Config>::TokenId) -> Result<Vec<u8>, Self::Err> {
			let tokenResult = Self::get_token_by_id(token_id);
			match tokenResult {
				Some(XToken::NEP141(_, _, name, _, _)) => Ok(name),
				_ => Err(())
			}
		}
	}*/

	impl<T: Config> NamedReservableToken<T::AccountId> for Pallet<T> {
		type ReserveIdentifier = T::ReserveIdentifier;

		fn can_reserve_named(id: &Self::ReserveIdentifier,
							 token: &T::TokenId,
							 who: &T::AccountId,
							 value: T::Balance) -> bool {
			if value.is_zero() {
				return true;
			}
			if !<Balances<T>>::contains_key((token, who)) {
				return false;
			}
			Self::free_balance(token, who).checked_sub(&value).is_some()
		}

		fn reserve_named(
			id: &Self::ReserveIdentifier,
			token: &T::TokenId,
			who: &T::AccountId,
			value: T::Balance,
		) -> sp_std::result::Result<T::Balance, DispatchError> {
			if value.is_zero() {
				return Ok(value);
			}

			let d = Reserves::<T>::try_mutate((who, token), |reserves| -> DispatchResult {
				match reserves.binary_search_by_key(id, |data| data.id) {
					Ok(index) => {
						// this add can't overflow but just to be defensive.
						reserves[index].amount = reserves[index].amount.saturating_add(value);
					},
					Err(index) => {
						reserves
							.try_insert(index, ReserveData { id: id.clone(), amount: value })
							.map_err(|_| Error::<T>::TooManyReserves)?;
					},
				};
				<Self as ReservableToken<_>>::reserve(token, who, value)?;
				Ok(())
			});
			Ok(value)
		}

		fn unreserve_named(
			id: &Self::ReserveIdentifier,
			token: &T::TokenId,
			who: &T::AccountId,
			value: T::Balance,
		) -> sp_std::result::Result<T::Balance, DispatchError> {

			if value.is_zero() {
				return Ok(Zero::zero())
			}

			 Reserves::<T>::mutate_exists((who, token), |maybe_reserves| -> Result<T::Balance, DispatchError> {
				if let Some(reserves) = maybe_reserves.as_mut() {
					match reserves.binary_search_by_key(id, |data| data.id) {
						Ok(index) => {
							let to_change = cmp::min(reserves[index].amount, value);

							let remain = <Self as ReservableToken<_>>::unreserve(token, who, to_change)?;

							// remain should always be zero but just to be defensive here
							let actual = to_change.saturating_sub(remain);

							// `actual <= to_change` and `to_change <= amount`; qed;
							reserves[index].amount -= actual;

							if reserves[index].amount.is_zero() {
								if reserves.len() == 1 {
									// no more named reserves
									*maybe_reserves = None;
								} else {
									// remove this named reserve
									reserves.remove(index);
								}
							}

							Ok(value - actual)
						},
						Err(_) => Ok(value),
					}
				} else {
					Ok(value)
				}
			})

		}

		fn reserved_balance_named(id: &Self::ReserveIdentifier, token: &Self::TokenId, who: &T::AccountId) -> Self::Balance {
			let reserves = Self::reserves((who, token));
			reserves
				.binary_search_by_key(id, |data| data.id)
				.map(|index| reserves[index].amount)
				.unwrap_or_default()
		}

		fn repatriate_reserved_named(
			id: &Self::ReserveIdentifier,
			token: &T::TokenId,
			slashed: &T::AccountId,
			beneficiary: &T::AccountId,
			value: T::Balance,
			status: BalanceStatus,
		) -> sp_std::result::Result<Self::Balance, DispatchError> {

			if value.is_zero() {
				return Ok(Zero::zero())
			}

			if slashed == beneficiary {
				return match status {
					BalanceStatus::Free => Self::unreserve_named(id, token, slashed, value),
					BalanceStatus::Reserved =>
						Ok(value.saturating_sub(Self::reserved_balance_named(id, token, slashed))),
				}
			}

			Reserves::<T>::try_mutate((slashed, token), |reserves| -> Result<Self::Balance, DispatchError> {
				match reserves.binary_search_by_key(id, |data| data.id) {
					Ok(index) => {
						let to_change = cmp::min(reserves[index].amount, value);

						let actual = if status == BalanceStatus::Reserved {
							// make it the reserved under same identifier
							Reserves::<T>::try_mutate(
								(beneficiary,token),
								|reserves| -> Result<T::Balance, DispatchError> {
									match reserves.binary_search_by_key(id, |data| data.id) {
										Ok(index) => {
											let remain =
												<Self as ReservableToken<_>>::repatriate_reserved(
													token,
													slashed,
													beneficiary,
													to_change,
													status,
												)?;

											// remain should always be zero but just to be defensive
											// here
											let actual = to_change.saturating_sub(remain);

											// this add can't overflow but just to be defensive.
											reserves[index].amount =
												reserves[index].amount.saturating_add(actual);

											Ok(actual)
										},
										Err(index) => {
											let remain =
												<Self as ReservableToken<_>>::repatriate_reserved(
													token,
													slashed,
													beneficiary,
													to_change,
													status,
												)?;

											// remain should always be zero but just to be defensive
											// here
											let actual = to_change.saturating_sub(remain);

											reserves
												.try_insert(
													index,
													ReserveData { id: id.clone(), amount: actual },
												)
												.map_err(|_| Error::<T>::TooManyReserves)?;

											Ok(actual)
										},
									}
								},
							)?
						} else {
							let remain = <Self as ReservableToken<_>>::repatriate_reserved(
								token,
								slashed,
								beneficiary,
								to_change,
								status,
							)?;

							// remain should always be zero but just to be defensive here
							to_change.saturating_sub(remain)
						};

						// `actual <= to_change` and `to_change <= amount`; qed;
						reserves[index].amount -= actual;

						Ok(value - actual)
					},
					Err(_) => Ok(value),
				}
			})
		}
	}
}

