use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_keyring::AccountKeyring;
use sp_runtime::MultiAddress;

#[test]
fn adding_accounts_to_blacklist_should_work() {
    new_test_ext().execute_with(|| {
        let alice: AccountId = AccountKeyring::Alice.into();
        let admin: AccountId =
            core::str::FromStr::from_str("5CZaAPMzYDstSfY4kSnFV78MvSe2peSyUyCQsguvcJRvsxBR")
                .unwrap();
        assert_ok!(crate::Pallet::<Test>::add_account_to_list(
            RawOrigin::Signed(admin).into(),
            MultiAddress::Id(alice.clone()).into(),
        ));
        assert_eq!(crate::BlackList::<Test>::get(alice.clone()), 0);
        assert_noop!(
            crate::Pallet::<Test>::add_account_to_list(
                RawOrigin::Signed(alice.clone()).into(),
                MultiAddress::Id(alice.clone()).into(),
            ),
            frame_support::dispatch::DispatchError::BadOrigin
        );
    });
}

#[test]
fn raping_accounts_should_work() {
    new_test_ext().execute_with(|| {
        let alice: AccountId = AccountKeyring::Alice.into();
        let admin: AccountId =
            core::str::FromStr::from_str("5CZaAPMzYDstSfY4kSnFV78MvSe2peSyUyCQsguvcJRvsxBR")
                .unwrap();
        assert_ok!(crate::Pallet::<Test>::add_account_to_list(
            RawOrigin::Signed(admin).into(),
            MultiAddress::Id(alice.clone()).into(),
        ));
        use fuso_support::traits::Smuggler;
        assert_eq!(
            pallet_balances::Pallet::<Test>::free_balance(&alice),
            1000 * DOLLARS
        );
        assert!(crate::Pallet::<Test>::repatriate_if_wanted(&alice));
        assert_eq!(pallet_balances::Pallet::<Test>::free_balance(&alice), 0);
    });
}
