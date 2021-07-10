#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>

use frame_support::{
	RuntimeDebug,
	traits::{ 
		Currency, ReservableCurrency
	},
};

use sp_std::prelude::*;

use codec::{Encode, Decode};
pub use pallet::*;

type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub struct Info<BlockNumber, Balance, AccountId> {
	id: u32,
	space: u32,
	info_type: u8, // 0 Sell, 1 Buy
	title: Vec<u8>,
	content: Vec<u8>,
	price: Balance,
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
	pub trait Config: frame_system::Config + pallet_space::Config {

		type Currency: ReservableCurrency<Self::AccountId>;

		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(crate) type InfoIndex<T> = StorageValue<_, u32, ValueQuery>;

	#[pallet::storage]
	pub(crate) type Infos<T: Config> = StorageMap<_, Twox64Concat, u32, Info<T::BlockNumber, BalanceOf<T>, T::AccountId>>;

	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		InfoPosted(u32, T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		PriceIsZero,
		SpaceIdInvalid,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T:Config> Pallet<T> {
		
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn post(
			origin: OriginFor<T>, 
			title: Vec<u8>, 
			content: Vec<u8>, 
			info_type: u8,
			space: u32, 
			price: BalanceOf<T>
		) -> DispatchResultWithPostInfo {
		
			let who = ensure_signed(origin)?;
			
			ensure!(price > 0u32.into(), Error::<T>::PriceIsZero);
			ensure!(space > 0u32.into(), Error::<T>::SpaceIdInvalid);

			// let space = <pallet_space::Pallet<T>>::get_spaces();
			let block_number = frame_system::Pallet::<T>::block_number();
			let index = InfoIndex::<T>::get();
			let next_index = index + 1;
			log::info!("info index {:?}", next_index);
			let info: Info<T::BlockNumber, BalanceOf<T>, T::AccountId> = Info {
				id: next_index,
				space: space,
				info_type: info_type, 
				title: title,
				content: content,
				price: price,
				creator: who.clone(),
				block: block_number,
				status: 1
			};

			Infos::<T>::insert(next_index, info.clone());

			log::info!("info: {:#?}", sp_std::str::from_utf8(&info.title));

			InfoIndex::<T>::put(next_index);
			Self::deposit_event(Event::<T>::InfoPosted(next_index, who));

			Ok(().into())
		}

	}
}