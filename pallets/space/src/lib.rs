#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>

use frame_support::{
	RuntimeDebug,
	
};

use sp_std::prelude::*;

use codec::{Encode, Decode};
pub use pallet::*;

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub struct Space<AccountId, BlockNumber> {
	id: u32,
	name: Vec<u8>,
	description: Vec<u8>,
	fee_rate: u32,
	follows: u32,
	creator: AccountId,
	block: BlockNumber,
	status: u8
}

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{dispatch::DispatchResultWithPostInfo, pallet_prelude::*};
	use frame_system::pallet_prelude::*;

	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {

		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(crate) type SpaceIndex<T> = StorageValue<_, u32, ValueQuery>;

	#[pallet::storage]
	pub(crate) type Spaces<T: Config> = StorageMap<_, Twox64Concat, u32, Space<T::AccountId, T::BlockNumber>>;

	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T:Config> Pallet<T> {
		
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn add(
			origin: OriginFor<T>, 
			name: Vec<u8>, 
			description: Vec<u8>, 
			fee_rate: u32
		) -> DispatchResultWithPostInfo {
		
			let who = ensure_signed(origin)?;
			
			let block_number = frame_system::Pallet::<T>::block_number();
			let index = SpaceIndex::<T>::get();
			let next_index = index + 1;
			let space: Space<T::AccountId, T::BlockNumber> = Space {
				id: next_index,
				name: name, 
				description: description,
				fee_rate: fee_rate,
				follows: 0,
				creator: who.clone(),
				block: block_number,
				status: 1
			};

			SpaceIndex::<T>::put(next_index);
			Spaces::<T>::insert(next_index, space);

			Ok(().into())
		}

	}
}