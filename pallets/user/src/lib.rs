#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
#[cfg(not(feature = "std"))]
extern crate alloc;

use codec::{Decode, Encode};
use sp_std::{str, prelude::*};
use sp_core::{crypto::KeyTypeId, H256};
use sp_runtime::{
	RuntimeDebug, 
	traits::{Hash, Keccak256, TrailingZeroInput},
};

use rand_chacha::{
	rand_core::{RngCore, SeedableRng},
	ChaChaRng,
};

use frame_support::{
	dispatch::DispatchResult, PalletId,
	storage::IterableStorageMap,
	traits::{
		Randomness, ReservableCurrency, Currency, 
		ExistenceRequirement::{AllowDeath, KeepAlive}
	},
	sp_runtime::{
		traits::AccountIdConversion, 
		offchain::{
			http, Duration, 
			storage::{MutateStorageError, StorageValueRef, StorageRetrievalError}
		}
	}, 
};

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>
pub use pallet::*;

pub type AuthorityId = crypto::Public;

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct InviteCode<AccountId, BlockNumber, Balance> {
	inviter: AccountId,
	expire_block: BlockNumber,
	attached_balance: Balance,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct User<BlockNumber> {
	status: u8,
	email: Vec<u8>,
	verify_hash: H256,
	register_block: BlockNumber,
	encrypted_data: Vec<u8>,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct EmailRequest<AccountId, BlockNumber> {
	requester: AccountId,
	email: Vec<u8>,
	requested_block: BlockNumber,
}

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"user");

/// Based on the above `KeyTypeId` we need to generate a pallet-specific crypto type wrappers.
/// We can use from supported crypto kinds (`sr25519`, `ed25519` and `ecdsa`) and augment
/// the types with this pallet-specific identifier.
pub mod crypto {
	use super::KEY_TYPE;
	use sp_runtime::app_crypto::{app_crypto, sr25519};
	app_crypto!(sr25519, KEY_TYPE);
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	use frame_support::pallet_prelude::*;
	use frame_system::{
		self as system, pallet_prelude::*, 
		offchain::{AppCrypto, CreateSignedTransaction, Signer, SubmitTransaction}
	};

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: CreateSignedTransaction<Call<Self>> + frame_system::Config {
		/// The identifier type for an offchain worker.
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;

		/// This pallet id
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// The currency trait.
		type Currency: ReservableCurrency<Self::AccountId>;

		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Invite code valid duration in blocks.
		type InviteCodeValidPeriod: Get<Self::BlockNumber>;

		/// Something that provides randomness in the runtime.
		type Randomness: Randomness<Self::Hash, Self::BlockNumber>;

		#[pallet::constant]
		type UnsignedPriority: Get<TransactionPriority>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub type InviteCodes<T: Config> = 
		StorageMap<_, Twox64Concat, H256, InviteCode<T::AccountId, T::BlockNumber, BalanceOf<T>>>;

	#[pallet::storage]
	pub type Users<T: Config> = 
		StorageMap<_, Twox64Concat, T::AccountId, User<T::BlockNumber>>;

	#[pallet::type_value]
	pub fn InitialEmailRequestId<T: Config>() -> u64 { 10000000u64 }
	
	#[pallet::storage]
	pub type EmailRequestId<T: Config> =StorageValue<_, u64, ValueQuery, InitialEmailRequestId<T>>;
	
	#[pallet::storage]
	pub type EmailRequests<T: Config> = StorageMap<_, Twox64Concat, u64, EmailRequest<T::AccountId, T::BlockNumber>>;

	#[pallet::storage]
	pub type AccountIdByEmailHash<T: Config> =
		StorageMap<_, Twox64Concat, H256, T::AccountId>;

	#[pallet::storage]
	#[pallet::getter(fn email_api)]
	pub type EmailApi<T> = StorageValue<_, Vec<u8>>;

	// Pallets use events to inform users when important changes are made.
	// https://substrate.dev/docs/en/knowledgebase/runtime/events
	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Invite code exist
		InviteCodeExist,
		/// Invite code not exist
		InviteCodeNotExist,
		/// Invite code expired
		InviteCodeExpired,
		/// Email used
		EmailUsed,
		/// Email api not set
		EmailApiNotSet,
		/// Not found user
		NotFoundUser,
		/// User status incorrect
		UserStatusIncorrect,
		/// Verify failed
		VerifyFailed,
		/// Email request not exist
		EmailRequestNotExist,
		/// User not exist
		UserNotExist,
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		/// Validate unsigned call to this module.
		///
		/// By default unsigned transactions are disallowed, but implementing the validator
		/// here we make sure that some particular calls (the ones produced by offchain worker)
		/// are being whitelisted and marked as valid.
		fn validate_unsigned(
			_source: TransactionSource,
			call: &Self::Call,
		) -> TransactionValidity {

			if let Call::clear_email_requests_unsigned(block_number,_processed_requests) = call {
				Self::validate_transaction(block_number)
			} else {
				InvalidTransaction::Call.into()
			}
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {

		fn offchain_worker(block_number: T::BlockNumber) {
			let parent_hash = <system::Pallet<T>>::block_hash(block_number - 1u32.into());
			log::debug!("Current block: {:?} (parent hash: {:?})", block_number, parent_hash);

			if !Self::should_send(block_number) {
				return;
			}

			let email_api = Self::email_api();

			if email_api.is_some() {
				if let Err(e) = Self::send_email(email_api.unwrap()) {
					log::info!("send email: Error: {}", e);
				}
			} else {
				log::info!("email api not set");
			}
			
		}

	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
	
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1, 1))]
		pub fn add_invite_code(
			origin: OriginFor<T>, 
			code: Vec<u8>, 
			attached_balance: BalanceOf<T>,
		) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://substrate.dev/docs/en/knowledgebase/runtime/origin
			let who = ensure_signed(origin)?;

			let code_hash = Keccak256::hash(&code);
			ensure!(<InviteCodes<T>>::get(&code_hash).is_none(), Error::<T>::InviteCodeExist);

			let expire_block = system::Pallet::<T>::block_number() + T::InviteCodeValidPeriod::get();

			// transfer
			T::Currency::transfer(
				&who,
				&Self::account_id(),
				attached_balance,
				KeepAlive
			)?;

			<InviteCodes<T>>::insert(&code_hash, InviteCode {
				inviter: who,
				expire_block,
				attached_balance
			});

			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}

		#[pallet::weight((
			0,
			DispatchClass::Normal,
			Pays::No
		))]
		pub fn register(
			origin: OriginFor<T>, 
			email: Vec<u8>,
			code: Vec<u8>,
			encrypted_data: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			
			let code_hash = Keccak256::hash(&code);
			let email_hash = Keccak256::hash(&email);

			log::info!("in register: {:?}, hash: {:?}", &who, &code_hash);

			let invite_code = <InviteCodes<T>>::get(&code_hash).ok_or(Error::<T>::InviteCodeNotExist)?;
	
			// check whether invite code valid
			let block_number = system::Pallet::<T>::block_number();
			if invite_code.expire_block < block_number {
				<InviteCodes<T>>::remove(&code_hash);
				Err(Error::<T>::InviteCodeExpired)?
			}
			ensure!(invite_code.expire_block > block_number, Error::<T>::InviteCodeExpired);
			
			// check if email have been used
			ensure!(<AccountIdByEmailHash<T>>::get(&email_hash).is_none(), Error::<T>::EmailUsed);

			let email_request_id = <EmailRequestId<T>>::get();

			<EmailRequestId<T>>::put(email_request_id + 1u64);
			<EmailRequests<T>>::insert(email_request_id, EmailRequest {
				requester: who.clone(),
				email: email.clone(),
				requested_block: block_number
			});

			// transfer balance
			T::Currency::transfer(
				&Self::account_id(),
				&who,
				invite_code.attached_balance,
				AllowDeath
			)?;

			<Users<T>>::insert(&who, User {
				status: 0,
				email,
				verify_hash: Keccak256::hash(&[0u8]),
				register_block: block_number,
				encrypted_data
			});

			<AccountIdByEmailHash<T>>::insert(&email_hash, who);

			// remove code
			<InviteCodes<T>>::remove(&code_hash);

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1, 1))]
		pub fn activate(
			origin: OriginFor<T>,
			verify_code: u32
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let mut user = <Users<T>>::get(&who).ok_or(Error::<T>::NotFoundUser)?;
			ensure!(user.status == 0, Error::<T>::UserStatusIncorrect);

			let code_hash = Keccak256::hash(&(&who, verify_code).encode());
			ensure!(user.verify_hash == code_hash, Error::<T>::VerifyFailed);
			// update status
			user.status = 1;

			<Users<T>>::insert(&who, user);

			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1, 1))]
		pub fn update_email_api(
			origin: OriginFor<T>,
			api_url: Vec<u8>
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			log::info!("in update email api, {:?}, {:?}", who, api_url.clone());

			<EmailApi<T>>::put(api_url);
			
			Ok(())
		}

		#[pallet::weight(0 + T::DbWeight::get().writes(1))]
		pub fn clear_email_requests_unsigned(
			origin: OriginFor<T>,
			_block_number: T::BlockNumber,
			processed_requests: Vec<(u64, H256)>
		) -> DispatchResult {
			// This ensures that the function can only be called via unsigned transaction.
			ensure_none(origin)?;
			for (key, hash) in processed_requests.iter() {
				let email_request = <EmailRequests<T>>::get(&key).ok_or(Error::<T>::EmailRequestNotExist)?;

				log::info!("clearing request: {:?}, {:?}...", key, hash);
				// update verify hash
				<Users<T>>::try_mutate(&email_request.requester, |maybe_user| -> DispatchResult {
					let mut user = maybe_user.take().ok_or(Error::<T>::UserNotExist)?;
					user.verify_hash = *hash;
					*maybe_user = Some(user);
					Ok(())
				})?;
			
				<EmailRequests<T>>::remove(&key);
			}
			Ok(().into())
		}

	}

	impl<T: Config> Pallet<T> {
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account()
		}

		fn should_send(block_number: T::BlockNumber) -> bool {
			/// A friendlier name for the error that is going to be returned in case we are in the grace
			/// period.
			const RECENTLY_SENT: () = ();

			let val = StorageValueRef::persistent(b"user::last_send");
		
			let res =
				val.mutate(|last_send: Result<Option<T::BlockNumber>, StorageRetrievalError>| {
					match last_send {
						// If we already have a value in storage and the block number is recent enough
						// we avoid sending another transaction at this time.
						Ok(Some(block)) if block_number < block => {
							Err(RECENTLY_SENT)
						}
						// In every other case we attempt to acquire the lock and send a transaction.
						_ => Ok(block_number),
					}
				});

			match res {
				Ok(_block_number) => true,
				Err(MutateStorageError::ValueFunctionFailed(RECENTLY_SENT)) => false,
				Err(MutateStorageError::ConcurrentModification(_)) => false,
			}
		}

		fn send_email(email_api: Vec<u8>) -> Result<(), &'static str> {

			let signer = Signer::<T, T::AuthorityId>::all_accounts();
			if !signer.can_sign() {
				return Err(
					"No local accounts available. Consider adding one via `author_insertKey` RPC.",
				)?;
			}

			let block_number = <system::Pallet<T>>::block_number();
			let mut processed_requests: Vec<(u64, H256)>  = Vec::new();
			let mut fetch_urls: Vec<Vec<u8>> = Vec::new();

			for (key, val) in <EmailRequests<T> as IterableStorageMap<_, _>>::iter() {
				
				let random_code = Self::generate_random_code();
				let email_api = str::from_utf8(email_api.as_ref()).unwrap();
				
				let str_email = str::from_utf8(&val.email.as_ref()).unwrap();
				let fetch_url = format!("{}?email={}&code={}", email_api, str_email, random_code);
			
				let random_code_hash = Keccak256::hash(&(&val.requester, random_code).encode());
				log::info!("code: {:?}, hash: {:?}, fetch_url: {:?}", random_code, random_code_hash, fetch_url);
				fetch_urls.push(fetch_url.into_bytes());

				processed_requests.push((key, random_code_hash));
			}

			let requests_count = processed_requests.iter().count();
			if requests_count > 0 {
				log::info!("try to clear queue, size: {:?}", requests_count);
				let result = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(
					Call::clear_email_requests_unsigned(block_number, processed_requests).into()
				);

				if let Err(e) = result {
					log::error!("Error submitting unsigned transaction: {:?}", e);
				}
			}

			for url in fetch_urls.iter() {
				let str_url = str::from_utf8(url.as_ref()).unwrap();
				Self::fetch_http(&str_url).map_err(|_| "Failed to send email")?;
				log::info!("email sent");
			}

			Ok(())
		}

		fn generate_random_code() -> u32 {
			let phrase = b"generate_code";
			let (seed, _) = T::Randomness::random(phrase);
			let seed = <[u8; 32]>::decode(&mut TrailingZeroInput::new(seed.as_ref()))
				.expect("input is padded with zeroes; qed");

			let mut rng = ChaChaRng::from_seed(seed);

			rng.next_u32()
		}

		fn fetch_http(url: &str) -> Result<Vec<u8>, http::Error> {
			let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(15_000));
			let request = http::Request::get(url);

			let pending = request
				.deadline(deadline)
				.send()
				.map_err(|_| http::Error::IoError)?;

			let response = pending
				.try_wait(deadline)
				.map_err(|_| http::Error::DeadlineReached)??;

			if response.code != 200 {
				log::info!("Unexpected status code: {}", response.code);
				return Err(http::Error::Unknown);
			}

			let body = response.body().collect::<Vec<u8>>();

			let body_str = str::from_utf8(&body).map_err(|_| {
				log::info!("No UTF8 body");
				http::Error::Unknown
			})?;

			log::info!("fetch_http_get_result Got {} result: {}", url, body_str);

			Ok(body_str.as_bytes().to_vec())
		}

		fn validate_transaction(block_number: &T::BlockNumber) -> TransactionValidity {

			// Let's make sure to reject transactions from the future.
			let current_block = <system::Pallet<T>>::block_number();
			if &current_block < block_number {
				return InvalidTransaction::Future.into();
			}
			ValidTransaction::with_tag_prefix("Eayseal OCW")
				.priority(T::UnsignedPriority::get())
				.longevity(5)
				.propagate(true)
				.build()
		
		}
	
	}

}
