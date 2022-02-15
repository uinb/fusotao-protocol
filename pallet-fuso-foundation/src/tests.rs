use frame_support::{assert_noop, assert_ok};
use sp_keyring::AccountKeyring;
use sp_runtime::traits::Zero;
use sp_runtime::MultiAddress;

use crate::frame_support::traits::OnInitialize;
use crate::mock::*;
use crate::Error;
use crate::FoundationData;
use crate::Pallet;
use frame_system::AccountInfo;

type Foundation = Pallet<Test>;

#[test]
fn test_foundation() {
    new_test_ext().execute_with(|| {
        let alice: AccountId = AccountKeyring::Alice.into();
        let charlie: AccountId = AccountKeyring::Charlie.into();
        frame_system::Pallet::<Test>::set_block_number(30);
        let alice_balance: AccountInfo<u64, pallet_balances::AccountData<u128>> =
            frame_system::Pallet::<Test>::account(&alice);
        assert_eq!(alice_balance.data.reserved, 1500000000000000000000);
        assert_eq!(alice_balance.data.free, 0);

        let alice_foundation = Foundation::foundation(&alice);
        assert!(alice_foundation.is_some());
        assert_eq!(
            alice_foundation.unwrap(),
            FoundationData {
                delay_durations: 2,
                interval_durations: 1,
                times: 5,
                amount: 300000000000000000000
            }
        );

        let weight = Foundation::on_initialize(5);
        assert!(weight == 0);
        let weight = Foundation::on_initialize(15);
        assert!(weight == 0);
        Foundation::on_initialize(20);
        let alice_foundation = Foundation::foundation(&alice);
        assert_eq!(
            alice_foundation.unwrap(),
            FoundationData {
                delay_durations: 2,
                interval_durations: 1,
                times: 4,
                amount: 300000000000000000000
            }
        );
        let alice_balance: AccountInfo<u64, pallet_balances::AccountData<u128>> =
            frame_system::Pallet::<Test>::account(&alice);
        assert_eq!(alice_balance.data.reserved, 1200000000000000000000);
        assert_eq!(alice_balance.data.free, 300000000000000000000);

        Foundation::on_initialize(30);
        Foundation::on_initialize(40);
        Foundation::on_initialize(50);
        let alice_foundation_data = Foundation::foundation(&alice);
        assert!(alice_foundation_data.is_some());
        let alice_foundation = Foundation::foundation(&alice);
        assert_eq!(
            alice_foundation.unwrap(),
            FoundationData {
                delay_durations: 2,
                interval_durations: 1,
                times: 1,
                amount: 300000000000000000000
            }
        );

        Foundation::on_initialize(60);
        let alice_foundation_data = Foundation::foundation(&alice);
        assert!(alice_foundation_data.is_none());
        let alice_balance: AccountInfo<u64, pallet_balances::AccountData<u128>> =
            frame_system::Pallet::<Test>::account(&alice);
        assert_eq!(alice_balance.data.free, 1500000000000000000000);
        assert_eq!(alice_balance.data.reserved, 0);

        let weight = Foundation::on_initialize(101);
        assert!(weight == 0);
        let alice_foundation_data = Foundation::foundation(&alice);
        assert!(alice_foundation_data.is_none());
        let alice_balance: AccountInfo<u64, pallet_balances::AccountData<u128>> =
            frame_system::Pallet::<Test>::account(&alice);
        assert_eq!(alice_balance.data.free, 1500000000000000000000);
        assert_eq!(alice_balance.data.reserved, 0);
    });
}
