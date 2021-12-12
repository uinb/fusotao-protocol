use frame_support::{assert_noop, assert_ok};
use frame_support::traits::BalanceStatus;
use fuso_support::traits::ReservableToken;
use sp_runtime::traits::Zero;

use crate::Error;
use crate::mock::*;
use crate::Module;
use crate::TokenAccountData;
use crate::TokenInfo;

type Token = Module<Test>;

fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into()
}

#[test]
fn issuing_token_and_transfer_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Token::issue(
                Origin::signed(1),
                1000000,
                br#"USDT"#.to_vec()
            ));
		let id = 0u32;
		assert_eq!(
			Token::get_token_info(&id).unwrap(),
			TokenInfo {
				total: 1000000,
				symbol: br#"USDT"#.to_vec(),
			}
		);
		assert_eq!(
			Token::get_token_balance((&id, &1)).unwrap(),
			TokenAccountData {
				free: 1000000,
				reserved: Zero::zero(),
			}
		);
		assert_ok!(Token::transfer(Origin::signed(1), id.clone(), 2, 1000000));
		assert_eq!(
			Token::get_token_balance((&id, &1)).unwrap(),
			TokenAccountData {
				free: Zero::zero(),
				reserved: Zero::zero(),
			}
		);
		assert_eq!(
			Token::get_token_balance((&id, &2)).unwrap(),
			TokenAccountData {
				free: 1000000,
				reserved: Zero::zero(),
			}
		);
	});
}

#[test]
fn reservable_token_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(Token::issue(
                Origin::signed(1),
                1000000,
                br#"USDT"#.to_vec()
            ));
		// let id = <Test as Trait>::Hashing::hash(&0u32.to_ne_bytes());
		let id = 0u32;
		assert_eq!(Token::can_reserve(&id, &1, 1000000), true);
		assert_ok!(Token::reserve(&id, &1, 500000));
		assert_eq!(Token::can_reserve(&id, &1, 1000000), false);
		assert_eq!(
			Token::get_token_balance((&id, &1)).unwrap(),
			TokenAccountData {
				free: 500000,
				reserved: 500000,
			}
		);
		assert_noop!(
                Token::transfer(Origin::signed(1), id, 2, 1000000),
                Error::<Test>::InsufficientBalance
            );
		assert_eq!(
			Token::get_token_balance((&id, &1)).unwrap(),
			TokenAccountData {
				free: 500000,
				reserved: 500000,
			}
		);
		assert_ok!(Token::reserve(&id, &1, 500000));
		assert_eq!(
			Token::get_token_balance((&id, &1)).unwrap(),
			TokenAccountData {
				free: Zero::zero(),
				reserved: 1000000,
			}
		);
		assert_ok!(Token::unreserve(&id, &1, 500000));
		assert_eq!(
			Token::get_token_balance((&id, &1)).unwrap(),
			TokenAccountData {
				free: 500000,
				reserved: 500000,
			}
		);
		assert_ok!(Token::transfer(Origin::signed(1), id.clone(), 2, 1));
		assert_ok!(Token::repatriate_reserved(
                &id,
                &1,
                &2,
                1,
                BalanceStatus::Free
            ));
		assert_eq!(
			Token::get_token_balance((&id, &1)).unwrap(),
			TokenAccountData {
				free: 499999,
				reserved: 499999,
			}
		);
		assert_eq!(
			Token::get_token_balance((&id, &2)).unwrap(),
			TokenAccountData {
				free: 2,
				reserved: Zero::zero(),
			}
		);
		assert_noop!(
                Token::repatriate_reserved(&id, &2, &1, 1, BalanceStatus::Free),
                Error::<Test>::InsufficientBalance
            );
	});
}
