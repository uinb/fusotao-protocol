#![cfg(test)]
use super::{
    mock::{
        assert_events, expect_event, new_test_ext, Assets, Balances, Bridge, Call,
        ChainBridgeTransfer, Event, NativeResourceId, Origin, ProposalLifetime, Test,
        ENDOWED_BALANCE, RELAYER_A, RELAYER_B, RELAYER_C,
    },
    *,
};
use crate::Error::InvalidCallMessage;
use crate::{
    mock::{event_exists, AccountId, Balance, DOLLARS},
    Event as ChainBridgeTransferEvent,
};
use frame_support::{
    assert_err, assert_noop, assert_ok, dispatch::DispatchError, traits::fungibles::Inspect,
};
use fuso_support::chainbridge::*;
use fuso_support::{derive_resource_id, traits::Token, XToken};
use pallet_fuso_token as assets;
use sp_core::bytes::from_hex;
use sp_core::{blake2_256, crypto::AccountId32, H256};
use sp_keyring::AccountKeyring;
use sp_runtime::traits::Zero;
use sp_runtime::ModuleError;

const TEST_THRESHOLD: u32 = 2;

fn make_remark_proposal(call: Vec<u8>) -> Call {
    let depositer = [0u8; 20];
    Call::ChainBridgeTransfer(crate::Call::remark {
        message: call,
        depositer,
        r_id: Default::default(),
    })
}

fn make_transfer_proposal(resource_id: ResourceId, to: AccountId32, amount: u64) -> Call {
    Call::ChainBridgeTransfer(crate::Call::transfer_in {
        to,
        amount: amount.into(),
        r_id: resource_id,
    })
}

#[test]
fn transfer_native() {
    new_test_ext().execute_with(|| {
        let dest_chain = 0;
        let resource_id = NativeResourceId::get();
        let amount: Balance = 1 * DOLLARS;
        let recipient = b"davirain.xyz".to_vec(); // recipient account

        assert_ok!(Bridge::whitelist_chain(Origin::root(), dest_chain.clone()));
        assert_ok!(ChainBridgeTransfer::transfer_out(
            Origin::signed(RELAYER_A),
            amount.clone(),
            resource_id.clone(),
            recipient.clone(),
            dest_chain,
        ));

        expect_event(bridge::Event::FungibleTransfer(
            dest_chain,
            1,
            resource_id,
            amount.into(),
            recipient,
        ));
    })
}

// #[test]
// fn transfer_erc721() {
//     new_test_ext().execute_with(|| {
//         let dest_chain = 0;
//         let resource_id = Erc721Id::get();
//         let token_id: U256 = U256::from(100);
//         let token_id_slice: &mut [u8] = &mut [0; 32];
//         token_id.to_big_endian(token_id_slice);
//         let metadata: Vec<u8> = vec![1, 2, 3, 4];
//         let recipient = vec![99];

//         // Create a token
//         assert_ok!(Erc721::mint(
//             Origin::root(),
//             RELAYER_A,
//             token_id,
//             metadata.clone()
//         ));
//         assert_eq!(
//             Erc721::tokens(token_id).unwrap(),
//             Erc721Token {
//                 id: token_id,
//                 metadata: metadata.clone()
//             }
//         );

//         // Whitelist destination and transfer
//         assert_ok!(Bridge::whitelist_chain(Origin::root(), dest_chain.clone()));
//         assert_ok!(ChainBridgeTransfer::transfer_erc721(
//             Origin::signed(RELAYER_A),
//             recipient.clone(),
//             token_id,
//             dest_chain,
//         ));

//         expect_event(bridge::Event::NonFungibleTransfer(
//             dest_chain,
//             1,
//             resource_id,
//             token_id_slice.to_vec(),
//             recipient.clone(),
//             metadata,
//         ));

//         // Ensure token no longer exists
//         assert_eq!(Erc721::tokens(token_id), None);

//         // Transfer should fail as token doesn't exist
//         assert_noop!(
//             ChainBridgeTransfer::transfer_erc721(
//                 Origin::signed(RELAYER_A),
//                 recipient.clone(),
//                 token_id,
//                 dest_chain,
//             ),
//             Error::<Test>::InvalidTransfer
//         );
//     })
// }

// #[test]
// fn mint_erc721() {
//     new_test_ext().execute_with(|| {
//         let token_id = U256::from(99);
//         let recipient = RELAYER_A;
//         let metadata = vec![1, 1, 1, 1];
//         let bridge_id: AccountId32 = Bridge::account_id();
//         let resource_id = HashId::get();
//         // Token doesn't yet exist
//         assert_eq!(Erc721::tokens(token_id), None);
//         // Mint
//         assert_ok!(ChainBridgeTransfer::mint_erc721(
//             Origin::signed(bridge_id.clone()),
//             recipient.clone(),
//             token_id,
//             metadata.clone(),
//             resource_id,
//         ));
//         // Ensure token exists
//         assert_eq!(
//             Erc721::tokens(token_id).unwrap(),
//             Erc721Token {
//                 id: token_id,
//                 metadata: metadata.clone()
//             }
//         );
//         // Cannot mint same token
//         assert_noop!(
//             ChainBridgeTransfer::mint_erc721(
//                 Origin::signed(bridge_id),
//                 recipient,
//                 token_id,
//                 metadata.clone(),
//                 resource_id,
//             ),
//             erc721::Error::<Test>::TokenAlreadyExists
//         );
//     })
// }

#[test]
fn transfer_non_native() {
    new_test_ext().execute_with(|| {
        let dest_chain = 5;
        // get resource id
        let ferdie: AccountId = AccountKeyring::Ferdie.into();
        let recipient = vec![99];
        // set token_id
        // assert_ok!(ChainBridgeTransfer::set_token_id(
        //     Origin::root(),
        //     resource_id.clone(),
        //     0,
        //     b"DENOM".to_vec()
        // ));

        // force_create Assets token_id 0
        // assert_ok!(Assets::force_create(
        //     Origin::root(),
        //     0,
        //     sp_runtime::MultiAddress::Id(ferdie.clone()),
        //     true,
        //     1
        // ));
        let contract_address = "304203995023530303420592059205902501";
        let denom = XToken::ERC20(
            br#"DENOM"#.to_vec(),
            hex::decode(contract_address).unwrap(),
            Zero::zero(),
            true,
            18,
        );
        let resource_id = derive_resource_id(
            dest_chain,
            hex::decode(contract_address).unwrap().as_slice(),
        )
        .unwrap();
        assert_ok!(Assets::issue(frame_system::RawOrigin::Root.into(), denom,));
        let amount: Balance = 1 * DOLLARS;
        assert_ok!(Assets::do_mint(1, &ferdie, amount, None));

        // make sure have some  amount after mint
        assert_eq!(Assets::free_balance(&1, &ferdie), amount);
        assert_ok!(Bridge::whitelist_chain(Origin::root(), dest_chain.clone()));
        assert_ok!(ChainBridgeTransfer::transfer_out(
            Origin::signed(ferdie.clone()),
            amount,
            resource_id,
            recipient.clone(),
            dest_chain,
        ));

        // make sure transfer have 0 amount
        assert_eq!(Assets::balance(0, &ferdie), 0);

        assert_events(vec![Event::Bridge(bridge::Event::FungibleTransfer(
            dest_chain,
            1,
            resource_id,
            U256::from(amount),
            recipient,
        ))]);
    })
}

#[test]
fn transfer() {
    new_test_ext().execute_with(|| {
        // Check inital state
        let bridge_id: AccountId32 = Bridge::account_id();
        let resource_id = NativeResourceId::get();
        assert_eq!(Balances::free_balance(&bridge_id), ENDOWED_BALANCE);
        // Transfer and check result
        assert_ok!(ChainBridgeTransfer::transfer_in(
            Origin::signed(Bridge::account_id()),
            RELAYER_A,
            10,
            resource_id,
        ));
        assert_eq!(Balances::free_balance(&bridge_id), ENDOWED_BALANCE - 10);
        assert_eq!(Balances::free_balance(RELAYER_A), ENDOWED_BALANCE + 10);

        assert_events(vec![Event::Balances(pallet_balances::Event::Transfer {
            from: Bridge::account_id(),
            to: RELAYER_A,
            amount: 10,
        })]);
    })
}

#[test]
fn execute_remark() {
    new_test_ext().execute_with(|| {
        let call = frame_system::Call::remark::<Test> {
            remark: vec![0xff; 32],
        };
        let proposal = make_remark_proposal(call.encode());
        let prop_id = 1;
        let src_id = 1;
        let r_id = derive_resource_id(src_id, b"hash").unwrap();
        let resource = b"Example.remark".to_vec();

        assert_ok!(Bridge::set_threshold(Origin::root(), TEST_THRESHOLD,));
        assert_ok!(Bridge::add_relayer(Origin::root(), RELAYER_A));
        assert_ok!(Bridge::add_relayer(Origin::root(), RELAYER_B));
        assert_ok!(Bridge::whitelist_chain(Origin::root(), src_id));
        assert_ok!(Bridge::set_resource(Origin::root(), r_id, resource));

        assert_ok!(Bridge::acknowledge_proposal(
            Origin::signed(RELAYER_A),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        /* assert_ok!(Bridge::acknowledge_proposal(
            Origin::signed(RELAYER_B),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));*/

        // event_exists(ChainBridgeTransferEvent::Remark(hash));
    })
}

#[test]
fn execute_remark_bad_origin() {
    new_test_ext().execute_with(|| {
        let depositer = [0u8; 20];
        let hash: H256 = "ABC".using_encoded(blake2_256).into();
        // Don't allow any signed origin except from bridge addr
        assert_noop!(
            ChainBridgeTransfer::remark(
                Origin::signed(RELAYER_A),
                hash.as_bytes().to_vec(),
                depositer,
                Default::default(),
            ),
            DispatchError::BadOrigin
        );
        // Don't allow root calls
        assert_noop!(
            ChainBridgeTransfer::remark(
                Origin::root(),
                hash.as_bytes().to_vec(),
                depositer,
                Default::default(),
            ),
            DispatchError::BadOrigin
        );
    })
}

#[test]
fn create_sucessful_transfer_proposal_non_native_token() {
    new_test_ext().execute_with(|| {
        let prop_id = 1;
        let src_id = 5;
        let r_id = derive_resource_id(src_id, b"transfer").unwrap();
        let resource = b"ChainBridgeTransfer.transfer".to_vec();
        // let resource_id = NativeTokenId::get();
        let contract_address = "b20f54288947a89a4891d181b10fe04560b55c5e82de1fa2";
        let resource_id =
            derive_resource_id(src_id, hex::decode(contract_address).unwrap().as_slice()).unwrap();
        let proposal = make_transfer_proposal(resource_id, RELAYER_A, 10);
        let ferdie: AccountId = AccountKeyring::Ferdie.into();

        let denom = XToken::ERC20(
            br#"DENOM"#.to_vec(),
            hex::decode(contract_address).unwrap(),
            Zero::zero(),
            true,
            18,
        );
        assert_ok!(Assets::issue(frame_system::RawOrigin::Root.into(), denom,));

        let amount: Balance = 1 * DOLLARS;

        assert_ok!(Bridge::set_threshold(Origin::root(), TEST_THRESHOLD,));
        assert_ok!(Bridge::add_relayer(Origin::root(), RELAYER_A));
        assert_ok!(Bridge::add_relayer(Origin::root(), RELAYER_B));
        assert_ok!(Bridge::add_relayer(Origin::root(), RELAYER_C));
        assert_ok!(Bridge::whitelist_chain(Origin::root(), src_id));
        assert_ok!(Bridge::set_resource(Origin::root(), r_id, resource));

        // Create proposal (& vote)
        assert_ok!(Bridge::acknowledge_proposal(
            Origin::signed(RELAYER_A),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = Bridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = bridge::ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![],
            status: bridge::ProposalStatus::Initiated,
            expiry: ProposalLifetime::get() + 1,
        };
        assert_eq!(prop, expected);

        // Second relayer votes against
        assert_ok!(Bridge::reject_proposal(
            Origin::signed(RELAYER_B),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = Bridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = bridge::ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![RELAYER_B],
            status: bridge::ProposalStatus::Initiated,
            expiry: ProposalLifetime::get() + 1,
        };
        assert_eq!(prop, expected);

        // Third relayer votes in favour
        assert_ok!(Bridge::acknowledge_proposal(
            Origin::signed(RELAYER_C),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = Bridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = bridge::ProposalVotes {
            votes_for: vec![RELAYER_A, RELAYER_C],
            votes_against: vec![RELAYER_B],
            status: bridge::ProposalStatus::Approved,
            expiry: ProposalLifetime::get() + 1,
        };
        assert_eq!(prop, expected);

        // mint 10 resource_id to RELAYER_A
        assert_eq!(Assets::free_balance(&1, &RELAYER_A), 10);

        assert_events(vec![
            Event::Bridge(bridge::Event::VoteFor(src_id, prop_id, RELAYER_A)),
            Event::Bridge(bridge::Event::VoteAgainst(src_id, prop_id, RELAYER_B)),
            Event::Bridge(bridge::Event::VoteFor(src_id, prop_id, RELAYER_C)),
            Event::Bridge(bridge::Event::ProposalApproved(src_id, prop_id)),
            Event::Assets(assets::Event::TokenMinted(1, RELAYER_A, 10)),
            Event::Bridge(bridge::Event::ProposalSucceeded(src_id, prop_id)),
        ]);
    })
}

#[test]
fn create_sucessful_transfer_proposal_native_token() {
    new_test_ext().execute_with(|| {
        let prop_id = 1;
        let src_id = 1;
        let r_id = derive_resource_id(src_id, b"transfer").unwrap();
        let resource = b"ChainBridgeTransfer.transfer".to_vec();
        let resource_id = NativeResourceId::get();
        let proposal = make_transfer_proposal(resource_id, RELAYER_A, 10);

        assert_ok!(Bridge::set_threshold(Origin::root(), TEST_THRESHOLD));
        assert_ok!(Bridge::add_relayer(Origin::root(), RELAYER_A));
        assert_ok!(Bridge::add_relayer(Origin::root(), RELAYER_B));
        assert_ok!(Bridge::add_relayer(Origin::root(), RELAYER_C));
        assert_ok!(Bridge::whitelist_chain(Origin::root(), src_id));
        assert_ok!(Bridge::set_resource(Origin::root(), r_id, resource));

        // Create proposal (& vote)
        assert_ok!(Bridge::acknowledge_proposal(
            Origin::signed(RELAYER_A),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = Bridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = bridge::ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![],
            status: bridge::ProposalStatus::Initiated,
            expiry: ProposalLifetime::get() + 1,
        };
        assert_eq!(prop, expected);

        // Second relayer votes against
        assert_ok!(Bridge::reject_proposal(
            Origin::signed(RELAYER_B),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = Bridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = bridge::ProposalVotes {
            votes_for: vec![RELAYER_A],
            votes_against: vec![RELAYER_B],
            status: bridge::ProposalStatus::Initiated,
            expiry: ProposalLifetime::get() + 1,
        };
        assert_eq!(prop, expected);

        // Third relayer votes in favour
        assert_ok!(Bridge::acknowledge_proposal(
            Origin::signed(RELAYER_C),
            prop_id,
            src_id,
            r_id,
            Box::new(proposal.clone())
        ));
        let prop = Bridge::votes(src_id, (prop_id.clone(), proposal.clone())).unwrap();
        let expected = bridge::ProposalVotes {
            votes_for: vec![RELAYER_A, RELAYER_C],
            votes_against: vec![RELAYER_B],
            status: bridge::ProposalStatus::Approved,
            expiry: ProposalLifetime::get() + 1,
        };
        assert_eq!(prop, expected);

        assert_eq!(Balances::free_balance(RELAYER_A), ENDOWED_BALANCE + 10);
        assert_eq!(
            Balances::free_balance(Bridge::account_id()),
            ENDOWED_BALANCE - 10
        );

        assert_events(vec![
            Event::Bridge(bridge::Event::VoteFor(src_id, prop_id, RELAYER_A)),
            Event::Bridge(bridge::Event::VoteAgainst(src_id, prop_id, RELAYER_B)),
            Event::Bridge(bridge::Event::VoteFor(src_id, prop_id, RELAYER_C)),
            Event::Bridge(bridge::Event::ProposalApproved(src_id, prop_id)),
            Event::Balances(pallet_balances::Event::Transfer {
                from: Bridge::account_id(),
                to: RELAYER_A,
                amount: 10,
            }),
            Event::Bridge(bridge::Event::ProposalSucceeded(src_id, prop_id)),
        ]);
    })
}
