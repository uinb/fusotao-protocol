use super::*;
use crate::Pallet as Verifier;
pub use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use sp_runtime::traits::StaticLookup;

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
    }:_(RawOrigin::Signed(caller), b"alice".to_vec())

    stake {
        frame_system::Pallet::<T>::set_block_number(30000.into());
        let ferdie: T::AccountId =  account("ferdie", 0, SEED);
        let alice:  T::AccountId =  account("alice", 0, SEED);
        let _ = Verifier::<T>::register(
            <T as frame_system::Config>::Origin::from(RawOrigin::Signed(alice.clone())),
            b"cool".to_vec()
        )?;
        let dominator = T::Lookup::unlookup(alice);
    }:_(RawOrigin::Signed(ferdie), dominator, 1000.into())

    unstake {
        frame_system::Pallet::<T>::set_block_number(30000.into());
        let ferdie: T::AccountId =  account("ferdie", 0, SEED);
        let alice:  T::AccountId =  account("alice", 0, SEED);
        let _ = Verifier::<T>::register(
            <T as frame_system::Config>::Origin::from(RawOrigin::Signed(alice.clone())),
            b"cool".to_vec()
        )?;
        let dominator = T::Lookup::unlookup(alice);
        let _ = Verifier::<T>::stake(
            <T as frame_system::Config>::Origin::from(RawOrigin::Signed(ferdie.clone())),
            dominator.clone(),
            10000.into()
        )?;
    } :_(RawOrigin::Signed(ferdie), dominator, 5000.into())

    claim_shares {
        frame_system::Pallet::<T>::set_block_number(20000.into());
        let ferdie: T::AccountId =  account("ferdie", 0, SEED);
        let alice:  T::AccountId =  account("alice", 0, SEED);
        let _ = Verifier::<T>::register(
            <T as frame_system::Config>::Origin::from(RawOrigin::Signed(alice.clone())),
            b"cool".to_vec()
        )?;
        let dominator = T::Lookup::unlookup(alice);
        let _ = Verifier::<T>::stake(
            <T as frame_system::Config>::Origin::from(RawOrigin::Signed(ferdie.clone())),
            dominator.clone(),
            10000.into()
        )?;
    } :_(RawOrigin::Signed(ferdie), dominator)

    authorize {
        frame_system::Pallet::<T>::set_block_number(2000.into());
        let ferdie: T::AccountId =  account("ferdie", 0, SEED);
        let alice:  T::AccountId =  account("alice", 0, SEED);
        let _ = Verifier::<T>::register(
            <T as frame_system::Config>::Origin::from(RawOrigin::Signed(alice.clone())),
            b"cool".to_vec()
        )?;
        let dominator = T::Lookup::unlookup(alice);
        <T as pallet_fuso_token::Pallet::Config>::issue(
            <T as frame_system::Config>::Origin::from(RawOrigin::Signed(ferdie.clone())),
            6,
            true,
            br#"USDT"#.to_vec(),
            br#"usdt.testnet"#.to_vec(),
        );
        <T as pallet_fuso_token::Pallet::Config>::do_mint(
            1,
            &ferdie,
            10000000,
            None)?;
    } :_(RawOrigin::Signed(ferdie),dominator,1.into(),500000000000.into())

}
