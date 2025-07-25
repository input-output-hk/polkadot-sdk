// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

use crate::{
	configuration::{self, HostConfiguration},
	dmp, ensure_parachain, initializer, paras,
};
use alloc::{
	collections::{btree_map::BTreeMap, btree_set::BTreeSet},
	vec,
	vec::Vec,
};
use codec::{Decode, Encode};
use core::{fmt, mem};
use frame_support::{pallet_prelude::*, traits::ReservableCurrency, DefaultNoBound};
use frame_system::pallet_prelude::*;
use polkadot_parachain_primitives::primitives::{HorizontalMessages, IsSystem};
use polkadot_primitives::{
	Balance, Hash, HrmpChannelId, Id as ParaId, InboundHrmpMessage, OutboundHrmpMessage,
	SessionIndex,
};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{AccountIdConversion, BlakeTwo256, Hash as HashT, UniqueSaturatedInto, Zero},
	ArithmeticError,
};

pub use pallet::*;

/// Maximum bound that can be set for inbound channels.
///
/// If inaccurate, the weighing of this pallet might become inaccurate. It is expected form the
/// `configurations` pallet to check these values before setting
pub const HRMP_MAX_INBOUND_CHANNELS_BOUND: u32 = 128;
/// Same as [`HRMP_MAX_INBOUND_CHANNELS_BOUND`], but for outbound channels.
pub const HRMP_MAX_OUTBOUND_CHANNELS_BOUND: u32 = 128;

#[cfg(test)]
pub(crate) mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub trait WeightInfo {
	fn hrmp_init_open_channel() -> Weight;
	fn hrmp_accept_open_channel() -> Weight;
	fn hrmp_close_channel() -> Weight;
	fn force_clean_hrmp(i: u32, e: u32) -> Weight;
	fn force_process_hrmp_open(c: u32) -> Weight;
	fn force_process_hrmp_close(c: u32) -> Weight;
	fn hrmp_cancel_open_request(c: u32) -> Weight;
	fn clean_open_channel_requests(c: u32) -> Weight;
	fn force_open_hrmp_channel(c: u32) -> Weight;
	fn establish_system_channel() -> Weight;
	fn poke_channel_deposits() -> Weight;
	fn establish_channel_with_system() -> Weight;
}

/// A weight info that is only suitable for testing.
pub struct TestWeightInfo;

impl WeightInfo for TestWeightInfo {
	fn hrmp_accept_open_channel() -> Weight {
		Weight::MAX
	}
	fn force_clean_hrmp(_: u32, _: u32) -> Weight {
		Weight::MAX
	}
	fn force_process_hrmp_close(_: u32) -> Weight {
		Weight::MAX
	}
	fn force_process_hrmp_open(_: u32) -> Weight {
		Weight::MAX
	}
	fn hrmp_cancel_open_request(_: u32) -> Weight {
		Weight::MAX
	}
	fn hrmp_close_channel() -> Weight {
		Weight::MAX
	}
	fn hrmp_init_open_channel() -> Weight {
		Weight::MAX
	}
	fn clean_open_channel_requests(_: u32) -> Weight {
		Weight::MAX
	}
	fn force_open_hrmp_channel(_: u32) -> Weight {
		Weight::MAX
	}
	fn establish_system_channel() -> Weight {
		Weight::MAX
	}
	fn poke_channel_deposits() -> Weight {
		Weight::MAX
	}
	fn establish_channel_with_system() -> Weight {
		Weight::MAX
	}
}

/// A description of a request to open an HRMP channel.
#[derive(Encode, Decode, TypeInfo)]
pub struct HrmpOpenChannelRequest {
	/// Indicates if this request was confirmed by the recipient.
	pub confirmed: bool,
	/// NOTE: this field is deprecated. Channel open requests became non-expiring and this value
	/// became unused.
	pub _age: SessionIndex,
	/// The amount that the sender supplied at the time of creation of this request.
	pub sender_deposit: Balance,
	/// The maximum message size that could be put into the channel.
	pub max_message_size: u32,
	/// The maximum number of messages that can be pending in the channel at once.
	pub max_capacity: u32,
	/// The maximum total size of the messages that can be pending in the channel at once.
	pub max_total_size: u32,
}

/// A metadata of an HRMP channel.
#[derive(Encode, Decode, TypeInfo)]
#[cfg_attr(test, derive(Debug))]
pub struct HrmpChannel {
	// NOTE: This structure is used by parachains via merkle proofs. Therefore, this struct
	// requires special treatment.
	//
	// A parachain requested this struct can only depend on the subset of this struct.
	// Specifically, only a first few fields can be depended upon (See `AbridgedHrmpChannel`).
	// These fields cannot be changed without corresponding migration of parachains.
	/// The maximum number of messages that can be pending in the channel at once.
	pub max_capacity: u32,
	/// The maximum total size of the messages that can be pending in the channel at once.
	pub max_total_size: u32,
	/// The maximum message size that could be put into the channel.
	pub max_message_size: u32,
	/// The current number of messages pending in the channel.
	/// Invariant: should be less or equal to `max_capacity`.s`.
	pub msg_count: u32,
	/// The total size in bytes of all message payloads in the channel.
	/// Invariant: should be less or equal to `max_total_size`.
	pub total_size: u32,
	/// A head of the Message Queue Chain for this channel. Each link in this chain has a form:
	/// `(prev_head, B, H(M))`, where
	/// - `prev_head`: is the previous value of `mqc_head` or zero if none.
	/// - `B`: is the [relay-chain] block number in which a message was appended
	/// - `H(M)`: is the hash of the message being appended.
	/// This value is initialized to a special value that consists of all zeroes which indicates
	/// that no messages were previously added.
	pub mqc_head: Option<Hash>,
	/// The amount that the sender supplied as a deposit when opening this channel.
	pub sender_deposit: Balance,
	/// The amount that the recipient supplied as a deposit when accepting opening this channel.
	pub recipient_deposit: Balance,
}

/// An error returned by [`Pallet::check_hrmp_watermark`] that indicates an acceptance criteria
/// check didn't pass.
pub(crate) enum HrmpWatermarkAcceptanceErr<BlockNumber> {
	AdvancementRule { new_watermark: BlockNumber, last_watermark: BlockNumber },
	AheadRelayParent { new_watermark: BlockNumber, relay_chain_parent_number: BlockNumber },
	LandsOnBlockWithNoMessages { new_watermark: BlockNumber },
}

/// An error returned by [`Pallet::check_outbound_hrmp`] that indicates an acceptance criteria check
/// didn't pass.
pub(crate) enum OutboundHrmpAcceptanceErr {
	MoreMessagesThanPermitted { sent: u32, permitted: u32 },
	NotSorted { idx: u32 },
	NoSuchChannel { idx: u32, channel_id: HrmpChannelId },
	MaxMessageSizeExceeded { idx: u32, msg_size: u32, max_size: u32 },
	TotalSizeExceeded { idx: u32, total_size: u32, limit: u32 },
	CapacityExceeded { idx: u32, count: u32, limit: u32 },
}

impl<BlockNumber> fmt::Debug for HrmpWatermarkAcceptanceErr<BlockNumber>
where
	BlockNumber: fmt::Debug,
{
	fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
		use HrmpWatermarkAcceptanceErr::*;
		match self {
			AdvancementRule { new_watermark, last_watermark } => write!(
				fmt,
				"the HRMP watermark is not advanced relative to the last watermark ({:?} > {:?})",
				new_watermark, last_watermark,
			),
			AheadRelayParent { new_watermark, relay_chain_parent_number } => write!(
				fmt,
				"the HRMP watermark is ahead the relay-parent ({:?} > {:?})",
				new_watermark, relay_chain_parent_number
			),
			LandsOnBlockWithNoMessages { new_watermark } => write!(
				fmt,
				"the HRMP watermark ({:?}) doesn't land on a block with messages received",
				new_watermark
			),
		}
	}
}

impl fmt::Debug for OutboundHrmpAcceptanceErr {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
		use OutboundHrmpAcceptanceErr::*;
		match self {
			MoreMessagesThanPermitted { sent, permitted } => write!(
				fmt,
				"more HRMP messages than permitted by config ({} > {})",
				sent, permitted,
			),
			NotSorted { idx } => {
				write!(fmt, "the HRMP messages are not sorted (first unsorted is at index {})", idx,)
			},
			NoSuchChannel { idx, channel_id } => write!(
				fmt,
				"the HRMP message at index {} is sent to a non existent channel {:?}->{:?}",
				idx, channel_id.sender, channel_id.recipient,
			),
			MaxMessageSizeExceeded { idx, msg_size, max_size } => write!(
				fmt,
				"the HRMP message at index {} exceeds the negotiated channel maximum message size ({} > {})",
				idx, msg_size, max_size,
			),
			TotalSizeExceeded { idx, total_size, limit } => write!(
				fmt,
				"sending the HRMP message at index {} would exceed the negotiated channel total size  ({} > {})",
				idx, total_size, limit,
			),
			CapacityExceeded { idx, count, limit } => write!(
				fmt,
				"sending the HRMP message at index {} would exceed the negotiated channel capacity  ({} > {})",
				idx, count, limit,
			),
		}
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config + configuration::Config + paras::Config + dmp::Config
	{
		/// The outer event type.
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type RuntimeOrigin: From<crate::Origin>
			+ From<<Self as frame_system::Config>::RuntimeOrigin>
			+ Into<Result<crate::Origin, <Self as Config>::RuntimeOrigin>>;

		/// The origin that can perform "force" actions on channels.
		type ChannelManager: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

		/// An interface for reserving deposits for opening channels.
		///
		/// NOTE that this Currency instance will be charged with the amounts defined in the
		/// `Configuration` pallet. Specifically, that means that the `Balance` of the `Currency`
		/// implementation should be the same as `Balance` as used in the `Configuration`.
		type Currency: ReservableCurrency<Self::AccountId>;

		/// The default channel size and capacity to use when opening a channel to a system
		/// parachain.
		type DefaultChannelSizeAndCapacityWithSystem: Get<(u32, u32)>;

		/// Means of converting an `Xcm` into a `VersionedXcm`. This pallet sends HRMP XCM
		/// notifications to the channel-related parachains, while the `WrapVersion` implementation
		/// attempts to wrap them into the most suitable XCM version for the destination parachain.
		///
		/// NOTE: For example, `pallet_xcm` provides an accurate implementation (recommended), or
		/// the default `()` implementation uses the latest XCM version for all parachains.
		type VersionWrapper: xcm::WrapVersion;

		/// Something that provides the weight of this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Open HRMP channel requested.
		OpenChannelRequested {
			sender: ParaId,
			recipient: ParaId,
			proposed_max_capacity: u32,
			proposed_max_message_size: u32,
		},
		/// An HRMP channel request sent by the receiver was canceled by either party.
		OpenChannelCanceled { by_parachain: ParaId, channel_id: HrmpChannelId },
		/// Open HRMP channel accepted.
		OpenChannelAccepted { sender: ParaId, recipient: ParaId },
		/// HRMP channel closed.
		ChannelClosed { by_parachain: ParaId, channel_id: HrmpChannelId },
		/// An HRMP channel was opened via Root origin.
		HrmpChannelForceOpened {
			sender: ParaId,
			recipient: ParaId,
			proposed_max_capacity: u32,
			proposed_max_message_size: u32,
		},
		/// An HRMP channel was opened with a system chain.
		HrmpSystemChannelOpened {
			sender: ParaId,
			recipient: ParaId,
			proposed_max_capacity: u32,
			proposed_max_message_size: u32,
		},
		/// An HRMP channel's deposits were updated.
		OpenChannelDepositsUpdated { sender: ParaId, recipient: ParaId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The sender tried to open a channel to themselves.
		OpenHrmpChannelToSelf,
		/// The recipient is not a valid para.
		OpenHrmpChannelInvalidRecipient,
		/// The requested capacity is zero.
		OpenHrmpChannelZeroCapacity,
		/// The requested capacity exceeds the global limit.
		OpenHrmpChannelCapacityExceedsLimit,
		/// The requested maximum message size is 0.
		OpenHrmpChannelZeroMessageSize,
		/// The open request requested the message size that exceeds the global limit.
		OpenHrmpChannelMessageSizeExceedsLimit,
		/// The channel already exists
		OpenHrmpChannelAlreadyExists,
		/// There is already a request to open the same channel.
		OpenHrmpChannelAlreadyRequested,
		/// The sender already has the maximum number of allowed outbound channels.
		OpenHrmpChannelLimitExceeded,
		/// The channel from the sender to the origin doesn't exist.
		AcceptHrmpChannelDoesntExist,
		/// The channel is already confirmed.
		AcceptHrmpChannelAlreadyConfirmed,
		/// The recipient already has the maximum number of allowed inbound channels.
		AcceptHrmpChannelLimitExceeded,
		/// The origin tries to close a channel where it is neither the sender nor the recipient.
		CloseHrmpChannelUnauthorized,
		/// The channel to be closed doesn't exist.
		CloseHrmpChannelDoesntExist,
		/// The channel close request is already requested.
		CloseHrmpChannelAlreadyUnderway,
		/// Canceling is requested by neither the sender nor recipient of the open channel request.
		CancelHrmpOpenChannelUnauthorized,
		/// The open request doesn't exist.
		OpenHrmpChannelDoesntExist,
		/// Cannot cancel an HRMP open channel request because it is already confirmed.
		OpenHrmpChannelAlreadyConfirmed,
		/// The provided witness data is wrong.
		WrongWitness,
		/// The channel between these two chains cannot be authorized.
		ChannelCreationNotAuthorized,
	}

	/// The set of pending HRMP open channel requests.
	///
	/// The set is accompanied by a list for iteration.
	///
	/// Invariant:
	/// - There are no channels that exists in list but not in the set and vice versa.
	#[pallet::storage]
	pub type HrmpOpenChannelRequests<T: Config> =
		StorageMap<_, Twox64Concat, HrmpChannelId, HrmpOpenChannelRequest>;

	// NOTE: could become bounded, but we don't have a global maximum for this.
	// `HRMP_MAX_INBOUND_CHANNELS_BOUND` are per parachain, while this storage tracks the
	// global state.
	#[pallet::storage]
	pub type HrmpOpenChannelRequestsList<T: Config> =
		StorageValue<_, Vec<HrmpChannelId>, ValueQuery>;

	/// This mapping tracks how many open channel requests are initiated by a given sender para.
	/// Invariant: `HrmpOpenChannelRequests` should contain the same number of items that has
	/// `(X, _)` as the number of `HrmpOpenChannelRequestCount` for `X`.
	#[pallet::storage]
	pub type HrmpOpenChannelRequestCount<T: Config> =
		StorageMap<_, Twox64Concat, ParaId, u32, ValueQuery>;

	/// This mapping tracks how many open channel requests were accepted by a given recipient para.
	/// Invariant: `HrmpOpenChannelRequests` should contain the same number of items `(_, X)` with
	/// `confirmed` set to true, as the number of `HrmpAcceptedChannelRequestCount` for `X`.
	#[pallet::storage]
	pub type HrmpAcceptedChannelRequestCount<T: Config> =
		StorageMap<_, Twox64Concat, ParaId, u32, ValueQuery>;

	/// A set of pending HRMP close channel requests that are going to be closed during the session
	/// change. Used for checking if a given channel is registered for closure.
	///
	/// The set is accompanied by a list for iteration.
	///
	/// Invariant:
	/// - There are no channels that exists in list but not in the set and vice versa.
	#[pallet::storage]
	pub type HrmpCloseChannelRequests<T: Config> = StorageMap<_, Twox64Concat, HrmpChannelId, ()>;

	#[pallet::storage]
	pub type HrmpCloseChannelRequestsList<T: Config> =
		StorageValue<_, Vec<HrmpChannelId>, ValueQuery>;

	/// The HRMP watermark associated with each para.
	/// Invariant:
	/// - each para `P` used here as a key should satisfy `Paras::is_valid_para(P)` within a
	///   session.
	#[pallet::storage]
	pub type HrmpWatermarks<T: Config> = StorageMap<_, Twox64Concat, ParaId, BlockNumberFor<T>>;

	/// HRMP channel data associated with each para.
	/// Invariant:
	/// - each participant in the channel should satisfy `Paras::is_valid_para(P)` within a session.
	#[pallet::storage]
	pub type HrmpChannels<T: Config> = StorageMap<_, Twox64Concat, HrmpChannelId, HrmpChannel>;

	/// Ingress/egress indexes allow to find all the senders and receivers given the opposite side.
	/// I.e.
	///
	/// (a) ingress index allows to find all the senders for a given recipient.
	/// (b) egress index allows to find all the recipients for a given sender.
	///
	/// Invariants:
	/// - for each ingress index entry for `P` each item `I` in the index should present in
	///   `HrmpChannels` as `(I, P)`.
	/// - for each egress index entry for `P` each item `E` in the index should present in
	///   `HrmpChannels` as `(P, E)`.
	/// - there should be no other dangling channels in `HrmpChannels`.
	/// - the vectors are sorted.
	#[pallet::storage]
	pub type HrmpIngressChannelsIndex<T: Config> =
		StorageMap<_, Twox64Concat, ParaId, Vec<ParaId>, ValueQuery>;

	// NOTE that this field is used by parachains via merkle storage proofs, therefore changing
	// the format will require migration of parachains.
	#[pallet::storage]
	pub type HrmpEgressChannelsIndex<T: Config> =
		StorageMap<_, Twox64Concat, ParaId, Vec<ParaId>, ValueQuery>;

	/// Storage for the messages for each channel.
	/// Invariant: cannot be non-empty if the corresponding channel in `HrmpChannels` is `None`.
	#[pallet::storage]
	pub type HrmpChannelContents<T: Config> = StorageMap<
		_,
		Twox64Concat,
		HrmpChannelId,
		Vec<InboundHrmpMessage<BlockNumberFor<T>>>,
		ValueQuery,
	>;

	/// Maintains a mapping that can be used to answer the question: What paras sent a message at
	/// the given block number for a given receiver. Invariants:
	/// - The inner `Vec<ParaId>` is never empty.
	/// - The inner `Vec<ParaId>` cannot store two same `ParaId`.
	/// - The outer vector is sorted ascending by block number and cannot store two items with the
	///   same block number.
	#[pallet::storage]
	pub type HrmpChannelDigests<T: Config> =
		StorageMap<_, Twox64Concat, ParaId, Vec<(BlockNumberFor<T>, Vec<ParaId>)>, ValueQuery>;

	/// Preopen the given HRMP channels.
	///
	/// The values in the tuple corresponds to
	/// `(sender, recipient, max_capacity, max_message_size)`, i.e. similar to `init_open_channel`.
	/// In fact, the initialization is performed as if the `init_open_channel` and
	/// `accept_open_channel` were called with the respective parameters and the session change take
	///  place.
	///
	/// As such, each channel initializer should satisfy the same constraints, namely:
	///
	/// 1. `max_capacity` and `max_message_size` should be within the limits set by the
	///    configuration pallet.
	/// 2. `sender` and `recipient` must be valid paras.
	#[pallet::genesis_config]
	#[derive(DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		#[serde(skip)]
		_config: core::marker::PhantomData<T>,
		preopen_hrmp_channels: Vec<(ParaId, ParaId, u32, u32)>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			initialize_storage::<T>(&self.preopen_hrmp_channels);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Initiate opening a channel from a parachain to a given recipient with given channel
		/// parameters.
		///
		/// - `proposed_max_capacity` - specifies how many messages can be in the channel at once.
		/// - `proposed_max_message_size` - specifies the maximum size of the messages.
		///
		/// These numbers are a subject to the relay-chain configuration limits.
		///
		/// The channel can be opened only after the recipient confirms it and only on a session
		/// change.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::hrmp_init_open_channel())]
		pub fn hrmp_init_open_channel(
			origin: OriginFor<T>,
			recipient: ParaId,
			proposed_max_capacity: u32,
			proposed_max_message_size: u32,
		) -> DispatchResult {
			let origin = ensure_parachain(<T as Config>::RuntimeOrigin::from(origin))?;
			Self::init_open_channel(
				origin,
				recipient,
				proposed_max_capacity,
				proposed_max_message_size,
			)?;
			Self::deposit_event(Event::OpenChannelRequested {
				sender: origin,
				recipient,
				proposed_max_capacity,
				proposed_max_message_size,
			});
			Ok(())
		}

		/// Accept a pending open channel request from the given sender.
		///
		/// The channel will be opened only on the next session boundary.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::hrmp_accept_open_channel())]
		pub fn hrmp_accept_open_channel(origin: OriginFor<T>, sender: ParaId) -> DispatchResult {
			let origin = ensure_parachain(<T as Config>::RuntimeOrigin::from(origin))?;
			Self::accept_open_channel(origin, sender)?;
			Self::deposit_event(Event::OpenChannelAccepted { sender, recipient: origin });
			Ok(())
		}

		/// Initiate unilateral closing of a channel. The origin must be either the sender or the
		/// recipient in the channel being closed.
		///
		/// The closure can only happen on a session change.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::hrmp_close_channel())]
		pub fn hrmp_close_channel(
			origin: OriginFor<T>,
			channel_id: HrmpChannelId,
		) -> DispatchResult {
			let origin = ensure_parachain(<T as Config>::RuntimeOrigin::from(origin))?;
			Self::close_channel(origin, channel_id.clone())?;
			Self::deposit_event(Event::ChannelClosed { by_parachain: origin, channel_id });
			Ok(())
		}

		/// This extrinsic triggers the cleanup of all the HRMP storage items that a para may have.
		/// Normally this happens once per session, but this allows you to trigger the cleanup
		/// immediately for a specific parachain.
		///
		/// Number of inbound and outbound channels for `para` must be provided as witness data.
		///
		/// Origin must be the `ChannelManager`.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::force_clean_hrmp(*num_inbound, *num_outbound))]
		pub fn force_clean_hrmp(
			origin: OriginFor<T>,
			para: ParaId,
			num_inbound: u32,
			num_outbound: u32,
		) -> DispatchResult {
			T::ChannelManager::ensure_origin(origin)?;

			ensure!(
				HrmpIngressChannelsIndex::<T>::decode_len(para).unwrap_or_default() <=
					num_inbound as usize,
				Error::<T>::WrongWitness
			);
			ensure!(
				HrmpEgressChannelsIndex::<T>::decode_len(para).unwrap_or_default() <=
					num_outbound as usize,
				Error::<T>::WrongWitness
			);

			Self::clean_hrmp_after_outgoing(&para);
			Ok(())
		}

		/// Force process HRMP open channel requests.
		///
		/// If there are pending HRMP open channel requests, you can use this function to process
		/// all of those requests immediately.
		///
		/// Total number of opening channels must be provided as witness data.
		///
		/// Origin must be the `ChannelManager`.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::force_process_hrmp_open(*channels))]
		pub fn force_process_hrmp_open(origin: OriginFor<T>, channels: u32) -> DispatchResult {
			T::ChannelManager::ensure_origin(origin)?;

			ensure!(
				HrmpOpenChannelRequestsList::<T>::decode_len().unwrap_or_default() as u32 <=
					channels,
				Error::<T>::WrongWitness
			);

			let host_config = configuration::ActiveConfig::<T>::get();
			Self::process_hrmp_open_channel_requests(&host_config);
			Ok(())
		}

		/// Force process HRMP close channel requests.
		///
		/// If there are pending HRMP close channel requests, you can use this function to process
		/// all of those requests immediately.
		///
		/// Total number of closing channels must be provided as witness data.
		///
		/// Origin must be the `ChannelManager`.
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::force_process_hrmp_close(*channels))]
		pub fn force_process_hrmp_close(origin: OriginFor<T>, channels: u32) -> DispatchResult {
			T::ChannelManager::ensure_origin(origin)?;

			ensure!(
				HrmpCloseChannelRequestsList::<T>::decode_len().unwrap_or_default() as u32 <=
					channels,
				Error::<T>::WrongWitness
			);

			Self::process_hrmp_close_channel_requests();
			Ok(())
		}

		/// This cancels a pending open channel request. It can be canceled by either of the sender
		/// or the recipient for that request. The origin must be either of those.
		///
		/// The cancellation happens immediately. It is not possible to cancel the request if it is
		/// already accepted.
		///
		/// Total number of open requests (i.e. `HrmpOpenChannelRequestsList`) must be provided as
		/// witness data.
		#[pallet::call_index(6)]
		#[pallet::weight(<T as Config>::WeightInfo::hrmp_cancel_open_request(*open_requests))]
		pub fn hrmp_cancel_open_request(
			origin: OriginFor<T>,
			channel_id: HrmpChannelId,
			open_requests: u32,
		) -> DispatchResult {
			let origin = ensure_parachain(<T as Config>::RuntimeOrigin::from(origin))?;
			ensure!(
				HrmpOpenChannelRequestsList::<T>::decode_len().unwrap_or_default() as u32 <=
					open_requests,
				Error::<T>::WrongWitness
			);
			Self::cancel_open_request(origin, channel_id.clone())?;
			Self::deposit_event(Event::OpenChannelCanceled { by_parachain: origin, channel_id });
			Ok(())
		}

		/// Open a channel from a `sender` to a `recipient` `ParaId`. Although opened by governance,
		/// the `max_capacity` and `max_message_size` are still subject to the Relay Chain's
		/// configured limits.
		///
		/// Expected use is when one (and only one) of the `ParaId`s involved in the channel is
		/// governed by the system, e.g. a system parachain.
		///
		/// Origin must be the `ChannelManager`.
		#[pallet::call_index(7)]
		#[pallet::weight(<T as Config>::WeightInfo::force_open_hrmp_channel(1))]
		pub fn force_open_hrmp_channel(
			origin: OriginFor<T>,
			sender: ParaId,
			recipient: ParaId,
			max_capacity: u32,
			max_message_size: u32,
		) -> DispatchResultWithPostInfo {
			T::ChannelManager::ensure_origin(origin)?;

			// Guard against a common footgun where someone makes a channel request to a system
			// parachain and then makes a proposal to open the channel via governance, which fails
			// because `init_open_channel` fails if there is an existing request. This check will
			// clear an existing request such that `init_open_channel` should otherwise succeed.
			let channel_id = HrmpChannelId { sender, recipient };
			let cancel_request: u32 =
				if let Some(_open_channel) = HrmpOpenChannelRequests::<T>::get(&channel_id) {
					Self::cancel_open_request(sender, channel_id)?;
					1
				} else {
					0
				};

			// Now we proceed with normal init/accept, except that we set `no_deposit` to true such
			// that it will not require deposits from either member.
			Self::init_open_channel(sender, recipient, max_capacity, max_message_size)?;
			Self::accept_open_channel(recipient, sender)?;
			Self::deposit_event(Event::HrmpChannelForceOpened {
				sender,
				recipient,
				proposed_max_capacity: max_capacity,
				proposed_max_message_size: max_message_size,
			});

			Ok(Some(<T as Config>::WeightInfo::force_open_hrmp_channel(cancel_request)).into())
		}

		/// Establish an HRMP channel between two system chains. If the channel does not already
		/// exist, the transaction fees will be refunded to the caller. The system does not take
		/// deposits for channels between system chains, and automatically sets the message number
		/// and size limits to the maximum allowed by the network's configuration.
		///
		/// Arguments:
		///
		/// - `sender`: A system chain, `ParaId`.
		/// - `recipient`: A system chain, `ParaId`.
		///
		/// Any signed origin can call this function, but _both_ inputs MUST be system chains. If
		/// the channel does not exist yet, there is no fee.
		#[pallet::call_index(8)]
		#[pallet::weight(<T as Config>::WeightInfo::establish_system_channel())]
		pub fn establish_system_channel(
			origin: OriginFor<T>,
			sender: ParaId,
			recipient: ParaId,
		) -> DispatchResultWithPostInfo {
			let _caller = ensure_signed(origin)?;

			// both chains must be system
			ensure!(
				sender.is_system() && recipient.is_system(),
				Error::<T>::ChannelCreationNotAuthorized
			);

			let config = configuration::ActiveConfig::<T>::get();
			let max_message_size = config.hrmp_channel_max_message_size;
			let max_capacity = config.hrmp_channel_max_capacity;

			Self::init_open_channel(sender, recipient, max_capacity, max_message_size)?;
			Self::accept_open_channel(recipient, sender)?;

			Self::deposit_event(Event::HrmpSystemChannelOpened {
				sender,
				recipient,
				proposed_max_capacity: max_capacity,
				proposed_max_message_size: max_message_size,
			});

			Ok(Pays::No.into())
		}

		/// Update the deposits held for an HRMP channel to the latest `Configuration`. Channels
		/// with system chains do not require a deposit.
		///
		/// Arguments:
		///
		/// - `sender`: A chain, `ParaId`.
		/// - `recipient`: A chain, `ParaId`.
		///
		/// Any signed origin can call this function.
		#[pallet::call_index(9)]
		#[pallet::weight(<T as Config>::WeightInfo::poke_channel_deposits())]
		pub fn poke_channel_deposits(
			origin: OriginFor<T>,
			sender: ParaId,
			recipient: ParaId,
		) -> DispatchResult {
			let _caller = ensure_signed(origin)?;
			let channel_id = HrmpChannelId { sender, recipient };
			let is_system = sender.is_system() || recipient.is_system();

			let config = configuration::ActiveConfig::<T>::get();

			// Channels with and amongst the system do not require a deposit.
			let (new_sender_deposit, new_recipient_deposit) = if is_system {
				(0, 0)
			} else {
				(config.hrmp_sender_deposit, config.hrmp_recipient_deposit)
			};

			HrmpChannels::<T>::mutate(&channel_id, |channel| -> DispatchResult {
				if let Some(ref mut channel) = channel {
					let current_sender_deposit = channel.sender_deposit;
					let current_recipient_deposit = channel.recipient_deposit;

					// nothing to update
					if current_sender_deposit == new_sender_deposit &&
						current_recipient_deposit == new_recipient_deposit
					{
						return Ok(())
					}

					// sender
					if current_sender_deposit > new_sender_deposit {
						// Can never underflow, but be paranoid.
						let amount = current_sender_deposit
							.checked_sub(new_sender_deposit)
							.ok_or(ArithmeticError::Underflow)?;
						T::Currency::unreserve(
							&channel_id.sender.into_account_truncating(),
							// The difference should always be convertible into `Balance`, but be
							// paranoid and do nothing in case.
							amount.try_into().unwrap_or(Zero::zero()),
						);
					} else if current_sender_deposit < new_sender_deposit {
						let amount = new_sender_deposit
							.checked_sub(current_sender_deposit)
							.ok_or(ArithmeticError::Underflow)?;
						T::Currency::reserve(
							&channel_id.sender.into_account_truncating(),
							amount.try_into().unwrap_or(Zero::zero()),
						)?;
					}

					// recipient
					if current_recipient_deposit > new_recipient_deposit {
						let amount = current_recipient_deposit
							.checked_sub(new_recipient_deposit)
							.ok_or(ArithmeticError::Underflow)?;
						T::Currency::unreserve(
							&channel_id.recipient.into_account_truncating(),
							amount.try_into().unwrap_or(Zero::zero()),
						);
					} else if current_recipient_deposit < new_recipient_deposit {
						let amount = new_recipient_deposit
							.checked_sub(current_recipient_deposit)
							.ok_or(ArithmeticError::Underflow)?;
						T::Currency::reserve(
							&channel_id.recipient.into_account_truncating(),
							amount.try_into().unwrap_or(Zero::zero()),
						)?;
					}

					// update storage
					channel.sender_deposit = new_sender_deposit;
					channel.recipient_deposit = new_recipient_deposit;
				} else {
					return Err(Error::<T>::OpenHrmpChannelDoesntExist.into())
				}
				Ok(())
			})?;

			Self::deposit_event(Event::OpenChannelDepositsUpdated { sender, recipient });

			Ok(())
		}

		/// Establish a bidirectional HRMP channel between a parachain and a system chain.
		///
		/// Arguments:
		///
		/// - `target_system_chain`: A system chain, `ParaId`.
		///
		/// The origin needs to be the parachain origin.
		#[pallet::call_index(10)]
		#[pallet::weight(<T as Config>::WeightInfo::establish_channel_with_system())]
		pub fn establish_channel_with_system(
			origin: OriginFor<T>,
			target_system_chain: ParaId,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_parachain(<T as Config>::RuntimeOrigin::from(origin))?;

			ensure!(target_system_chain.is_system(), Error::<T>::ChannelCreationNotAuthorized);

			let (max_message_size, max_capacity) =
				T::DefaultChannelSizeAndCapacityWithSystem::get();

			// create bidirectional channel
			Self::init_open_channel(sender, target_system_chain, max_capacity, max_message_size)?;
			Self::accept_open_channel(target_system_chain, sender)?;

			Self::init_open_channel(target_system_chain, sender, max_capacity, max_message_size)?;
			Self::accept_open_channel(sender, target_system_chain)?;

			Self::deposit_event(Event::HrmpSystemChannelOpened {
				sender,
				recipient: target_system_chain,
				proposed_max_capacity: max_capacity,
				proposed_max_message_size: max_message_size,
			});

			Self::deposit_event(Event::HrmpSystemChannelOpened {
				sender: target_system_chain,
				recipient: sender,
				proposed_max_capacity: max_capacity,
				proposed_max_message_size: max_message_size,
			});

			Ok(Pays::No.into())
		}
	}
}

fn initialize_storage<T: Config>(preopen_hrmp_channels: &[(ParaId, ParaId, u32, u32)]) {
	let host_config = configuration::ActiveConfig::<T>::get();
	for &(sender, recipient, max_capacity, max_message_size) in preopen_hrmp_channels {
		if let Err(err) =
			preopen_hrmp_channel::<T>(sender, recipient, max_capacity, max_message_size)
		{
			panic!("failed to initialize the genesis storage: {:?}", err);
		}
	}
	Pallet::<T>::process_hrmp_open_channel_requests(&host_config);
}

fn preopen_hrmp_channel<T: Config>(
	sender: ParaId,
	recipient: ParaId,
	max_capacity: u32,
	max_message_size: u32,
) -> DispatchResult {
	Pallet::<T>::init_open_channel(sender, recipient, max_capacity, max_message_size)?;
	Pallet::<T>::accept_open_channel(recipient, sender)?;
	Ok(())
}

/// Routines and getters related to HRMP.
impl<T: Config> Pallet<T> {
	/// Block initialization logic, called by initializer.
	pub(crate) fn initializer_initialize(_now: BlockNumberFor<T>) -> Weight {
		Weight::zero()
	}

	/// Block finalization logic, called by initializer.
	pub(crate) fn initializer_finalize() {}

	/// Called by the initializer to note that a new session has started.
	pub(crate) fn initializer_on_new_session(
		notification: &initializer::SessionChangeNotification<BlockNumberFor<T>>,
		outgoing_paras: &[ParaId],
	) -> Weight {
		let w1 = Self::perform_outgoing_para_cleanup(&notification.prev_config, outgoing_paras);
		Self::process_hrmp_open_channel_requests(&notification.prev_config);
		Self::process_hrmp_close_channel_requests();
		w1.saturating_add(<T as Config>::WeightInfo::force_process_hrmp_open(
			outgoing_paras.len() as u32
		))
		.saturating_add(<T as Config>::WeightInfo::force_process_hrmp_close(
			outgoing_paras.len() as u32,
		))
	}

	/// Iterate over all paras that were noted for offboarding and remove all the data
	/// associated with them.
	fn perform_outgoing_para_cleanup(
		config: &HostConfiguration<BlockNumberFor<T>>,
		outgoing: &[ParaId],
	) -> Weight {
		let mut w = Self::clean_open_channel_requests(config, outgoing);
		for outgoing_para in outgoing {
			Self::clean_hrmp_after_outgoing(outgoing_para);

			// we need a few extra bits of data to weigh this -- all of this is read internally
			// anyways, so no overhead.
			let ingress_count =
				HrmpIngressChannelsIndex::<T>::decode_len(outgoing_para).unwrap_or_default() as u32;
			let egress_count =
				HrmpEgressChannelsIndex::<T>::decode_len(outgoing_para).unwrap_or_default() as u32;
			w = w.saturating_add(<T as Config>::WeightInfo::force_clean_hrmp(
				ingress_count,
				egress_count,
			));
		}
		w
	}

	// Go over the HRMP open channel requests and remove all in which offboarding paras participate.
	//
	// This will also perform the refunds for the counterparty if it doesn't offboard.
	pub(crate) fn clean_open_channel_requests(
		config: &HostConfiguration<BlockNumberFor<T>>,
		outgoing: &[ParaId],
	) -> Weight {
		// First collect all the channel ids of the open requests in which there is at least one
		// party presents in the outgoing list.
		//
		// Both the open channel request list and outgoing list are expected to be small enough.
		// In the most common case there will be only single outgoing para.
		let open_channel_reqs = HrmpOpenChannelRequestsList::<T>::get();
		let (go, stay): (Vec<HrmpChannelId>, Vec<HrmpChannelId>) = open_channel_reqs
			.into_iter()
			.partition(|req_id| outgoing.iter().any(|id| req_id.is_participant(*id)));
		HrmpOpenChannelRequestsList::<T>::put(stay);

		// Then iterate over all open requests to be removed, pull them out of the set and perform
		// the refunds if applicable.
		for req_id in go {
			let req_data = match HrmpOpenChannelRequests::<T>::take(&req_id) {
				Some(req_data) => req_data,
				None => {
					// Can't normally happen but no need to panic.
					continue
				},
			};

			// Return the deposit of the sender, but only if it is not the para being offboarded.
			if !outgoing.contains(&req_id.sender) {
				T::Currency::unreserve(
					&req_id.sender.into_account_truncating(),
					req_data.sender_deposit.unique_saturated_into(),
				);
			}

			// If the request was confirmed, then it means it was confirmed in the finished session.
			// Therefore, the config's hrmp_recipient_deposit represents the actual value of the
			// deposit.
			//
			// We still want to refund the deposit only if the para is not being offboarded.
			if req_data.confirmed {
				if !outgoing.contains(&req_id.recipient) {
					T::Currency::unreserve(
						&req_id.recipient.into_account_truncating(),
						config.hrmp_recipient_deposit.unique_saturated_into(),
					);
				}
				Self::decrease_accepted_channel_request_count(req_id.recipient);
			}
		}

		<T as Config>::WeightInfo::clean_open_channel_requests(outgoing.len() as u32)
	}

	/// Remove all storage entries associated with the given para.
	fn clean_hrmp_after_outgoing(outgoing_para: &ParaId) {
		HrmpOpenChannelRequestCount::<T>::remove(outgoing_para);
		HrmpAcceptedChannelRequestCount::<T>::remove(outgoing_para);

		let ingress = HrmpIngressChannelsIndex::<T>::take(outgoing_para)
			.into_iter()
			.map(|sender| HrmpChannelId { sender, recipient: *outgoing_para });
		let egress = HrmpEgressChannelsIndex::<T>::take(outgoing_para)
			.into_iter()
			.map(|recipient| HrmpChannelId { sender: *outgoing_para, recipient });
		let mut to_close = ingress.chain(egress).collect::<Vec<_>>();
		to_close.sort();
		to_close.dedup();

		for channel in to_close {
			Self::close_hrmp_channel(&channel);
		}
	}

	/// Iterate over all open channel requests and:
	///
	/// - prune the stale requests
	/// - enact the confirmed requests
	fn process_hrmp_open_channel_requests(config: &HostConfiguration<BlockNumberFor<T>>) {
		let mut open_req_channels = HrmpOpenChannelRequestsList::<T>::get();
		if open_req_channels.is_empty() {
			return
		}

		// iterate the vector starting from the end making our way to the beginning. This way we
		// can leverage `swap_remove` to efficiently remove an item during iteration.
		let mut idx = open_req_channels.len();
		loop {
			// bail if we've iterated over all items.
			if idx == 0 {
				break
			}

			idx -= 1;
			let channel_id = open_req_channels[idx].clone();
			let request = HrmpOpenChannelRequests::<T>::get(&channel_id).expect(
				"can't be `None` due to the invariant that the list contains the same items as the set; qed",
			);

			let system_channel = channel_id.sender.is_system() || channel_id.recipient.is_system();
			let sender_deposit = request.sender_deposit;
			let recipient_deposit = if system_channel { 0 } else { config.hrmp_recipient_deposit };

			if request.confirmed {
				if paras::Pallet::<T>::is_valid_para(channel_id.sender) &&
					paras::Pallet::<T>::is_valid_para(channel_id.recipient)
				{
					HrmpChannels::<T>::insert(
						&channel_id,
						HrmpChannel {
							sender_deposit,
							recipient_deposit,
							max_capacity: request.max_capacity,
							max_total_size: request.max_total_size,
							max_message_size: request.max_message_size,
							msg_count: 0,
							total_size: 0,
							mqc_head: None,
						},
					);

					HrmpIngressChannelsIndex::<T>::mutate(&channel_id.recipient, |v| {
						if let Err(i) = v.binary_search(&channel_id.sender) {
							v.insert(i, channel_id.sender);
						}
					});
					HrmpEgressChannelsIndex::<T>::mutate(&channel_id.sender, |v| {
						if let Err(i) = v.binary_search(&channel_id.recipient) {
							v.insert(i, channel_id.recipient);
						}
					});
				}

				Self::decrease_open_channel_request_count(channel_id.sender);
				Self::decrease_accepted_channel_request_count(channel_id.recipient);

				let _ = open_req_channels.swap_remove(idx);
				HrmpOpenChannelRequests::<T>::remove(&channel_id);
			}
		}

		HrmpOpenChannelRequestsList::<T>::put(open_req_channels);
	}

	/// Iterate over all close channel requests unconditionally closing the channels.
	fn process_hrmp_close_channel_requests() {
		let close_reqs = HrmpCloseChannelRequestsList::<T>::take();
		for condemned_ch_id in close_reqs {
			HrmpCloseChannelRequests::<T>::remove(&condemned_ch_id);
			Self::close_hrmp_channel(&condemned_ch_id);
		}
	}

	/// Close and remove the designated HRMP channel.
	///
	/// This includes returning the deposits.
	///
	/// This function is idempotent, meaning that after the first application it should have no
	/// effect (i.e. it won't return the deposits twice).
	fn close_hrmp_channel(channel_id: &HrmpChannelId) {
		if let Some(HrmpChannel { sender_deposit, recipient_deposit, .. }) =
			HrmpChannels::<T>::take(channel_id)
		{
			T::Currency::unreserve(
				&channel_id.sender.into_account_truncating(),
				sender_deposit.unique_saturated_into(),
			);
			T::Currency::unreserve(
				&channel_id.recipient.into_account_truncating(),
				recipient_deposit.unique_saturated_into(),
			);
		}

		HrmpChannelContents::<T>::remove(channel_id);

		HrmpEgressChannelsIndex::<T>::mutate(&channel_id.sender, |v| {
			if let Ok(i) = v.binary_search(&channel_id.recipient) {
				v.remove(i);
			}
		});
		HrmpIngressChannelsIndex::<T>::mutate(&channel_id.recipient, |v| {
			if let Ok(i) = v.binary_search(&channel_id.sender) {
				v.remove(i);
			}
		});
	}

	/// Check that the candidate of the given recipient controls the HRMP watermark properly.
	pub(crate) fn check_hrmp_watermark(
		recipient: ParaId,
		relay_chain_parent_number: BlockNumberFor<T>,
		new_hrmp_watermark: BlockNumberFor<T>,
	) -> Result<(), HrmpWatermarkAcceptanceErr<BlockNumberFor<T>>> {
		// First, check where the watermark CANNOT legally land.
		//
		// (a) For ensuring that messages are eventually processed, we require each parablock's
		//     watermark to be greater than the last one. The exception to this is if the previous
		//     watermark was already equal to the current relay-parent number.
		//
		// (b) However, a parachain cannot read into "the future", therefore the watermark should
		//     not be greater than the relay-chain context block which the parablock refers to.
		if new_hrmp_watermark == relay_chain_parent_number {
			return Ok(())
		}

		if new_hrmp_watermark > relay_chain_parent_number {
			return Err(HrmpWatermarkAcceptanceErr::AheadRelayParent {
				new_watermark: new_hrmp_watermark,
				relay_chain_parent_number,
			})
		}

		if let Some(last_watermark) = HrmpWatermarks::<T>::get(&recipient) {
			if new_hrmp_watermark < last_watermark {
				return Err(HrmpWatermarkAcceptanceErr::AdvancementRule {
					new_watermark: new_hrmp_watermark,
					last_watermark,
				})
			}

			if new_hrmp_watermark == last_watermark {
				return Ok(())
			}
		}

		// Second, check where the watermark CAN land. It's one of the following:
		//
		// (a) The relay parent block number (checked above).
		// (b) A relay-chain block in which this para received at least one message (checked here)
		let digest = HrmpChannelDigests::<T>::get(&recipient);
		if !digest
			.binary_search_by_key(&new_hrmp_watermark, |(block_no, _)| *block_no)
			.is_ok()
		{
			return Err(HrmpWatermarkAcceptanceErr::LandsOnBlockWithNoMessages {
				new_watermark: new_hrmp_watermark,
			})
		}
		Ok(())
	}

	/// Returns HRMP watermarks of previously sent messages to a given para.
	pub(crate) fn valid_watermarks(recipient: ParaId) -> Vec<BlockNumberFor<T>> {
		let mut valid_watermarks: Vec<_> = HrmpChannelDigests::<T>::get(&recipient)
			.into_iter()
			.map(|(block_no, _)| block_no)
			.collect();

		// The current watermark will remain valid until updated.
		if let Some(last_watermark) = HrmpWatermarks::<T>::get(&recipient) {
			if valid_watermarks.first().map_or(false, |w| w > &last_watermark) {
				valid_watermarks.insert(0, last_watermark);
			}
		}

		valid_watermarks
	}

	pub(crate) fn check_outbound_hrmp(
		config: &HostConfiguration<BlockNumberFor<T>>,
		sender: ParaId,
		out_hrmp_msgs: &[OutboundHrmpMessage<ParaId>],
	) -> Result<(), OutboundHrmpAcceptanceErr> {
		if out_hrmp_msgs.len() as u32 > config.hrmp_max_message_num_per_candidate {
			return Err(OutboundHrmpAcceptanceErr::MoreMessagesThanPermitted {
				sent: out_hrmp_msgs.len() as u32,
				permitted: config.hrmp_max_message_num_per_candidate,
			})
		}

		let mut last_recipient = None::<ParaId>;

		for (idx, out_msg) in
			out_hrmp_msgs.iter().enumerate().map(|(idx, out_msg)| (idx as u32, out_msg))
		{
			match last_recipient {
				// the messages must be sorted in ascending order and there must be no two messages
				// sent to the same recipient. Thus we can check that every recipient is strictly
				// greater than the previous one.
				Some(last_recipient) if out_msg.recipient <= last_recipient =>
					return Err(OutboundHrmpAcceptanceErr::NotSorted { idx }),
				_ => last_recipient = Some(out_msg.recipient),
			}

			let channel_id = HrmpChannelId { sender, recipient: out_msg.recipient };

			let channel = match HrmpChannels::<T>::get(&channel_id) {
				Some(channel) => channel,
				None => return Err(OutboundHrmpAcceptanceErr::NoSuchChannel { channel_id, idx }),
			};

			let msg_size = out_msg.data.len() as u32;
			if msg_size > channel.max_message_size {
				return Err(OutboundHrmpAcceptanceErr::MaxMessageSizeExceeded {
					idx,
					msg_size,
					max_size: channel.max_message_size,
				})
			}

			let new_total_size = channel.total_size + out_msg.data.len() as u32;
			if new_total_size > channel.max_total_size {
				return Err(OutboundHrmpAcceptanceErr::TotalSizeExceeded {
					idx,
					total_size: new_total_size,
					limit: channel.max_total_size,
				})
			}

			let new_msg_count = channel.msg_count + 1;
			if new_msg_count > channel.max_capacity {
				return Err(OutboundHrmpAcceptanceErr::CapacityExceeded {
					idx,
					count: new_msg_count,
					limit: channel.max_capacity,
				})
			}
		}

		Ok(())
	}

	/// Returns remaining outbound channels capacity in messages and in bytes per recipient para.
	pub(crate) fn outbound_remaining_capacity(sender: ParaId) -> Vec<(ParaId, (u32, u32))> {
		let recipients = HrmpEgressChannelsIndex::<T>::get(&sender);
		let mut remaining = Vec::with_capacity(recipients.len());

		for recipient in recipients {
			let Some(channel) = HrmpChannels::<T>::get(&HrmpChannelId { sender, recipient }) else {
				continue
			};
			remaining.push((
				recipient,
				(
					channel.max_capacity - channel.msg_count,
					channel.max_total_size - channel.total_size,
				),
			));
		}

		remaining
	}

	pub(crate) fn prune_hrmp(recipient: ParaId, new_hrmp_watermark: BlockNumberFor<T>) {
		// sift through the incoming messages digest to collect the paras that sent at least one
		// message to this parachain between the old and new watermarks.
		let senders = HrmpChannelDigests::<T>::mutate(&recipient, |digest| {
			let mut senders = BTreeSet::new();
			let mut leftover = Vec::with_capacity(digest.len());
			for (block_no, paras_sent_msg) in mem::replace(digest, Vec::new()) {
				if block_no <= new_hrmp_watermark {
					senders.extend(paras_sent_msg);
				} else {
					leftover.push((block_no, paras_sent_msg));
				}
			}
			*digest = leftover;
			senders
		});

		// having all senders we can trivially find out the channels which we need to prune.
		let channels_to_prune =
			senders.into_iter().map(|sender| HrmpChannelId { sender, recipient });
		for channel_id in channels_to_prune {
			// prune each channel up to the new watermark keeping track how many messages we removed
			// and what is the total byte size of them.
			let (mut pruned_cnt, mut pruned_size) = (0, 0);

			let contents = HrmpChannelContents::<T>::get(&channel_id);
			let mut leftover = Vec::with_capacity(contents.len());
			for msg in contents {
				if msg.sent_at <= new_hrmp_watermark {
					pruned_cnt += 1;
					pruned_size += msg.data.len();
				} else {
					leftover.push(msg);
				}
			}
			if !leftover.is_empty() {
				HrmpChannelContents::<T>::insert(&channel_id, leftover);
			} else {
				HrmpChannelContents::<T>::remove(&channel_id);
			}

			// update the channel metadata.
			HrmpChannels::<T>::mutate(&channel_id, |channel| {
				if let Some(ref mut channel) = channel {
					channel.msg_count -= pruned_cnt as u32;
					channel.total_size -= pruned_size as u32;
				}
			});
		}

		HrmpWatermarks::<T>::insert(&recipient, new_hrmp_watermark);
	}

	/// Process the outbound HRMP messages by putting them into the appropriate recipient queues.
	pub(crate) fn queue_outbound_hrmp(sender: ParaId, out_hrmp_msgs: HorizontalMessages) {
		let now = frame_system::Pallet::<T>::block_number();

		for out_msg in out_hrmp_msgs {
			let channel_id = HrmpChannelId { sender, recipient: out_msg.recipient };

			let mut channel = match HrmpChannels::<T>::get(&channel_id) {
				Some(channel) => channel,
				None => {
					// apparently, that since acceptance of this candidate the recipient was
					// offboarded and the channel no longer exists.
					continue
				},
			};

			let inbound = InboundHrmpMessage { sent_at: now, data: out_msg.data };

			// book keeping
			channel.msg_count += 1;
			channel.total_size += inbound.data.len() as u32;

			// compute the new MQC head of the channel
			let prev_head = channel.mqc_head.unwrap_or(Default::default());
			let new_head = BlakeTwo256::hash_of(&(
				prev_head,
				inbound.sent_at,
				T::Hashing::hash_of(&inbound.data),
			));
			channel.mqc_head = Some(new_head);

			HrmpChannels::<T>::insert(&channel_id, channel);
			HrmpChannelContents::<T>::append(&channel_id, inbound);

			// The digests are sorted in ascending by block number order. There are only two
			// possible scenarios here ("the current" is the block of candidate's inclusion):
			//
			// (a) It's the first time anybody sends a message to this recipient within this block.
			//     In this case, the digest vector would be empty or the block number of the latest
			//     entry is smaller than the current.
			//
			// (b) Somebody has already sent a message within the current block. That means that
			//     the block number of the latest entry is equal to the current.
			//
			// Note that having the latest entry greater than the current block number is a logical
			// error.
			let mut recipient_digest = HrmpChannelDigests::<T>::get(&channel_id.recipient);
			if let Some(cur_block_digest) = recipient_digest
				.last_mut()
				.filter(|(block_no, _)| *block_no == now)
				.map(|(_, ref mut d)| d)
			{
				cur_block_digest.push(sender);
			} else {
				recipient_digest.push((now, vec![sender]));
			}
			HrmpChannelDigests::<T>::insert(&channel_id.recipient, recipient_digest);
		}
	}

	/// Initiate opening a channel from a parachain to a given recipient with given channel
	/// parameters. If neither chain is part of the system, then a deposit from the `Configuration`
	/// will be required for `origin` (the sender) upon opening the request and the `recipient` upon
	/// accepting it.
	///
	/// Basically the same as [`hrmp_init_open_channel`](Pallet::hrmp_init_open_channel) but
	/// intended for calling directly from other pallets rather than dispatched.
	pub fn init_open_channel(
		origin: ParaId,
		recipient: ParaId,
		proposed_max_capacity: u32,
		proposed_max_message_size: u32,
	) -> DispatchResult {
		ensure!(origin != recipient, Error::<T>::OpenHrmpChannelToSelf);
		ensure!(
			paras::Pallet::<T>::is_valid_para(recipient),
			Error::<T>::OpenHrmpChannelInvalidRecipient,
		);

		let config = configuration::ActiveConfig::<T>::get();
		ensure!(proposed_max_capacity > 0, Error::<T>::OpenHrmpChannelZeroCapacity);
		ensure!(
			proposed_max_capacity <= config.hrmp_channel_max_capacity,
			Error::<T>::OpenHrmpChannelCapacityExceedsLimit,
		);
		ensure!(proposed_max_message_size > 0, Error::<T>::OpenHrmpChannelZeroMessageSize);
		ensure!(
			proposed_max_message_size <= config.hrmp_channel_max_message_size,
			Error::<T>::OpenHrmpChannelMessageSizeExceedsLimit,
		);

		let channel_id = HrmpChannelId { sender: origin, recipient };
		ensure!(
			HrmpOpenChannelRequests::<T>::get(&channel_id).is_none(),
			Error::<T>::OpenHrmpChannelAlreadyRequested,
		);
		ensure!(
			HrmpChannels::<T>::get(&channel_id).is_none(),
			Error::<T>::OpenHrmpChannelAlreadyExists,
		);

		let egress_cnt = HrmpEgressChannelsIndex::<T>::decode_len(&origin).unwrap_or(0) as u32;
		let open_req_cnt = HrmpOpenChannelRequestCount::<T>::get(&origin);
		let channel_num_limit = config.hrmp_max_parachain_outbound_channels;
		ensure!(
			egress_cnt + open_req_cnt < channel_num_limit,
			Error::<T>::OpenHrmpChannelLimitExceeded,
		);

		// Do not require deposits for channels with or amongst the system.
		let is_system = origin.is_system() || recipient.is_system();
		let deposit = if is_system { 0 } else { config.hrmp_sender_deposit };
		if !deposit.is_zero() {
			T::Currency::reserve(
				&origin.into_account_truncating(),
				deposit.unique_saturated_into(),
			)?;
		}

		// mutating storage directly now -- shall not bail henceforth.

		HrmpOpenChannelRequestCount::<T>::insert(&origin, open_req_cnt + 1);
		HrmpOpenChannelRequests::<T>::insert(
			&channel_id,
			HrmpOpenChannelRequest {
				confirmed: false,
				_age: 0,
				sender_deposit: deposit,
				max_capacity: proposed_max_capacity,
				max_message_size: proposed_max_message_size,
				max_total_size: config.hrmp_channel_max_total_size,
			},
		);
		HrmpOpenChannelRequestsList::<T>::append(channel_id);

		Self::send_to_para(
			"init_open_channel",
			&config,
			recipient,
			Self::wrap_notification(|| {
				use xcm::opaque::latest::{prelude::*, Xcm};
				Xcm(vec![HrmpNewChannelOpenRequest {
					sender: origin.into(),
					max_capacity: proposed_max_capacity,
					max_message_size: proposed_max_message_size,
				}])
			}),
		);

		Ok(())
	}

	/// Accept a pending open channel request from the given sender.
	///
	/// Basically the same as [`hrmp_accept_open_channel`](Pallet::hrmp_accept_open_channel) but
	/// intended for calling directly from other pallets rather than dispatched.
	pub fn accept_open_channel(origin: ParaId, sender: ParaId) -> DispatchResult {
		let channel_id = HrmpChannelId { sender, recipient: origin };
		let mut channel_req = HrmpOpenChannelRequests::<T>::get(&channel_id)
			.ok_or(Error::<T>::AcceptHrmpChannelDoesntExist)?;
		ensure!(!channel_req.confirmed, Error::<T>::AcceptHrmpChannelAlreadyConfirmed);

		// check if by accepting this open channel request, this parachain would exceed the
		// number of inbound channels.
		let config = configuration::ActiveConfig::<T>::get();
		let channel_num_limit = config.hrmp_max_parachain_inbound_channels;
		let ingress_cnt = HrmpIngressChannelsIndex::<T>::decode_len(&origin).unwrap_or(0) as u32;
		let accepted_cnt = HrmpAcceptedChannelRequestCount::<T>::get(&origin);
		ensure!(
			ingress_cnt + accepted_cnt < channel_num_limit,
			Error::<T>::AcceptHrmpChannelLimitExceeded,
		);

		// Do not require deposits for channels with or amongst the system.
		let is_system = origin.is_system() || sender.is_system();
		let deposit = if is_system { 0 } else { config.hrmp_recipient_deposit };
		if !deposit.is_zero() {
			T::Currency::reserve(
				&origin.into_account_truncating(),
				deposit.unique_saturated_into(),
			)?;
		}

		// persist the updated open channel request and then increment the number of accepted
		// channels.
		channel_req.confirmed = true;
		HrmpOpenChannelRequests::<T>::insert(&channel_id, channel_req);
		HrmpAcceptedChannelRequestCount::<T>::insert(&origin, accepted_cnt + 1);

		Self::send_to_para(
			"accept_open_channel",
			&config,
			sender,
			Self::wrap_notification(|| {
				use xcm::opaque::latest::{prelude::*, Xcm};
				Xcm(vec![HrmpChannelAccepted { recipient: origin.into() }])
			}),
		);

		Ok(())
	}

	fn cancel_open_request(origin: ParaId, channel_id: HrmpChannelId) -> DispatchResult {
		// check if the origin is allowed to close the channel.
		ensure!(channel_id.is_participant(origin), Error::<T>::CancelHrmpOpenChannelUnauthorized);

		let open_channel_req = HrmpOpenChannelRequests::<T>::get(&channel_id)
			.ok_or(Error::<T>::OpenHrmpChannelDoesntExist)?;
		ensure!(!open_channel_req.confirmed, Error::<T>::OpenHrmpChannelAlreadyConfirmed);

		// Remove the request by the channel id and sync the accompanying list with the set.
		HrmpOpenChannelRequests::<T>::remove(&channel_id);
		HrmpOpenChannelRequestsList::<T>::mutate(|open_req_channels| {
			if let Some(pos) = open_req_channels.iter().position(|x| x == &channel_id) {
				open_req_channels.swap_remove(pos);
			}
		});

		Self::decrease_open_channel_request_count(channel_id.sender);
		// Don't decrease `HrmpAcceptedChannelRequestCount` because we don't consider confirmed
		// requests here.

		// Unreserve the sender's deposit. The recipient could not have left their deposit because
		// we ensured that the request is not confirmed.
		T::Currency::unreserve(
			&channel_id.sender.into_account_truncating(),
			open_channel_req.sender_deposit.unique_saturated_into(),
		);

		Ok(())
	}

	fn close_channel(origin: ParaId, channel_id: HrmpChannelId) -> Result<(), Error<T>> {
		// check if the origin is allowed to close the channel.
		ensure!(channel_id.is_participant(origin), Error::<T>::CloseHrmpChannelUnauthorized);

		// check if the channel requested to close does exist.
		ensure!(
			HrmpChannels::<T>::get(&channel_id).is_some(),
			Error::<T>::CloseHrmpChannelDoesntExist,
		);

		// check that there is no outstanding close request for this channel
		ensure!(
			HrmpCloseChannelRequests::<T>::get(&channel_id).is_none(),
			Error::<T>::CloseHrmpChannelAlreadyUnderway,
		);

		HrmpCloseChannelRequests::<T>::insert(&channel_id, ());
		HrmpCloseChannelRequestsList::<T>::append(channel_id.clone());

		let config = configuration::ActiveConfig::<T>::get();
		let opposite_party =
			if origin == channel_id.sender { channel_id.recipient } else { channel_id.sender };

		Self::send_to_para(
			"close_channel",
			&config,
			opposite_party,
			Self::wrap_notification(|| {
				use xcm::opaque::latest::{prelude::*, Xcm};
				Xcm(vec![HrmpChannelClosing {
					initiator: origin.into(),
					sender: channel_id.sender.into(),
					recipient: channel_id.recipient.into(),
				}])
			}),
		);

		Ok(())
	}

	/// Returns the list of MQC heads for the inbound channels of the given recipient para paired
	/// with the sender para ids. This vector is sorted ascending by the para id and doesn't contain
	/// multiple entries with the same sender.
	#[cfg(test)]
	fn hrmp_mqc_heads(recipient: ParaId) -> Vec<(ParaId, Hash)> {
		let sender_set = HrmpIngressChannelsIndex::<T>::get(&recipient);

		// The ingress channels vector is sorted, thus `mqc_heads` is sorted as well.
		let mut mqc_heads = Vec::with_capacity(sender_set.len());
		for sender in sender_set {
			let channel_metadata = HrmpChannels::<T>::get(&HrmpChannelId { sender, recipient });
			let mqc_head = channel_metadata
				.and_then(|metadata| metadata.mqc_head)
				.unwrap_or(Hash::default());
			mqc_heads.push((sender, mqc_head));
		}

		mqc_heads
	}

	/// Returns contents of all channels addressed to the given recipient. Channels that have no
	/// messages in them are also included.
	pub(crate) fn inbound_hrmp_channels_contents(
		recipient: ParaId,
	) -> BTreeMap<ParaId, Vec<InboundHrmpMessage<BlockNumberFor<T>>>> {
		let sender_set = HrmpIngressChannelsIndex::<T>::get(&recipient);

		let mut inbound_hrmp_channels_contents = BTreeMap::new();
		for sender in sender_set {
			let channel_contents =
				HrmpChannelContents::<T>::get(&HrmpChannelId { sender, recipient });
			inbound_hrmp_channels_contents.insert(sender, channel_contents);
		}

		inbound_hrmp_channels_contents
	}
}

impl<T: Config> Pallet<T> {
	/// Decreases the open channel request count for the given sender. If the value reaches zero
	/// it is removed completely.
	fn decrease_open_channel_request_count(sender: ParaId) {
		HrmpOpenChannelRequestCount::<T>::mutate_exists(&sender, |opt_rc| {
			*opt_rc = opt_rc.and_then(|rc| match rc.saturating_sub(1) {
				0 => None,
				n => Some(n),
			});
		});
	}

	/// Decreases the accepted channel request count for the given sender. If the value reaches
	/// zero it is removed completely.
	fn decrease_accepted_channel_request_count(recipient: ParaId) {
		HrmpAcceptedChannelRequestCount::<T>::mutate_exists(&recipient, |opt_rc| {
			*opt_rc = opt_rc.and_then(|rc| match rc.saturating_sub(1) {
				0 => None,
				n => Some(n),
			});
		});
	}

	#[cfg(any(feature = "runtime-benchmarks", test))]
	fn assert_storage_consistency_exhaustive() {
		fn assert_is_sorted<T: Ord>(slice: &[T], id: &str) {
			assert!(slice.windows(2).all(|xs| xs[0] <= xs[1]), "{} supposed to be sorted", id);
		}

		let assert_contains_only_onboarded = |paras: Vec<ParaId>, cause: &str| {
			for para in paras {
				assert!(
					crate::paras::Pallet::<T>::is_valid_para(para),
					"{}: {:?} para is offboarded",
					cause,
					para
				);
			}
		};

		assert_eq!(
			HrmpOpenChannelRequests::<T>::iter().map(|(k, _)| k).collect::<BTreeSet<_>>(),
			HrmpOpenChannelRequestsList::<T>::get().into_iter().collect::<BTreeSet<_>>(),
		);

		// verify that the set of keys in `HrmpOpenChannelRequestCount` corresponds to the set
		// of _senders_ in `HrmpOpenChannelRequests`.
		//
		// having ensured that, we can go ahead and go over all counts and verify that they match.
		assert_eq!(
			HrmpOpenChannelRequestCount::<T>::iter()
				.map(|(k, _)| k)
				.collect::<BTreeSet<_>>(),
			HrmpOpenChannelRequests::<T>::iter()
				.map(|(k, _)| k.sender)
				.collect::<BTreeSet<_>>(),
		);
		for (open_channel_initiator, expected_num) in HrmpOpenChannelRequestCount::<T>::iter() {
			let actual_num = HrmpOpenChannelRequests::<T>::iter()
				.filter(|(ch, _)| ch.sender == open_channel_initiator)
				.count() as u32;
			assert_eq!(expected_num, actual_num);
		}

		// The same as above, but for accepted channel request count. Note that we are interested
		// only in confirmed open requests.
		assert_eq!(
			HrmpAcceptedChannelRequestCount::<T>::iter()
				.map(|(k, _)| k)
				.collect::<BTreeSet<_>>(),
			HrmpOpenChannelRequests::<T>::iter()
				.filter(|(_, v)| v.confirmed)
				.map(|(k, _)| k.recipient)
				.collect::<BTreeSet<_>>(),
		);
		for (channel_recipient, expected_num) in HrmpAcceptedChannelRequestCount::<T>::iter() {
			let actual_num = HrmpOpenChannelRequests::<T>::iter()
				.filter(|(ch, v)| ch.recipient == channel_recipient && v.confirmed)
				.count() as u32;
			assert_eq!(expected_num, actual_num);
		}

		assert_eq!(
			HrmpCloseChannelRequests::<T>::iter().map(|(k, _)| k).collect::<BTreeSet<_>>(),
			HrmpCloseChannelRequestsList::<T>::get().into_iter().collect::<BTreeSet<_>>(),
		);

		// A HRMP watermark can be None for an onboarded parachain. However, an offboarded parachain
		// cannot have an HRMP watermark: it should've been cleanup.
		assert_contains_only_onboarded(
			HrmpWatermarks::<T>::iter().map(|(k, _)| k).collect::<Vec<_>>(),
			"HRMP watermarks should contain only onboarded paras",
		);

		// An entry in `HrmpChannels` indicates that the channel is open. Only open channels can
		// have contents.
		for (non_empty_channel, contents) in HrmpChannelContents::<T>::iter() {
			assert!(HrmpChannels::<T>::contains_key(&non_empty_channel));

			// pedantic check: there should be no empty vectors in storage, those should be modeled
			// by a removed kv pair.
			assert!(!contents.is_empty());
		}

		// Senders and recipients must be onboarded. Otherwise, all channels associated with them
		// are removed.
		assert_contains_only_onboarded(
			HrmpChannels::<T>::iter()
				.flat_map(|(k, _)| vec![k.sender, k.recipient])
				.collect::<Vec<_>>(),
			"senders and recipients in all channels should be onboarded",
		);

		// Check the docs for `HrmpIngressChannelsIndex` and `HrmpEgressChannelsIndex` in decl
		// storage to get an index what are the channel mappings indexes.
		//
		// Here, from indexes.
		//
		// ingress         egress
		//
		// a -> [x, y]     x -> [a, b]
		// b -> [x, z]     y -> [a]
		//                 z -> [b]
		//
		// we derive a list of channels they represent.
		//
		//   (a, x)         (a, x)
		//   (a, y)         (a, y)
		//   (b, x)         (b, x)
		//   (b, z)         (b, z)
		//
		// and then that we compare that to the channel list in the `HrmpChannels`.
		let channel_set_derived_from_ingress = HrmpIngressChannelsIndex::<T>::iter()
			.flat_map(|(p, v)| v.into_iter().map(|i| (i, p)).collect::<Vec<_>>())
			.collect::<BTreeSet<_>>();
		let channel_set_derived_from_egress = HrmpEgressChannelsIndex::<T>::iter()
			.flat_map(|(p, v)| v.into_iter().map(|e| (p, e)).collect::<Vec<_>>())
			.collect::<BTreeSet<_>>();
		let channel_set_ground_truth = HrmpChannels::<T>::iter()
			.map(|(k, _)| (k.sender, k.recipient))
			.collect::<BTreeSet<_>>();
		assert_eq!(channel_set_derived_from_ingress, channel_set_derived_from_egress);
		assert_eq!(channel_set_derived_from_egress, channel_set_ground_truth);

		HrmpIngressChannelsIndex::<T>::iter()
			.map(|(_, v)| v)
			.for_each(|v| assert_is_sorted(&v, "HrmpIngressChannelsIndex"));
		HrmpEgressChannelsIndex::<T>::iter()
			.map(|(_, v)| v)
			.for_each(|v| assert_is_sorted(&v, "HrmpIngressChannelsIndex"));

		assert_contains_only_onboarded(
			HrmpChannelDigests::<T>::iter().map(|(k, _)| k).collect::<Vec<_>>(),
			"HRMP channel digests should contain only onboarded paras",
		);
		for (_digest_for_para, digest) in HrmpChannelDigests::<T>::iter() {
			// Assert that items are in **strictly** ascending order. The strictness also implies
			// there are no duplicates.
			assert!(digest.windows(2).all(|xs| xs[0].0 < xs[1].0));

			for (_, mut senders) in digest {
				assert!(!senders.is_empty());

				// check for duplicates. For that we sort the vector, then perform deduplication.
				// if the vector stayed the same, there are no duplicates.
				senders.sort();
				let orig_senders = senders.clone();
				senders.dedup();
				assert_eq!(
					orig_senders, senders,
					"duplicates removed implies existence of duplicates"
				);
			}
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Wraps HRMP XCM notifications to the most suitable XCM version for the destination para.
	/// If the XCM version is unknown, the latest XCM version is used as a best effort.
	fn wrap_notification(
		mut notification: impl FnMut() -> xcm::opaque::latest::opaque::Xcm,
	) -> impl FnOnce(ParaId) -> polkadot_primitives::DownwardMessage {
		use xcm::{
			opaque::VersionedXcm,
			prelude::{Junction, Location},
			WrapVersion,
		};

		// Return a closure that can prepare notifications.
		move |dest| {
			// Attempt to wrap the notification for the destination parachain.
			T::VersionWrapper::wrap_version(
				&Location::new(0, [Junction::Parachain(dest.into())]),
				notification(),
			)
			.unwrap_or_else(|_| {
				// As a best effort, if we cannot resolve the version, fallback to using the latest
				// version.
				VersionedXcm::from(notification())
			})
			.encode()
		}
	}

	/// Sends/enqueues notification to the destination parachain.
	fn send_to_para(
		log_label: &str,
		config: &HostConfiguration<BlockNumberFor<T>>,
		dest: ParaId,
		notification_bytes_for: impl FnOnce(ParaId) -> polkadot_primitives::DownwardMessage,
	) {
		// prepare notification
		let notification_bytes = notification_bytes_for(dest);

		// try to enqueue
		if let Err(dmp::QueueDownwardMessageError::ExceedsMaxMessageSize) =
			dmp::Pallet::<T>::queue_downward_message(&config, dest, notification_bytes)
		{
			// this should never happen unless the max downward message size is configured to a
			// jokingly small number.
			log::error!(
				target: "runtime::hrmp",
				"sending '{log_label}::notification_bytes' failed."
			);
			debug_assert!(false);
		}
	}
}
