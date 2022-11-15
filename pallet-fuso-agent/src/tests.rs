use crate::mock::*;
use crate::Pallet;
use codec::{Decode, Encode};
use frame_support::assert_ok;
use frame_system::RawOrigin;
use secp256k1::*;
use sp_keyring::AccountKeyring;
use sp_runtime::traits::TrailingZeroInput;
use sp_runtime::MultiAddress;

type Agent = Pallet<Test, crate::EthInstance>;
type Balances = pallet_balances::Pallet<Test>;

fn imply_account(pubkey: PublicKey) -> AccountId {
    let address = sp_io::hashing::keccak_256(&pubkey.serialize_uncompressed()[1..])[12..].to_vec();
    let h = (b"-*-#fusotao#-*-", 1u16, address).using_encoded(sp_io::hashing::blake2_256);
    Decode::decode(&mut TrailingZeroInput::new(h.as_ref())).unwrap()
}

#[test]
fn test_derive_address() {
    new_test_ext().execute_with(|| {
        let addr = hex::decode("847Dc5Ea89c407f1416f23D87B40CE317798E133").unwrap();
        let h = (b"-*-#fusotao#-*-", 1u16, addr).using_encoded(|e| {
            println!("{}", hex::encode(&e));
            sp_io::hashing::blake2_256(e)
        });

        use sp_core::crypto::Ss58Codec;
        println!(
            "{}",
            sp_runtime::AccountId32::from(h.clone()).to_ss58check()
        );
    });
}

#[test]
fn basic_sign_should_work() {
    new_test_ext().execute_with(|| {
        let alice: AccountId = AccountKeyring::Alice.into();
        let tx = Call::Balances(pallet_balances::Call::transfer::<Test> {
            dest: MultiAddress::Id(alice.clone()),
            value: 10 * DOLLARS,
        });
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[0xcd; 32]).expect("32 bytes, within curve order");
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);

        use sp_core::Pair;
        let someone = sp_core::ecdsa::Pair::from_seed(&[0xcd; 32]);
        assert_eq!(someone.public().0, public_key.serialize());

        let mut payload = (0u32, tx.clone()).encode();
        let mut prefix = b"\x19Ethereum Signed Message:\n48".to_vec();
        prefix.append(&mut payload);
        let digest = sp_io::hashing::keccak_256(&prefix);
        // sign by substrate
        let sf = someone.sign_prehashed(&digest);
        // sign by secp256k1
        let s = secp.sign_ecdsa_recoverable(&Message::from_slice(&digest).unwrap(), &secret_key);
        let (r, r64) = s.serialize_compact();
        let mut sig = [0u8; 65];
        sig[0..64].copy_from_slice(&r64[..]);
        sig[64] = r.to_i32().try_into().unwrap();
        // compare signature substrate with secp256k1
        assert_eq!(sig, sf.0);
        // compare recover
        let recovered = secp.recover_ecdsa(&Message::from_slice(&digest).unwrap(), &s).unwrap();
        let re = sp_io::crypto::secp256k1_ecdsa_recover_compressed(&sig, &digest).map_err(|_| ()).unwrap();
        assert_eq!(recovered.serialize(), re);
        let re = sp_io::crypto::secp256k1_ecdsa_recover(&sig, &digest).map_err(|_| ()).unwrap();
        assert_eq!(recovered.serialize_uncompressed()[1..], re[..]);
        let unchecked = crate::ExternalVerifiable::Ecdsa {
            tx: Box::new(tx),
            nonce: 0u32,
            signature: sig,
        };
        let account = imply_account(public_key.clone());
        // compare recover from signature of unittest by substrate and secp256k1
        assert_eq!(account, Agent::extract(&unchecked).unwrap());
        assert_ok!(Balances::transfer(
            RawOrigin::Signed(alice.clone()).into(),
            MultiAddress::Id(account.clone()),
            100 * DOLLARS
        ));
        assert_ok!(Agent::submit_external_tx(Origin::none(), unchecked));
        assert_eq!(Balances::free_balance(&account), 90 * DOLLARS);
        // use sp_core::crypto::Ss58Codec;
        // println!(
        //     "{}",
        //     sp_runtime::AccountId32::from(account.clone()).to_ss58check()
        // );
        let mut to_be_sign = hex_literal::hex!("00000000050000d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d130000e8890423c78a");
        let mut prefix = b"\x19Ethereum Signed Message:\n48".to_vec();
        prefix.extend_from_slice(&mut to_be_sign);
        let digest = sp_io::hashing::keccak_256(&prefix);
        let sig = secp.sign_ecdsa_recoverable(&Message::from_slice(&digest).unwrap(), &secret_key);
        let (r, r64) = sig.serialize_compact();
        let mut sig = [0u8; 65];
        sig[0..64].copy_from_slice(&r64[..]);
        sig[64] = r.to_i32().try_into().unwrap();

        let prefix = b"\x19Ethereum Signed Message:\n8Ofg4NGHw";
        let digest = sp_io::hashing::keccak_256(&prefix[..]);
        let signature: [u8; 65] = hex_literal::hex!("aecf9f42ffd739ba2057adea3f035e286c4a40d16875da3b63f116227489831a33b5a3e58ce58bb9ad3bafe81bb7582fc57de2cd23fd90cf3ab58d7a390996b51b");
        let pubkey = sp_io::crypto::secp256k1_ecdsa_recover(&signature, &digest).map_err(|_|()).unwrap();
        let addr = &sp_io::hashing::keccak_256(&pubkey[..])[12..];
        assert_eq!(addr.to_vec(), hex_literal::hex!("544f52f459a42e098775118e0a1880f1fa3eb9a9"));
    });
}
