use frame_support::traits::BalanceStatus;
use frame_support::{assert_noop, assert_ok};
use fuso_support::traits::ReservableToken;
use pallet_octopus_support::traits::AssetIdAndNameProvider;
use sp_keyring::{sr25519::Keyring, AccountKeyring};
use sp_runtime::traits::Zero;
use sp_runtime::MultiAddress;

use crate::mock::*;
use crate::Error;
use crate::Module;
use crate::Pallet;
use crate::TokenAccountData;
use crate::XToken;

type Token = Module<Test>;

#[test]
fn issuing_token_and_transfer_should_work() {
    let ferdie: AccountId = AccountKeyring::Ferdie.into();
    let alice: AccountId = AccountKeyring::Alice.into();
    new_test_ext().execute_with(|| {
        assert_ok!(Token::issue(
            Origin::signed(ferdie.clone()),
            1000000,
            br#"USDT"#.to_vec()
        ));
        let id = 1u32;
        assert_eq!(
            Token::get_token_info(&id),
            Some(XToken::NEP141(
                br#"USDT"#.to_vec(),
                br#"USDT"#.to_vec(),
                1000000,
                false
            ))
        );

        assert_eq!(
            Token::get_token_balance((&id, &ferdie)),
            TokenAccountData {
                free: 1000000,
                reserved: Zero::zero(),
            }
        );

        assert_ok!(Token::transfer(
            Origin::signed(ferdie.clone()),
            id.clone(),
            MultiAddress::Id(alice.clone()),
            1000000
        ));
        assert_eq!(
            Token::get_token_balance((&id, &ferdie)),
            TokenAccountData {
                free: Zero::zero(),
                reserved: Zero::zero(),
            }
        );
        assert_eq!(
            Token::get_token_balance((&id, &alice)),
            TokenAccountData {
                free: 1000000,
                reserved: Zero::zero(),
            }
        );
    });
}

#[test]
fn reservable_token_should_work() {
    let ferdie: AccountId = AccountKeyring::Ferdie.into();
    let alice: AccountId = AccountKeyring::Alice.into();
    new_test_ext().execute_with(|| {
        assert_ok!(Token::issue(
            Origin::signed(ferdie.clone()),
            1000000,
            br#"USDT"#.to_vec()
        ));
        let id = 1u32;
        assert_eq!(Token::can_reserve(&id, &ferdie, 1000000), true);
        assert_ok!(Token::reserve(&id, &ferdie, 500000));
        assert_eq!(Token::can_reserve(&id, &ferdie, 1000000), false);
        assert_eq!(
            Token::get_token_balance((&id, &ferdie)),
            TokenAccountData {
                free: 500000,
                reserved: 500000,
            }
        );
        assert_noop!(
            Token::transfer(
                Origin::signed(ferdie.clone()),
                id,
                MultiAddress::Id(alice.clone()),
                1000000
            ),
            Error::<Test>::InsufficientBalance
        );
        assert_eq!(
            Token::get_token_balance((&id, &ferdie)),
            TokenAccountData {
                free: 500000,
                reserved: 500000,
            }
        );
        assert_ok!(Token::reserve(&id, &ferdie, 500000));
        assert_eq!(
            Token::get_token_balance((&id, &ferdie)),
            TokenAccountData {
                free: 0,
                reserved: 1000000,
            }
        );
        assert_ok!(Token::unreserve(&id, &ferdie, 500000));
        assert_eq!(
            Token::get_token_balance((&id, &ferdie)),
            TokenAccountData {
                free: 500000,
                reserved: 500000,
            }
        );
        assert_ok!(Token::transfer(
            Origin::signed(ferdie.clone()),
            id.clone(),
            MultiAddress::Id(alice.clone()),
            1
        ));
        assert_ok!(Token::repatriate_reserved(
            &id,
            &ferdie,
            &alice,
            1,
            BalanceStatus::Free
        ));
        assert_eq!(
            Token::get_token_balance((&id, &ferdie)),
            TokenAccountData {
                free: 499999,
                reserved: 499999,
            }
        );
        assert_eq!(
            Token::get_token_balance((&id, &alice)),
            TokenAccountData {
                free: 2,
                reserved: Zero::zero(),
            }
        );
        assert_noop!(
            Token::repatriate_reserved(&id, &alice, &ferdie, 1, BalanceStatus::Free),
            Error::<Test>::InsufficientBalance
        );
    });
}

#[test]
fn test_xtoken_should_work() {
    new_test_ext().execute_with(|| {
        let alice: AccountId = AccountKeyring::Alice.into();
        let ferdie: AccountId = AccountKeyring::Ferdie.into();
        //token new
        let token_id = Token::try_get_asset_id("USDT").unwrap();
        assert_eq!(token_id, 1);
        let token_id = Token::try_get_asset_id("USDC").unwrap();
        assert_eq!(token_id, 2);
        let token_id = Token::try_get_asset_id("USDT").unwrap();
        assert_eq!(token_id, 1);
        let token_name = Token::try_get_asset_name(1).unwrap();
        assert_eq!(String::from_utf8(token_name).unwrap(), "USDT".to_string());
        let token_info: XToken<u128> = Token::get_token_info(1).unwrap();
        assert_eq!(
            token_info,
            XToken::NEP141(br#"USDT"#.to_vec(), br#"USDT"#.to_vec(), 0, false)
        );
        assert_noop!(
            Token::do_mint(3, &alice, 100000000000, Option::None),
            Error::<Test>::InvalidToken
        );

        assert_ok!(Token::do_mint(1, &alice, 100000000000, Option::None));
        let b: TokenAccountData<u128> = Token::get_token_balance((&1, &alice));
        assert_eq!(
            b,
            TokenAccountData {
                free: 100000000000,
                reserved: 0
            }
        );
        let token_info: XToken<u128> = Token::get_token_info(1).unwrap();
        assert_eq!(
            token_info,
            XToken::NEP141(
                br#"USDT"#.to_vec(),
                br#"USDT"#.to_vec(),
                100000000000,
                false
            )
        );

        assert_ok!(Token::do_mint(1, &ferdie, 100000000000, Option::None));
        let b: TokenAccountData<u128> = Token::get_token_balance((&1, &ferdie));
        assert_eq!(
            b,
            TokenAccountData {
                free: 100000000000,
                reserved: 0
            }
        );
        let token_info: XToken<u128> = Token::get_token_info(1).unwrap();
        assert_eq!(
            token_info,
            XToken::NEP141(
                br#"USDT"#.to_vec(),
                br#"USDT"#.to_vec(),
                200000000000,
                false
            )
        );

        assert_noop!(
            Token::do_burn(1, &alice, 1000000000000, Option::None),
            Error::<Test>::InsufficientBalance
        );
        assert_ok!(Token::do_burn(1, &alice, 100000000000, Option::None));
        let b: TokenAccountData<u128> = Token::get_token_balance((&1, &alice));
        assert_eq!(
            b,
            TokenAccountData {
                free: 0,
                reserved: 0
            }
        );
        let token_info: XToken<u128> = Token::get_token_info(1).unwrap();
        assert_eq!(
            token_info,
            XToken::NEP141(
                br#"USDT"#.to_vec(),
                br#"USDT"#.to_vec(),
                100000000000,
                false
            )
        );
    });
}
