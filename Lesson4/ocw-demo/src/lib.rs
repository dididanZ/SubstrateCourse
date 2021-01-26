//! A demonstration of an offchain worker that sends onchain callbacks

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

use core::{convert::TryInto, fmt};
use frame_support::{
	debug, decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult,
};
use parity_scale_codec::{Decode, Encode};

use frame_system::{
	self as system, ensure_none, ensure_signed,
	offchain::{
		AppCrypto, CreateSignedTransaction, SendSignedTransaction, SendUnsignedTransaction,
		SignedPayload, SigningTypes, Signer, SubmitTransaction,
	},
};
use sp_core::crypto::KeyTypeId;
use sp_runtime::{
	RuntimeDebug,
	offchain as rt_offchain,
	offchain::{
		storage::StorageValueRef,
		storage_lock::{StorageLock, BlockAndTime},
	},
	transaction_validity::{
		InvalidTransaction, TransactionSource, TransactionValidity,
		ValidTransaction,
	},
};
use sp_std::{
	prelude::*, str,
	collections::vec_deque::VecDeque,
};

use serde::{Deserialize, Deserializer};
use sp_std::str::FromStr;
use sp_runtime::offchain::http::Method;

/// Defines application identifier for crypto keys of this module.
///
/// Every module that deals with signatures needs to declare its unique identifier for
/// its crypto keys.
/// When an offchain worker is signing transactions it's going to request keys from type
/// `KeyTypeId` via the keystore to sign the transaction.
/// The keys can be inserted manually via RPC (see `author_insertKey`).
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"demo");
pub const NUM_VEC_LEN: usize = 10;
/// The type to sign and send transactions.
pub const UNSIGNED_TXS_PRIORITY: u64 = 100;

// We are fetching information from the github public API about organization`substrate-developer-hub`.
pub const HTTP_REMOTE_REQUEST: &str = "https://api.coincap.io/v2/assets/polkadot";

pub const FETCH_TIMEOUT_PERIOD: u64 = 10000; // in milli-seconds
pub const LOCK_TIMEOUT_EXPIRATION: u64 = FETCH_TIMEOUT_PERIOD + 1000; // in milli-seconds
pub const LOCK_BLOCK_EXPIRATION: u32 = 3; // in block number


#[derive(Deserialize, Encode, Decode, Default,Debug)]
struct PolkadotResponse{

	#[serde(default)]
	pub data : PolkadotPrice
}

#[derive(Deserialize, Encode, Decode, Default,Debug)]
struct PolkadotPrice{

	#[serde(deserialize_with = "u128_from_price")]
	pub priceUsd : u128
}
///保留6位小数
pub fn u128_from_price<'de, D>(de: D) -> Result<u128, D::Error>
	where
		D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	let sps : Vec<_>= s.split(".").collect();
	let mut p1 = u128::from_str(sps[0]).unwrap_or(0) * 100_000;
	if sps.len() == 2{
		if  sps[1].len() >= 6{
			let p2 = u128::from_str(&sps[1][0..6]).unwrap_or(0);
			p1 += p2;
		}else{
			let remain = 6 - sps[1].len();
			let p2 = u128::from_str(sps[1]).unwrap_or(0) * 10u128.pow(remain as u32);
			p1 += p2;
		}

	}
	debug::info!("-------------u128_from_price----{}---{:?} ",s, p1);
	Ok(p1)
}


/// This is the pallet's configuration trait
pub trait Trait: system::Trait + CreateSignedTransaction<Call<Self>> {

	/// The overarching dispatch call type.
	type Call: From<Call<Self>>;
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_storage! {
	trait Store for Module<T: Trait> as Example {
		/// 价格存储
		Princes get(fn princes): VecDeque<u128>;
	}
}

decl_event!(
	pub enum Event<T>
	where
		AccountId = <T as system::Trait>::AccountId
	{
		/// 新价格
		NewPrice(Option<AccountId>,u128),
	}
);

decl_error! {
	pub enum Error for Module<T: Trait> {
		// Error returned when not sure which ocw function to executed
		UnknownOffchainMux,

		// Error returned when making signed transactions in off-chain worker
		NoLocalAcctForSigning,
		OffchainSignedTxError,

		// Error returned when making unsigned transactions in off-chain worker
		OffchainUnsignedTxError,

		// Error returned when making unsigned transactions with signed payloads in off-chain worker
		OffchainUnsignedTxSignedPayloadError,

		// Error returned when fetching github info
		HttpFetchingError,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;



		#[weight = 10000]
		pub fn submit_number_unsigned(origin, prince: u128) -> DispatchResult {
			let _ = ensure_none(origin)?;
			debug::info!("submit_number_unsigned: {}", prince);
			Self::append_or_replace_prince(prince);

			Self::deposit_event(RawEvent::NewPrice(None,prince));
			Ok(())
		}
		//该数据不需要知道来源是谁，在生产环境中，不适用预加一个已知用户私钥
		//所以采用不具签名交易
		fn offchain_worker(block_number: T::BlockNumber) {
			debug::info!("Entering off-chain worker");
			const TX_TYPES: u32 = 4;
			let current :u32 = TryInto::<u32>::try_into(block_number).unwrap_or(0);
			let modu = current % TX_TYPES;
			let result = match modu {

				1 => Self::offchain_unsigned_tx(block_number),

				_ => Err(Error::<T>::UnknownOffchainMux),
			};

			if let Err(e) = result {
				debug::error!("offchain_worker error: {:?}", e);
			}
		}
	}
}

impl<T: Trait> Module<T> {
	/// Append a new number to the tail of the list, removing an element from the head if reaching
	///   the bounded length.
	fn append_or_replace_prince(prince: u128) {
		Princes::mutate(|princes| {
			if princes.len() == NUM_VEC_LEN {
				let _ = princes.pop_front();
			}
			princes.push_back(prince);
			debug::info!("Princes vector: {:?}", princes);
		});
	}

	/// Fetch from remote and deserialize the JSON to a struct
	fn fetch_n_parse() -> Result<PolkadotResponse, Error<T>> {
		let resp_bytes = Self::fetch_from_remote().map_err(|e| {
			debug::error!("fetch_from_remote error: {:?}", e);
			<Error<T>>::HttpFetchingError
		})?;

		let resp_str = str::from_utf8(&resp_bytes).map_err(|_| <Error<T>>::HttpFetchingError)?;
		// Print out our fetched JSON string
		debug::info!("{}", resp_str);

		// Deserializing JSON to struct, thanks to `serde` and `serde_derive`
		let info: PolkadotResponse =
			serde_json::from_str(&resp_str).map_err(|_| <Error<T>>::HttpFetchingError)?;
		Ok(info)
	}
	fn fetch_from_remote() -> Result<Vec<u8>, Error<T>>{
		let timeout = sp_io::offchain::timestamp()
			.add(rt_offchain::Duration::from_millis(FETCH_TIMEOUT_PERIOD));
		let pending =rt_offchain::http::Request::get(HTTP_REMOTE_REQUEST)
			.add_header("Content-Type","application/json")
			.send()
			.map_err(|_| <Error<T>>::HttpFetchingError)?;
		let response = pending.try_wait(timeout).map_err(|_| <Error<T>>::HttpFetchingError)?.map_err(|_| <Error<T>>::HttpFetchingError)?;
		if response.code != 200 {
			debug::error!("Unexpected http request status code: {}", response.code);
			return Err(<Error<T>>::HttpFetchingError);
		}
		Ok(response.body().collect::<Vec<u8>>())
	}


	fn offchain_unsigned_tx(block_number: T::BlockNumber) -> Result<(), Error<T>> {
		let result = Self::fetch_n_parse();
		match result {
			Ok(info) => {
				let call = Call::submit_number_unsigned(info.data.priceUsd);
				// `submit_unsigned_transaction` returns a type of `Result<(), ()>`
				//   ref: https://substrate.dev/rustdocs/v2.0.0/frame_system/offchain/struct.SubmitTransaction.html#method.submit_unsigned_transaction
				SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
					.map_err(|_| {
						debug::error!("Failed in offchain_unsigned_tx");
						<Error<T>>::OffchainUnsignedTxError
					})
			},
			Err(e) => {
				Err(e)
			}
		}
	}

}

impl<T: Trait> frame_support::unsigned::ValidateUnsigned for Module<T> {
	type Call = Call<T>;

	fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
		let valid_tx = |provide| ValidTransaction::with_tag_prefix("ocw-demo")
			.priority(UNSIGNED_TXS_PRIORITY)
			.and_provides([&provide])
			.longevity(3)
			.propagate(true)
			.build();

		match call {
			Call::submit_number_unsigned(_number) => valid_tx(b"submit_number_unsigned".to_vec()),

			_ => InvalidTransaction::Call.into(),
		}
	}
}

impl<T: Trait> rt_offchain::storage_lock::BlockNumberProvider for Module<T> {
	type BlockNumber = T::BlockNumber;
	fn current_block_number() -> Self::BlockNumber {
	  <frame_system::Module<T>>::block_number()
	}
}
