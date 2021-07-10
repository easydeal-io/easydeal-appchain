#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>
use frame_support::RuntimeDebug;

use sp_std::prelude::*;

use codec::{Decode, Encode};
pub use pallet::*;

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub struct Sign<AccountId, BlockNumber> {
	total_count: u32,
	daily_count: u32,
	total_reword: u32,
	last_signed_time: u32,
	creator: AccountId,
	block: BlockNumber,
}

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		dispatch::DispatchResultWithPostInfo, PalletId,
		pallet_prelude::*,
		sp_runtime::traits::AccountIdConversion,
		traits::{Currency, ExistenceRequirement, Get},
	};
	use frame_system::pallet_prelude::*;
	
	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Currency: Currency<Self::AccountId>;
		
	}

	#[pallet::extra_constants]
    impl<T: Config> Pallet<T> {
        /// Returns the accountID for the treasury balance
        /// Transferring balance to this account funds the treasury
        pub fn account_id() -> T::AccountId {
            T::PalletId::get().into_account()
        }
    }

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::type_value]
	pub(super) fn DefaultAmount() -> u32 {
		1000_000_000u32
	}
	#[pallet::storage]
	pub(crate) type DailyRewordAmount<T> = StorageValue<_, u32, ValueQuery, DefaultAmount>;

	#[pallet::storage]
	#[pallet::getter(fn fetch_sign_info)]
	pub(crate) type SignInfo<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, Sign<T::AccountId, T::BlockNumber>>;

	// #[pallet::genesis_config]
	// #[derive(Default)]
	// pub struct GenesisConfig;

	// #[pallet::genesis_build]
	// impl<T: Config> GenesisBuild<T> for GenesisConfig {
	// 	fn build(&self) {
	// 		let account_id = T::account_id();
	// 		let min = T::Currency::minimum_balance() * 5000_000_000.into();
	// 		if T::Currency::free_balance(&account_id) < min {
	// 			let _ = T::Currency::make_free_balance_be(&account_id, min);
	// 		}
	// 	}
	// }

	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn do_sign(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let block_number = frame_system::Pallet::<T>::block_number();

			//Fetch the user signInfo
			let my_sign_info = SignInfo::<T>::get(&who); //Self::fetch_sign_info(&who);
			log::info!("my_sign_info: {:#?}", my_sign_info);
			let mut t_count = 0;
			let mut d_count = 0;
			let mut t_reword = 0;
			match my_sign_info {
				Some(data) => {
					t_count = data.total_count;
					d_count = data.daily_count;
					t_reword = data.total_reword;

					// log::info!("data: {:#?}", data.total_count);
				}
				None => {}
			}

			let total_count = t_count + 1;
			let daily_count = d_count + 1;
			let g_reword_base = DailyRewordAmount::<T>::get();
			let total_reword = t_reword + g_reword_base;

			let sign: Sign<T::AccountId, T::BlockNumber> = Sign {
				total_count: total_count,
				daily_count: daily_count,
				total_reword: total_reword,
				// isSigned: true,
				last_signed_time: total_count, //TODO time
				creator: who.clone(),
				block: block_number,
			};

			log::info!("=============who:{:#?}", who);
			// log::info!("=============alice:{:#?}", Alice.pair());
			// Send signed reword

			T::Currency::transfer(
				&Self::account_id(),
				&who,
				g_reword_base.into(),
				ExistenceRequirement::KeepAlive,
			)?;

			SignInfo::<T>::insert(who.clone(), sign);

			Ok(().into())
		}

		//  global set daily reword value
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn set_daily_base(origin: OriginFor<T>, amount: u32) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;
			//TODO amount have to bigger than zero
			// let newAmount = amount > 0
			DailyRewordAmount::<T>::put(amount);
			Ok(().into())
		}
	}
}