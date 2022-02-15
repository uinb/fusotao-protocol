use frame_support::traits::BalanceStatus;
use frame_support::{assert_noop, assert_ok};
use fuso_support::traits::ReservableToken;
use sp_keyring::AccountKeyring;

use crate::mock::*;
use crate::Error;
use crate::Pallet;

type Token = Pallet<Test>;

#[test]
fn test_reward_should_work() {
    new_test_ext().execute_with(|| {

    });
}
