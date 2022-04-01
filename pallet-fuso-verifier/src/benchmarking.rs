#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::mock::*;
use crate::Pallet as Verifier;
use ascii::AsciiStr;
pub use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Hooks;
use frame_system::RawOrigin;
use sp_keyring::AccountKeyring;
use sp_runtime::MultiAddress;
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_runtime::{
    generic,
    traits::{AccountIdLookup, BlakeTwo256},
    MultiSignature,
};
pub(crate) type AccountId = <<MultiSignature as Verify>::Signer as IdentifyAccount>::AccountId;


const SEED: u32 = 0;

benchmarks! {

    where_clause {
        where
        TokenId<T>: Copy + From<u32> + Into<u32>,
        Balance<T>: Copy + From<u128> + Into<u128>,
        T::BlockNumber: Into<u32> + From<u32>,
    }

    register {
        frame_system::Pallet::<T>::set_block_number(30000.into());
        let caller: T::AccountId = account("caller", 0, SEED);
        let alice = AsciiStr::from_ascii(b"alice");
    }:_(RawOrigin::Signed(caller), alice.unwrap().as_bytes().to_vec())

    stake {
        frame_system::Pallet::<T>::set_block_number(30000.into());
        let ferdie: AccountId = AccountKeyring::Ferdie.into();
        let alice: AccountId = AccountKeyring::Alice.into();
        Verifier::register(
            RawOrigin::Signed(alice.clone()),
            b"cool".to_vec()
        );
    }:_(Origin::signed(ferdie), MultiAddress::Id(alice), 1000.into())
}
