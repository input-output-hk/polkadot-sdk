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

use polkadot_node_core_pvf_common::error::{InternalValidationError, PrepareError};

/// A error raised during validation of the candidate.
#[derive(thiserror::Error, Debug, Clone)]
pub enum ValidationError {
	/// Deterministic preparation issue. In practice, most of the problems should be caught by
	/// prechecking, so this may be a sign of internal conditions.
	///
	/// In principle if preparation of the `WASM` fails, the current candidate cannot be the
	/// reason for that. So we can't say whether it is invalid or not. In addition, with
	/// pre-checking enabled only valid runtimes should ever get enacted, so we can be
	/// reasonably sure that this is some local problem on the current node. However, as this
	/// particular error *seems* to indicate a deterministic error, we raise a warning.
	#[error("candidate validation: {0}")]
	Preparation(PrepareError),
	/// The error was raised because the candidate is invalid. Should vote against.
	#[error("candidate validation: {0}")]
	Invalid(#[from] InvalidCandidate),
	/// Possibly transient issue that may resolve after retries. Should vote against when retries
	/// fail.
	#[error("candidate validation: {0}")]
	PossiblyInvalid(#[from] PossiblyInvalidError),
	/// Preparation or execution issue caused by an internal condition. Should not vote against.
	#[error("candidate validation: internal: {0}")]
	Internal(#[from] InternalValidationError),
	/// The execution deadline of allowed_ancestry_len + 1 has been reached. Jobs like backing have
	/// a limited time to execute. Once the deadline is reached, the current candidate cannot be
	/// backed, regardless of its validity.
	#[error("candidate validation: execution deadline has been reached.")]
	ExecutionDeadline,
}

/// A description of an error raised during executing a PVF and can be attributed to the combination
/// of the candidate [`polkadot_parachain_primitives::primitives::ValidationParams`] and the PVF.
#[derive(thiserror::Error, Debug, Clone)]
pub enum InvalidCandidate {
	/// The candidate is reported to be invalid by the execution worker. The string contains the
	/// error message.
	#[error("invalid: worker reported: {0}")]
	WorkerReportedInvalid(String),
	/// PVF execution (compilation is not included) took more time than was allotted.
	#[error("invalid: hard timeout")]
	HardTimeout,
	/// Proof-of-validity failed to decompress correctly
	#[error("invalid: PoV failed to decompress")]
	PoVDecompressionFailure,
}

/// Possibly transient issue that may resolve after retries.
#[derive(thiserror::Error, Debug, Clone)]
pub enum PossiblyInvalidError {
	/// The worker process (not the job) has died during validation of a candidate.
	///
	/// It's unlikely that this is caused by malicious code since workers spawn separate job
	/// processes, and those job processes are sandboxed. But, it is possible. We retry in this
	/// case, and if the error persists, we assume it's caused by the candidate and vote against.
	#[error("possibly invalid: ambiguous worker death")]
	AmbiguousWorkerDeath,
	/// The job process (not the worker) has died for one of the following reasons:
	///
	/// (a) A seccomp violation occurred, most likely due to an attempt by malicious code to
	/// execute arbitrary code. Note that there is no foolproof way to detect this if the operator
	/// has seccomp auditing disabled.
	///
	/// (b) The host machine ran out of free memory and the OOM killer started killing the
	/// processes, and in order to save the parent it will "sacrifice child" first.
	///
	/// (c) Some other reason, perhaps transient or perhaps caused by malicious code.
	///
	/// We cannot treat this as an internal error because malicious code may have caused this.
	#[error("possibly invalid: ambiguous job death: {0}")]
	AmbiguousJobDeath(String),
	/// An unexpected error occurred in the job process and we can't be sure whether the candidate
	/// is really invalid or some internal glitch occurred. Whenever we are unsure, we can never
	/// treat an error as internal as we would abstain from voting. This is bad because if the
	/// issue was due to the candidate, then all validators would abstain, stalling finality on the
	/// chain. So we will first retry the candidate, and if the issue persists we are forced to
	/// vote invalid.
	#[error("possibly invalid: job error: {0}")]
	JobError(String),
	/// Instantiation of the WASM module instance failed during an execution.
	/// Possibly related to local issues or dirty node update. May be retried with re-preparation.
	#[error("possibly invalid: runtime construction: {0}")]
	RuntimeConstruction(String),
	/// The artifact is corrupted, re-prepare the artifact and try again.
	#[error("possibly invalid: artifact is corrupted")]
	CorruptedArtifact,
}

impl From<PrepareError> for ValidationError {
	fn from(error: PrepareError) -> Self {
		// Here we need to classify the errors into two errors: deterministic and non-deterministic.
		// See [`PrepareError::is_deterministic`].
		if error.is_deterministic() {
			Self::Preparation(error)
		} else {
			Self::Internal(InternalValidationError::NonDeterministicPrepareError(error))
		}
	}
}
