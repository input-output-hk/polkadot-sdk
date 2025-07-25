// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Autogenerated weights for `pallet_bags_list`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 32.0.0
//! DATE: 2025-07-01, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `66f1737e2c94`, CPU: `Intel(R) Xeon(R) CPU @ 2.60GHz`
//! WASM-EXECUTION: `Compiled`, CHAIN: `None`, DB CACHE: `1024`

// Executed Command:
// frame-omni-bencher
// v1
// benchmark
// pallet
// --extrinsic=*
// --runtime=target/production/wbuild/kitchensink-runtime/kitchensink_runtime.wasm
// --pallet=pallet_bags_list
// --header=/__w/polkadot-sdk/polkadot-sdk/substrate/HEADER-APACHE2
// --output=/__w/polkadot-sdk/polkadot-sdk/substrate/frame/bags-list/src/weights.rs
// --wasm-execution=compiled
// --steps=50
// --repeat=20
// --heap-pages=4096
// --template=substrate/.maintain/frame-weight-template.hbs
// --no-storage-info
// --no-min-squares
// --no-median-slopes
// --exclude-pallets=pallet_xcm,pallet_xcm_benchmarks::fungible,pallet_xcm_benchmarks::generic,pallet_nomination_pools,pallet_remark,pallet_transaction_storage

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]
#![allow(dead_code)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for `pallet_bags_list`.
pub trait WeightInfo {
	fn rebag_non_terminal() -> Weight;
	fn rebag_terminal() -> Weight;
	fn put_in_front_of() -> Weight;
	fn on_idle() -> Weight;
}

/// Weights for `pallet_bags_list` using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: `VoterList::Lock` (r:1 w:0)
	/// Proof: `VoterList::Lock` (`max_values`: Some(1), `max_size`: Some(0), added: 495, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListNodes` (r:4 w:4)
	/// Proof: `VoterList::ListNodes` (`max_values`: None, `max_size`: Some(154), added: 2629, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Bonded` (r:1 w:0)
	/// Proof: `Staking::Bonded` (`max_values`: None, `max_size`: Some(72), added: 2547, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Ledger` (r:1 w:0)
	/// Proof: `Staking::Ledger` (`max_values`: None, `max_size`: Some(1091), added: 3566, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListBags` (r:1 w:1)
	/// Proof: `VoterList::ListBags` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	fn rebag_non_terminal() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1818`
		//  Estimated: `11506`
		// Minimum execution time: 70_816_000 picoseconds.
		Weight::from_parts(73_697_000, 11506)
			.saturating_add(T::DbWeight::get().reads(8_u64))
			.saturating_add(T::DbWeight::get().writes(5_u64))
	}
	/// Storage: `VoterList::Lock` (r:1 w:0)
	/// Proof: `VoterList::Lock` (`max_values`: Some(1), `max_size`: Some(0), added: 495, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListNodes` (r:3 w:3)
	/// Proof: `VoterList::ListNodes` (`max_values`: None, `max_size`: Some(154), added: 2629, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Bonded` (r:1 w:0)
	/// Proof: `Staking::Bonded` (`max_values`: None, `max_size`: Some(72), added: 2547, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Ledger` (r:1 w:0)
	/// Proof: `Staking::Ledger` (`max_values`: None, `max_size`: Some(1091), added: 3566, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListBags` (r:2 w:2)
	/// Proof: `VoterList::ListBags` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	fn rebag_terminal() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1712`
		//  Estimated: `8877`
		// Minimum execution time: 68_640_000 picoseconds.
		Weight::from_parts(70_632_000, 8877)
			.saturating_add(T::DbWeight::get().reads(8_u64))
			.saturating_add(T::DbWeight::get().writes(5_u64))
	}
	/// Storage: `VoterList::Lock` (r:1 w:0)
	/// Proof: `VoterList::Lock` (`max_values`: Some(1), `max_size`: Some(0), added: 495, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListNodes` (r:4 w:4)
	/// Proof: `VoterList::ListNodes` (`max_values`: None, `max_size`: Some(154), added: 2629, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Bonded` (r:2 w:0)
	/// Proof: `Staking::Bonded` (`max_values`: None, `max_size`: Some(72), added: 2547, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Ledger` (r:2 w:0)
	/// Proof: `Staking::Ledger` (`max_values`: None, `max_size`: Some(1091), added: 3566, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::CounterForListNodes` (r:1 w:1)
	/// Proof: `VoterList::CounterForListNodes` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListBags` (r:1 w:1)
	/// Proof: `VoterList::ListBags` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	fn put_in_front_of() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2024`
		//  Estimated: `11506`
		// Minimum execution time: 79_740_000 picoseconds.
		Weight::from_parts(81_090_000, 11506)
			.saturating_add(T::DbWeight::get().reads(11_u64))
			.saturating_add(T::DbWeight::get().writes(6_u64))
	}
	/// Storage: `VoterList::CounterForListNodes` (r:1 w:0)
	/// Proof: `VoterList::CounterForListNodes` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::Lock` (r:1 w:0)
	/// Proof: `VoterList::Lock` (`max_values`: Some(1), `max_size`: Some(0), added: 495, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::NextNodeAutoRebagged` (r:1 w:1)
	/// Proof: `VoterList::NextNodeAutoRebagged` (`max_values`: Some(1), `max_size`: Some(32), added: 527, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListBags` (r:200 w:4)
	/// Proof: `VoterList::ListBags` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListNodes` (r:11 w:11)
	/// Proof: `VoterList::ListNodes` (`max_values`: None, `max_size`: Some(154), added: 2629, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Bonded` (r:10 w:0)
	/// Proof: `Staking::Bonded` (`max_values`: None, `max_size`: Some(72), added: 2547, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Ledger` (r:10 w:0)
	/// Proof: `Staking::Ledger` (`max_values`: None, `max_size`: Some(1091), added: 3566, mode: `MaxEncodedLen`)
	fn on_idle() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4920`
		//  Estimated: `512390`
		// Minimum execution time: 706_288_000 picoseconds.
		Weight::from_parts(716_009_000, 512390)
			.saturating_add(T::DbWeight::get().reads(234_u64))
			.saturating_add(T::DbWeight::get().writes(16_u64))
	}
}

// For backwards compatibility and tests.
impl WeightInfo for () {
	/// Storage: `VoterList::Lock` (r:1 w:0)
	/// Proof: `VoterList::Lock` (`max_values`: Some(1), `max_size`: Some(0), added: 495, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListNodes` (r:4 w:4)
	/// Proof: `VoterList::ListNodes` (`max_values`: None, `max_size`: Some(154), added: 2629, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Bonded` (r:1 w:0)
	/// Proof: `Staking::Bonded` (`max_values`: None, `max_size`: Some(72), added: 2547, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Ledger` (r:1 w:0)
	/// Proof: `Staking::Ledger` (`max_values`: None, `max_size`: Some(1091), added: 3566, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListBags` (r:1 w:1)
	/// Proof: `VoterList::ListBags` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	fn rebag_non_terminal() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1818`
		//  Estimated: `11506`
		// Minimum execution time: 70_816_000 picoseconds.
		Weight::from_parts(73_697_000, 11506)
			.saturating_add(RocksDbWeight::get().reads(8_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: `VoterList::Lock` (r:1 w:0)
	/// Proof: `VoterList::Lock` (`max_values`: Some(1), `max_size`: Some(0), added: 495, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListNodes` (r:3 w:3)
	/// Proof: `VoterList::ListNodes` (`max_values`: None, `max_size`: Some(154), added: 2629, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Bonded` (r:1 w:0)
	/// Proof: `Staking::Bonded` (`max_values`: None, `max_size`: Some(72), added: 2547, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Ledger` (r:1 w:0)
	/// Proof: `Staking::Ledger` (`max_values`: None, `max_size`: Some(1091), added: 3566, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListBags` (r:2 w:2)
	/// Proof: `VoterList::ListBags` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	fn rebag_terminal() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1712`
		//  Estimated: `8877`
		// Minimum execution time: 68_640_000 picoseconds.
		Weight::from_parts(70_632_000, 8877)
			.saturating_add(RocksDbWeight::get().reads(8_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: `VoterList::Lock` (r:1 w:0)
	/// Proof: `VoterList::Lock` (`max_values`: Some(1), `max_size`: Some(0), added: 495, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListNodes` (r:4 w:4)
	/// Proof: `VoterList::ListNodes` (`max_values`: None, `max_size`: Some(154), added: 2629, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Bonded` (r:2 w:0)
	/// Proof: `Staking::Bonded` (`max_values`: None, `max_size`: Some(72), added: 2547, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Ledger` (r:2 w:0)
	/// Proof: `Staking::Ledger` (`max_values`: None, `max_size`: Some(1091), added: 3566, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::CounterForListNodes` (r:1 w:1)
	/// Proof: `VoterList::CounterForListNodes` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListBags` (r:1 w:1)
	/// Proof: `VoterList::ListBags` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	fn put_in_front_of() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2024`
		//  Estimated: `11506`
		// Minimum execution time: 79_740_000 picoseconds.
		Weight::from_parts(81_090_000, 11506)
			.saturating_add(RocksDbWeight::get().reads(11_u64))
			.saturating_add(RocksDbWeight::get().writes(6_u64))
	}
	/// Storage: `VoterList::CounterForListNodes` (r:1 w:0)
	/// Proof: `VoterList::CounterForListNodes` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::Lock` (r:1 w:0)
	/// Proof: `VoterList::Lock` (`max_values`: Some(1), `max_size`: Some(0), added: 495, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::NextNodeAutoRebagged` (r:1 w:1)
	/// Proof: `VoterList::NextNodeAutoRebagged` (`max_values`: Some(1), `max_size`: Some(32), added: 527, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListBags` (r:200 w:4)
	/// Proof: `VoterList::ListBags` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	/// Storage: `VoterList::ListNodes` (r:11 w:11)
	/// Proof: `VoterList::ListNodes` (`max_values`: None, `max_size`: Some(154), added: 2629, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Bonded` (r:10 w:0)
	/// Proof: `Staking::Bonded` (`max_values`: None, `max_size`: Some(72), added: 2547, mode: `MaxEncodedLen`)
	/// Storage: `Staking::Ledger` (r:10 w:0)
	/// Proof: `Staking::Ledger` (`max_values`: None, `max_size`: Some(1091), added: 3566, mode: `MaxEncodedLen`)
	fn on_idle() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4920`
		//  Estimated: `512390`
		// Minimum execution time: 706_288_000 picoseconds.
		Weight::from_parts(716_009_000, 512390)
			.saturating_add(RocksDbWeight::get().reads(234_u64))
			.saturating_add(RocksDbWeight::get().writes(16_u64))
	}
}
