use frame_support::traits::BalanceStatus;
use frame_support::{assert_noop, assert_ok};
use fuso_support::traits::ReservableToken;
use sp_keyring::{sr25519::Keyring, AccountKeyring};
use sp_runtime::traits::Zero;
use sp_runtime::MultiAddress;

use crate::mock::*;
use crate::Error;
use crate::Module;
use crate::TokenAccountData;
use crate::TokenInfo;

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
            Some(TokenInfo {
                total: 1000000,
                symbol: br#"USDT"#.to_vec(),
            })
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
