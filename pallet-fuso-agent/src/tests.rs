use crate::mock::*;
use crate::{Error, Pallet};
use codec::{Decode, Encode};
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use sp_core::{ecdsa, Pair};
use sp_keyring::AccountKeyring;
use sp_runtime::traits::TrailingZeroInput;
use sp_runtime::MultiAddress;

type Agent = Pallet<Test, crate::EthInstance>;
type Balances = pallet_balances::Pallet<Test>;

fn generate_pair() -> ecdsa::Pair {
    ecdsa::Pair::from_phrase(
        "embark speed ignore close kid junior target frost tissue laundry amount cradle",
        None,
    )
    .unwrap()
    .0
}

fn imply_account(pubkey: ecdsa::Public) -> AccountId {
    let address = sp_io::hashing::keccak_256(&pubkey.0)[12..].to_vec();
    let h = (b"-*-#fusotao#-*-", 1u16, address).using_encoded(sp_io::hashing::blake2_256);
    Decode::decode(&mut TrailingZeroInput::new(h.as_ref())).unwrap()
}

#[test]
fn basic_sign_should_work() {
    new_test_ext().execute_with(|| {
        let alice: AccountId = AccountKeyring::Alice.into();
        let tx = Call::Balances(pallet_balances::Call::transfer::<Test> {
            dest: MultiAddress::Id(alice.clone()),
            value: 10 * DOLLARS,
        });
        let someone = generate_pair();
        let mut payload = (0u64, tx.clone()).encode();
        let mut prefix = b"\x19Ethereum Signed Message:\n51".to_vec();
        prefix.append(&mut payload);
        let r = someone.sign(&prefix);
        let unchecked = crate::ExternalVerifiable::Ecdsa {
            tx: Box::new(tx),
            nonce: 0u64,
            signature: r,
        };
        let account = imply_account(someone.public());
        assert_eq!(account, Agent::extract(&unchecked).unwrap());
        assert_ok!(Balances::transfer(
            RawOrigin::Signed(alice.clone()).into(),
            MultiAddress::Id(account.clone()),
            100 * DOLLARS
        ));
        assert_ok!(Agent::submit_external_tx(Origin::none(), unchecked));
        assert_eq!(Balances::free_balance(&account), 90 * DOLLARS);
    });
}
