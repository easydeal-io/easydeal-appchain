#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use sp_std::prelude::*;
use sp_core::H256;
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
	traits::{
		Randomness, ReservableCurrency, Currency, 
		ExistenceRequirement::{AllowDeath, KeepAlive}
	},
	sp_runtime::traits::AccountIdConversion, 
};

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>
pub use pallet::*;

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
	user_name: Vec<u8>,
	email: Vec<u8>,
	verify_hash: H256,
	register_block: BlockNumber,
	encrypted_data: Vec<u8>,
}


#[frame_support::pallet]
pub mod pallet {
	use super::*;

	use frame_support::pallet_prelude::*;
	use frame_system::{self as system, pallet_prelude::*};

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
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

	#[pallet::storage]
	pub type AccountIdByUserName<T: Config> =
		StorageMap<_, Twox64Concat, Vec<u8>, T::AccountId>;

	#[pallet::storage]
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
		/// User name exist
		UserNameExist,
		/// Email api not set
		EmailApiNotSet,
		/// Not found user
		NotFoundUser,
		/// User status incorrect
		UserStatusIncorrect,
		/// Verify failed
		VerifyFailed,
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
			user_name: Vec<u8>,
			email: Vec<u8>,
			code: Vec<u8>,
			encrypted_data: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			
			let code_hash = Keccak256::hash(&code);
			log::info!("in pre_register: {:?}, hash: {:?}", &who, &code_hash);

			let invite_code = <InviteCodes<T>>::get(&code_hash).ok_or(Error::<T>::InviteCodeNotExist)?;
	
			// check whether invite code valid
			let block_number = system::Pallet::<T>::block_number();
			if invite_code.expire_block < block_number {
				<InviteCodes<T>>::remove(&code_hash);
				Err(Error::<T>::InviteCodeExpired)?
			}
			ensure!(invite_code.expire_block > block_number, Error::<T>::InviteCodeExpired);
	
			// check if user name have been used
			ensure!(<AccountIdByUserName<T>>::get(&user_name).is_none(), Error::<T>::UserNameExist);

			// transfer
			T::Currency::transfer(
				&Self::account_id(),
				&who,
				invite_code.attached_balance,
				AllowDeath
			)?;

			let random_code = Self::generate_random_code();

			// let email_api = <EmailApi<T>>::get().ok_or(Error::<T>::EmailApiNotSet)?;
			let random_code_hash = Keccak256::hash(&(&who, random_code).encode());
			log::info!("code: {:?}, hash: {:?}", random_code, random_code_hash);

			<Users<T>>::insert(&who, User {
				status: 0,
				user_name: user_name.clone(),
				email,
				verify_hash: random_code_hash,
				register_block: block_number,
				encrypted_data
			});

			<AccountIdByUserName<T>>::insert(&user_name, who);

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
	}

	impl<T: Config> Pallet<T> {
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account()
		}

		fn generate_random_code() -> u32 {
			let phrase = b"generate_code";
			let (seed, _) = T::Randomness::random(phrase);
			let seed = <[u8; 32]>::decode(&mut TrailingZeroInput::new(seed.as_ref()))
				.expect("input is padded with zeroes; qed");

			let mut rng = ChaChaRng::from_seed(seed);

			rng.next_u32()
		}
	}

}
