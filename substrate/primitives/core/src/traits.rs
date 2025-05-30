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

//! Shareable Substrate traits.

use alloc::{borrow::Cow, boxed::Box, string::String, vec::Vec};
use core::fmt::{Debug, Display};

pub use sp_externalities::{Externalities, ExternalitiesExt};

/// The context in which a call is done.
///
/// Depending on the context the executor may chooses different kind of heap sizes for the runtime
/// instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum CallContext {
	/// The call is happening in some offchain context.
	Offchain,
	/// The call is happening in some on-chain context like building or importing a block.
	Onchain,
}

/// Code execution engine.
pub trait CodeExecutor: Sized + Send + Sync + ReadRuntimeVersion + Clone + 'static {
	/// Externalities error type.
	type Error: Display + Debug + Send + Sync + 'static;

	/// Call a given method in the runtime.
	///
	/// Returns a tuple of the result (either the output data or an execution error) together with a
	/// `bool`, which is true if native execution was used.
	fn call(
		&self,
		ext: &mut dyn Externalities,
		runtime_code: &RuntimeCode,
		method: &str,
		data: &[u8],
		context: CallContext,
	) -> (Result<Vec<u8>, Self::Error>, bool);
}

/// Something that can fetch the runtime `:code`.
pub trait FetchRuntimeCode {
	/// Fetch the runtime `:code`.
	///
	/// If the `:code` could not be found/not available, `None` should be returned.
	fn fetch_runtime_code(&self) -> Option<Cow<[u8]>>;
}

/// Wrapper to use a `u8` slice or `Vec` as [`FetchRuntimeCode`].
pub struct WrappedRuntimeCode<'a>(pub Cow<'a, [u8]>);

impl<'a> FetchRuntimeCode for WrappedRuntimeCode<'a> {
	fn fetch_runtime_code(&self) -> Option<Cow<[u8]>> {
		Some(self.0.as_ref().into())
	}
}

/// Type that implements [`FetchRuntimeCode`] and always returns `None`.
pub struct NoneFetchRuntimeCode;

impl FetchRuntimeCode for NoneFetchRuntimeCode {
	fn fetch_runtime_code(&self) -> Option<Cow<[u8]>> {
		None
	}
}

/// The Wasm code of a Substrate runtime.
#[derive(Clone)]
pub struct RuntimeCode<'a> {
	/// The code fetcher that can be used to lazily fetch the code.
	pub code_fetcher: &'a dyn FetchRuntimeCode,
	/// The optional heap pages this `code` should be executed with.
	///
	/// If `None` are given, the default value of the executor will be used.
	pub heap_pages: Option<u64>,
	/// The hash of `code`.
	///
	/// The hashing algorithm isn't that important, as long as all runtime
	/// code instances use the same.
	pub hash: Vec<u8>,
}

impl<'a> PartialEq for RuntimeCode<'a> {
	fn eq(&self, other: &Self) -> bool {
		self.hash == other.hash
	}
}

impl<'a> RuntimeCode<'a> {
	/// Create an empty instance.
	///
	/// This is only useful for tests that don't want to execute any code.
	pub fn empty() -> Self {
		Self { code_fetcher: &NoneFetchRuntimeCode, hash: Vec::new(), heap_pages: None }
	}
}

impl<'a> FetchRuntimeCode for RuntimeCode<'a> {
	fn fetch_runtime_code(&self) -> Option<Cow<[u8]>> {
		self.code_fetcher.fetch_runtime_code()
	}
}

/// Could not find the `:code` in the externalities while initializing the [`RuntimeCode`].
#[derive(Debug)]
pub struct CodeNotFound;

impl core::fmt::Display for CodeNotFound {
	fn fmt(&self, f: &mut core::fmt::Formatter) -> Result<(), core::fmt::Error> {
		write!(f, "the storage entry `:code` doesn't have any code")
	}
}

/// A trait that allows reading version information from the binary.
pub trait ReadRuntimeVersion: Send + Sync {
	/// Reads the runtime version information from the given wasm code.
	///
	/// The version information may be embedded into the wasm binary itself. If it is not present,
	/// then this function may fallback to the legacy way of reading the version.
	///
	/// The legacy mechanism involves instantiating the passed wasm runtime and calling
	/// `Core_version` on it. This is a very expensive operation.
	///
	/// `ext` is only needed in case the calling into runtime happens. Otherwise it is ignored.
	///
	/// Compressed wasm blobs are supported and will be decompressed if needed. If uncompression
	/// fails, the error is returned.
	///
	/// # Errors
	///
	/// If the version information present in binary, but is corrupted - returns an error.
	///
	/// Otherwise, if there is no version information present, and calling into the runtime takes
	/// place, then an error would be returned if `Core_version` is not provided.
	fn read_runtime_version(
		&self,
		wasm_code: &[u8],
		ext: &mut dyn Externalities,
	) -> Result<Vec<u8>, String>;
}

impl ReadRuntimeVersion for alloc::sync::Arc<dyn ReadRuntimeVersion> {
	fn read_runtime_version(
		&self,
		wasm_code: &[u8],
		ext: &mut dyn Externalities,
	) -> Result<Vec<u8>, String> {
		(**self).read_runtime_version(wasm_code, ext)
	}
}

sp_externalities::decl_extension! {
	/// An extension that provides functionality to read version information from a given wasm blob.
	pub struct ReadRuntimeVersionExt(Box<dyn ReadRuntimeVersion>);
}

impl ReadRuntimeVersionExt {
	/// Creates a new instance of the extension given a version determinator instance.
	pub fn new<T: ReadRuntimeVersion + 'static>(inner: T) -> Self {
		Self(Box::new(inner))
	}
}

/// Something that can spawn tasks (blocking and non-blocking) with an assigned name
/// and optional group.
pub trait SpawnNamed: dyn_clone::DynClone + Send + Sync {
	/// Spawn the given blocking future.
	///
	/// The given `group` and `name` is used to identify the future in tracing.
	fn spawn_blocking(
		&self,
		name: &'static str,
		group: Option<&'static str>,
		future: futures::future::BoxFuture<'static, ()>,
	);
	/// Spawn the given non-blocking future.
	///
	/// The given `group` and `name` is used to identify the future in tracing.
	fn spawn(
		&self,
		name: &'static str,
		group: Option<&'static str>,
		future: futures::future::BoxFuture<'static, ()>,
	);
}

dyn_clone::clone_trait_object!(SpawnNamed);

impl SpawnNamed for Box<dyn SpawnNamed> {
	fn spawn_blocking(
		&self,
		name: &'static str,
		group: Option<&'static str>,
		future: futures::future::BoxFuture<'static, ()>,
	) {
		(**self).spawn_blocking(name, group, future)
	}
	fn spawn(
		&self,
		name: &'static str,
		group: Option<&'static str>,
		future: futures::future::BoxFuture<'static, ()>,
	) {
		(**self).spawn(name, group, future)
	}
}

/// Something that can spawn essential tasks (blocking and non-blocking) with an assigned name
/// and optional group.
///
/// Essential tasks are special tasks that should take down the node when they end.
pub trait SpawnEssentialNamed: dyn_clone::DynClone + Send + Sync {
	/// Spawn the given blocking future.
	///
	/// The given `group` and `name` is used to identify the future in tracing.
	fn spawn_essential_blocking(
		&self,
		name: &'static str,
		group: Option<&'static str>,
		future: futures::future::BoxFuture<'static, ()>,
	);
	/// Spawn the given non-blocking future.
	///
	/// The given `group` and `name` is used to identify the future in tracing.
	fn spawn_essential(
		&self,
		name: &'static str,
		group: Option<&'static str>,
		future: futures::future::BoxFuture<'static, ()>,
	);
}

dyn_clone::clone_trait_object!(SpawnEssentialNamed);

impl SpawnEssentialNamed for Box<dyn SpawnEssentialNamed> {
	fn spawn_essential_blocking(
		&self,
		name: &'static str,
		group: Option<&'static str>,
		future: futures::future::BoxFuture<'static, ()>,
	) {
		(**self).spawn_essential_blocking(name, group, future)
	}

	fn spawn_essential(
		&self,
		name: &'static str,
		group: Option<&'static str>,
		future: futures::future::BoxFuture<'static, ()>,
	) {
		(**self).spawn_essential(name, group, future)
	}
}
