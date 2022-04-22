use super::*;
use crate::mock::*;
use crate::mock::{new_tester, AccountId};
use crate::Error;
use crate::Pallet;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use fuso_support::constants::RESERVE_FOR_STAKING;
use fuso_support::constants::*;
use pallet_fuso_token::XToken;
use sp_keyring::AccountKeyring;
use sp_runtime::MultiAddress;

type Token = pallet_fuso_token::Pallet<Test>;
type Verifier = Pallet<Test>;

#[test]
pub fn register_should_work() {
    new_tester().execute_with(|| {
        let alice: AccountId = AccountKeyring::Alice.into();
        let charlie: AccountId = AccountKeyring::Charlie.into();
        frame_system::Pallet::<Test>::set_block_number(15);
        assert_ok!(Verifier::register(
            Origin::signed(alice.clone()),
            b"cool".to_vec()
        ));
        let alice_dominator = Verifier::dominators(&alice);
        assert!(alice_dominator.is_some());
        assert_noop!(
            Verifier::register(Origin::signed(charlie.clone()), b"cool".to_vec()),
            Error::<Test>::InvalidName
        );
        assert_noop!(
            Verifier::register(Origin::signed(alice.clone()), b"cooq".to_vec()),
            Error::<Test>::DominatorAlreadyExists
        );
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
        assert_ok!(Verifier::register(
            Origin::signed(alice.clone()),
            b"cool".to_vec()
        ));
        assert_ok!(Verifier::launch(
            RawOrigin::Root.into(),
            MultiAddress::Id(alice.clone())
        ));

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
        assert_eq!(alice_dominator.status, DOMINATOR_INACTIVE);
        let reserves = Verifier::reserves(&(RESERVE_FOR_STAKING, ferdie.clone(), 0u32), &alice);
        assert_eq!(reserves, 1000);

        assert_ok!(Verifier::stake(
            Origin::signed(ferdie.clone()),
            MultiAddress::Id(alice.clone()),
            9000
        ));
        let alice_dominator: Dominator<u128, u32> = Verifier::dominators(&alice).unwrap();
        assert_eq!(alice_dominator.staked, 10000);
        assert_eq!(alice_dominator.status, DOMINATOR_ACTIVE);
        let reserves = Verifier::reserves(&(RESERVE_FOR_STAKING, ferdie.clone(), 0u32), &alice);
        assert_eq!(reserves, 10000);

        assert_noop!(
            //50 < MinimalStakingAmount(100)
            Verifier::stake(
                Origin::signed(ferdie.clone()),
                MultiAddress::Id(alice.clone()),
                50
            ),
            Error::<Test>::LittleStakingAmount
        );
        let reserves = Verifier::reserves(&(RESERVE_FOR_STAKING, ferdie.clone(), 0u32), &alice);
        assert_eq!(reserves, 10000);
        assert_noop!(
            //10000-9990 < MinimalStakingAmount(100)
            Verifier::unstake(
                Origin::signed(ferdie.clone()),
                MultiAddress::Id(alice.clone()),
                9990
            ),
            Error::<Test>::LittleStakingAmount
        );
        let reserves = Verifier::reserves(&(RESERVE_FOR_STAKING, ferdie.clone(), 0u32), &alice);
        assert_eq!(reserves, 10000);
        assert_ok!(Verifier::unstake(
            Origin::signed(ferdie.clone()),
            MultiAddress::Id(alice.clone()),
            9000
        ));
        let reserves = Verifier::reserves(&(RESERVE_FOR_STAKING, ferdie.clone(), 0u32), &alice);
        assert_eq!(reserves, 1000);
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
        assert_eq!(alice_dominator.status, DOMINATOR_INACTIVE);
    });
}

#[test]
pub fn test_authorize() {
    new_tester().execute_with(|| {
        let alice: AccountId = AccountKeyring::Alice.into();
        let ferdie: AccountId = AccountKeyring::Ferdie.into();
        frame_system::Pallet::<Test>::set_block_number(15);
        assert_ok!(Token::issue(
            Origin::signed(ferdie.clone()),
            6,
            true,
            br#"USDT"#.to_vec(),
            br#"usdt.testnet"#.to_vec(),
        ));
        assert_ok!(Token::do_mint(1, &ferdie, 10000000, None));
        // assert_ok!(Token::issue(
        //     Origin::signed(ferdie.clone()),
        //     10000000000000000000,
        //     br#"USDT"#.to_vec()
        // ));
        let token_info = Token::get_token_info(1);
        assert!(token_info.is_some());
        let token_info: XToken<u128> = token_info.unwrap();
        match token_info {
            XToken::NEP141(_, _, total, _, _) => {
                assert_eq!(total, 10000000000000000000);
            }
        }
        assert_ok!(Verifier::register(
            Origin::signed(alice.clone()),
            b"cool".to_vec()
        ));
        assert_noop!(
            Verifier::authorize(
                Origin::signed(ferdie.clone()),
                MultiAddress::Id(alice.clone()),
                1,
                500000000000
            ),
            Error::<Test>::DominatorInactive
        );
        assert_noop!(
            Verifier::stake(
                Origin::signed(ferdie.clone()),
                MultiAddress::Id(alice.clone()),
                10000
            ),
            Error::<Test>::DominatorStatusInvalid
        );
        assert_noop!(
            Verifier::authorize(
                Origin::signed(ferdie.clone()),
                MultiAddress::Id(alice.clone()),
                1,
                5000000000000000000000000
            ),
            Error::<Test>::DominatorInactive
        );

        assert_ok!(Verifier::launch(
            RawOrigin::Root.into(),
            MultiAddress::Id(alice.clone())
        ));

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
        let reserves = Verifier::reserves(&(RESERVE_FOR_AUTHORIZING, ferdie.clone(), 1u32), &alice);
        assert_eq!(reserves, 0);

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
        let reserves = Verifier::reserves(
            &(RESERVE_FOR_AUTHORIZING_STASH, ferdie.clone(), 1u32),
            &alice,
        );
        assert_eq!(reserves, 100000);
        let t = Verifier::reserves(
            (RESERVE_FOR_AUTHORIZING_STASH, ferdie.clone(), 1),
            alice.clone(),
        );
        assert_eq!(t, 100000);
        assert_noop!(
            Verifier::authorize(
                Origin::signed(ferdie.clone()),
                MultiAddress::Id(alice.clone()),
                1,
                1000000
            ),
            Error::<Test>::ReceiptAlreadyExists
        );
        let t = Verifier::reserves(
            (RESERVE_FOR_AUTHORIZING_STASH, ferdie.clone(), 1),
            alice.clone(),
        );
        assert_eq!(t, 100000);
    });
}
