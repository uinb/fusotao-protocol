use super::*;
use crate::mock::{new_tester, AccountId};
use frame_support::pallet_prelude::*;
use frame_support::{assert_noop, assert_ok};
use pallet_balances::*;
use sp_keyring::{sr25519::Keyring, AccountKeyring};

use crate::mock::*;
use crate::Error;
use crate::Module;
use pallet_fuso_token::TokenInfo;
use sp_runtime::MultiAddress;

type Token = pallet_fuso_token::Pallet<Test>;
type Verifier = Module<Test>;

#[test]
pub fn register_and_stablecoin_chainge_should_work() {
    new_tester().execute_with(|| {
        let alice: AccountId = AccountKeyring::Alice.into();
        let ferdie: AccountId = AccountKeyring::Ferdie.into();
        let bob: AccountId = AccountKeyring::Bob.into();
        let charlie: AccountId = AccountKeyring::Charlie.into();
        frame_system::Pallet::<Test>::set_block_number(15);
        assert_ok!(Verifier::register(Origin::signed(alice.clone())));
        assert_ok!(Verifier::register(Origin::signed(charlie.clone())));
        assert_noop!(
            Verifier::register(Origin::signed(bob.clone())),
            Error::<Test>::OutOfDominatorSizeLimit
        );
        assert_noop!(
            Verifier::register(Origin::signed(alice.clone())),
            Error::<Test>::DominatorAlreadyExists
        );
        /* assert_noop!(
            Verifier::add_stablecoin(Origin::signed(ferdie.clone()), 1),
            Error::<Test>::DominatorNotFound
        );

        assert_ok!(Verifier::add_stablecoin(Origin::signed(alice.clone()), 3));

        let alice_dominator: Dominator<u32, u128, u32> = Verifier::dominators(&alice).unwrap();
        assert!(alice_dominator.stablecoins.contains(&3));

        assert_ok!(Verifier::add_stablecoin(Origin::signed(alice.clone()), 3));
        let alice_dominator: Dominator<u32, u128, u32> = Verifier::dominators(&alice).unwrap();
        assert!(alice_dominator.stablecoins.contains(&3));
        assert_eq!(alice_dominator.stablecoins.len(), 1);

        assert_ok!(Verifier::add_stablecoin(Origin::signed(alice.clone()), 2));

        let alice_dominator: Dominator<u32, u128, u32> = Verifier::dominators(&alice).unwrap();
        assert!(alice_dominator.stablecoins.contains(&3));
        assert!(alice_dominator.stablecoins.contains(&2));
        assert_eq!(alice_dominator.stablecoins.len(), 2);

        assert_ok!(Verifier::remove_stablecoin(
            Origin::signed(alice.clone()),
            2
        ));

        let alice_dominator: Dominator<u32, u128, u32> = Verifier::dominators(&alice).unwrap();
        assert!(alice_dominator.stablecoins.contains(&3));
        assert!(!alice_dominator.stablecoins.contains(&2));
        assert_eq!(alice_dominator.stablecoins.len(), 1);

        assert_ok!(Verifier::add_stablecoin(Origin::signed(alice.clone()), 5));

        let alice_dominator: Dominator<u32, u128, u32> = Verifier::dominators(&alice).unwrap();
        assert!(alice_dominator.stablecoins.contains(&3));
        assert!(alice_dominator.stablecoins.contains(&5));

        assert_ok!(Verifier::add_stablecoin(Origin::signed(alice.clone()), 2));

        let alice_dominator: Dominator<u32, u128, u32> = Verifier::dominators(&alice).unwrap();
        assert_eq!(alice_dominator.stablecoins.len(), 3);
        assert_ok!(Verifier::add_stablecoin(Origin::signed(alice.clone()), 2));

        assert_noop!(
            Verifier::add_stablecoin(Origin::signed(alice.clone()), 0),
            Error::<Test>::OutOfStablecoinLimit
        );*/
    });
}

#[test]
pub fn test_stake_unstake_should_work() {
    new_tester().execute_with(|| {
        let alice: AccountId = AccountKeyring::Alice.into();
        let ferdie: AccountId = AccountKeyring::Ferdie.into();
        let bob: AccountId = AccountKeyring::Bob.into();
        frame_system::Pallet::<Test>::set_block_number(15);
        assert_noop!(
            Verifier::stake(
                Origin::signed(ferdie.clone()),
                MultiAddress::Id(alice.clone()),
                10000
            ),
            Error::<Test>::DominatorNotFound
        );
        assert_ok!(Verifier::register(Origin::signed(alice.clone())));

        //bob don't have enough TAO
        assert_noop!(
            Verifier::stake(
                Origin::signed(bob.clone()),
                MultiAddress::Id(alice.clone()),
                10000
            ),
            pallet_balances::Error::<Test>::InsufficientBalance
        );
        assert_ok!(Verifier::stake(
            Origin::signed(ferdie.clone()),
            MultiAddress::Id(alice.clone()),
            1000
        ));
        let alice_dominator: Dominator<u128, u32> = Verifier::dominators(&alice).unwrap();
        assert_eq!(alice_dominator.staked, 1000);
        assert_eq!(alice_dominator.active, false);

        assert_ok!(Verifier::stake(
            Origin::signed(ferdie.clone()),
            MultiAddress::Id(alice.clone()),
            9000
        ));
        let alice_dominator: Dominator<u128, u32> = Verifier::dominators(&alice).unwrap();
        assert_eq!(alice_dominator.staked, 10000);
        assert_eq!(alice_dominator.active, true);
        assert_noop!(
            //50 < MinimalStakingAmount(100)
            Verifier::stake(
                Origin::signed(ferdie.clone()),
                MultiAddress::Id(alice.clone()),
                50
            ),
            Error::<Test>::LittleStakingAmount
        );
        assert_noop!(
            //10000-9990 < MinimalStakingAmount(100)
            Verifier::unstake(
                Origin::signed(ferdie.clone()),
                MultiAddress::Id(alice.clone()),
                9990
            ),
            Error::<Test>::LittleStakingAmount
        );
        assert_ok!(Verifier::unstake(
            Origin::signed(ferdie.clone()),
            MultiAddress::Id(alice.clone()),
            9000
        ));
        assert_noop!(
            Verifier::unstake(
                Origin::signed(ferdie.clone()),
                MultiAddress::Id(alice.clone()),
                5000
            ),
            Error::<Test>::InsufficientBalance
        );
        let alice_dominator: Dominator<u128, u32> = Verifier::dominators(&alice).unwrap();
        assert_eq!(alice_dominator.staked, 1000);
        assert_eq!(alice_dominator.active, false);
    });
}

#[test]
pub fn test_authorize() {
    new_tester().execute_with(|| {
        let alice: AccountId = AccountKeyring::Alice.into();
        let ferdie: AccountId = AccountKeyring::Ferdie.into();
        let bob: AccountId = AccountKeyring::Bob.into();
        frame_system::Pallet::<Test>::set_block_number(15);
        assert_ok!(Token::issue(
            Origin::signed(ferdie.clone()),
            10000000000000000000,
            br#"USDT"#.to_vec()
        ));
        let token_info = Token::get_token_info(1);
        assert!(token_info.is_some());
        let token_info: TokenInfo<u128> = token_info.unwrap();
        assert_eq!(token_info.total, 10000000000000000000);
        assert_ok!(Verifier::register(Origin::signed(alice.clone())));
        assert_noop!(
            Verifier::authorize(
                Origin::signed(ferdie.clone()),
                MultiAddress::Id(alice.clone()),
                1,
                500000000000
            ),
            Error::<Test>::DominatorInactive
        );

        assert_ok!(Verifier::stake(
            Origin::signed(ferdie.clone()),
            MultiAddress::Id(alice.clone()),
            10000
        ));
        assert_noop!(
            Verifier::authorize(
                Origin::signed(ferdie.clone()),
                MultiAddress::Id(alice.clone()),
                1,
                5000000000000000000000000
            ),
            Error::<Test>::InsufficientBalance
        );

        assert_noop!(
            Verifier::authorize(
                Origin::signed(ferdie.clone()),
                MultiAddress::Id(alice.clone()),
                0,
                5000000000000000000000000
            ),
            Error::<Test>::InsufficientBalance
        );
        assert_ok!(Verifier::authorize(
            Origin::signed(ferdie.clone()),
            MultiAddress::Id(alice.clone()),
            1,
            100000
        ));
        let t = Verifier::reserves((1u8, ferdie.clone(), 1), alice.clone());
        assert_eq!(t, 100000);
        assert_ok!(Verifier::authorize(
            Origin::signed(ferdie.clone()),
            MultiAddress::Id(alice.clone()),
            1,
            0
        ));
        let t = Verifier::reserves((1u8, ferdie.clone(), 1), alice.clone());
        assert_eq!(t, 100000);
    });
}
