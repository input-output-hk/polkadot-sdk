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

//! Mock runtime for pallet-bags-lists tests.

use super::*;
use crate::{self as bags_list};
use frame_election_provider_support::VoteWeight;
use frame_support::{derive_impl, parameter_types};
use sp_runtime::BuildStorage;
use std::collections::HashMap;

pub type AccountId = <Runtime as frame_system::Config>::AccountId;
pub type Balance = u32;

parameter_types! {
	// Set the vote weight for any id who's weight has _not_ been set with `set_score_of`.
	pub static NextVoteWeightMap: HashMap<AccountId, VoteWeight> = Default::default();
}

pub struct StakingMock;
impl ScoreProvider<AccountId> for StakingMock {
	type Score = VoteWeight;

	fn score(id: &AccountId) -> Option<Self::Score> {
		NextVoteWeightMap::get().get(id).cloned()
	}

	frame_election_provider_support::runtime_benchmarks_or_std_enabled! {
		fn set_score_of(id: &AccountId, weight: Self::Score) {
			NEXT_VOTE_WEIGHT_MAP.with(|m| m.borrow_mut().insert(*id, weight));
		}
	}
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Runtime {
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
}

parameter_types! {
	pub static BagThresholds: &'static [VoteWeight] = &[10, 20, 30, 40, 50, 60, 1_000, 2_000, 10_000];
	pub static AutoRebagNumber: u32 = 10;
}

impl bags_list::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type ScoreProvider = StakingMock;
	type BagThresholds = BagThresholds;
	type MaxAutoRebagPerBlock = AutoRebagNumber;
	type Score = VoteWeight;
}

type Block = frame_system::mocking::MockBlock<Runtime>;
frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		BagsList: bags_list,
	}
);

/// Default AccountIds and their weights.
pub(crate) const GENESIS_IDS: [(AccountId, VoteWeight); 4] =
	[(1, 10), (2, 1_000), (3, 1_000), (4, 1_000)];

#[derive(Default)]
pub struct ExtBuilder {
	ids: Vec<(AccountId, VoteWeight)>,
	skip_genesis_ids: bool,
}

#[cfg(any(feature = "runtime-benchmarks", feature = "fuzz", test))]
impl ExtBuilder {
	/// Skip adding the default genesis ids to the list.
	#[cfg(test)]
	pub(crate) fn skip_genesis_ids(mut self) -> Self {
		self.skip_genesis_ids = true;
		self
	}

	/// Add some AccountIds to insert into `List`.
	#[cfg(test)]
	pub(crate) fn add_ids(mut self, ids: Vec<(AccountId, VoteWeight)>) -> Self {
		self.ids = ids;
		self
	}

	pub(crate) fn build(self) -> sp_io::TestExternalities {
		sp_tracing::try_init_simple();
		let storage = frame_system::GenesisConfig::<Runtime>::default().build_storage().unwrap();

		let ids_with_weight: Vec<_> = if self.skip_genesis_ids {
			self.ids.iter().collect()
		} else {
			GENESIS_IDS.iter().chain(self.ids.iter()).collect()
		};

		let mut ext = sp_io::TestExternalities::from(storage);
		ext.execute_with(|| {
			for (id, weight) in ids_with_weight {
				frame_support::assert_ok!(List::<Runtime>::insert(*id, *weight));
				StakingMock::set_score_of(id, *weight);
			}
		});

		ext
	}

	pub fn build_and_execute(self, test: impl FnOnce() -> ()) {
		self.build().execute_with(|| {
			test();
			List::<Runtime>::do_try_state().expect("do_try_state post condition failed")
		})
	}

	#[cfg(test)]
	pub(crate) fn build_and_execute_no_post_check(self, test: impl FnOnce() -> ()) {
		self.build().execute_with(test)
	}
}

#[cfg(test)]
pub(crate) mod test_utils {
	use super::*;
	use list::Bag;

	/// Returns the ordered ids within the given bag.
	pub(crate) fn bag_as_ids(bag: &Bag<Runtime>) -> Vec<AccountId> {
		bag.iter().map(|n| *n.id()).collect::<Vec<_>>()
	}

	/// Returns the ordered ids from the list.
	pub(crate) fn get_list_as_ids() -> Vec<AccountId> {
		List::<Runtime>::iter().map(|n| *n.id()).collect::<Vec<_>>()
	}
}
