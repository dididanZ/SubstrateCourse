#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// https://substrate.dev/docs/en/knowledgebase/runtime/frame

use frame_support::{decl_module, decl_storage, decl_event, decl_error, dispatch, traits::Get, ensure};
use frame_system::ensure_signed;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
use sp_std::vec::Vec;
use sp_std::vec;
pub trait Trait: frame_system::Trait {

	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	type ClaimLenLimit : Get<u32>;
}

decl_storage! {

	trait Store for Module<T: Trait> as TemplateModule {

		pub Proofs get(fn proofs): map
		    hasher(blake2_128_concat) Vec<u8> => (T::AccountId, T::BlockNumber);
	}
}

decl_event!(
	pub enum Event<T> where AccountId = <T as frame_system::Trait>::AccountId {

		Create(AccountId,Vec<u8>),
		Remove(AccountId, Vec<u8>),
		Transfer(AccountId, Vec<u8>,AccountId),
	}
);

decl_error! {
	pub enum Error for Module<T: Trait> {
		ClaimExist,
		ClaimNotExist,
		NotClaimOwner,
		ClaimLenghtLimit,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		type Error = Error<T>;

		fn deposit_event() = default;
		const ClaimLenLimit: u32 = T::ClaimLenLimit::get();
		#[weight = 0]
		pub fn Create_Claim(origin, claim :Vec<u8>) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(claim.len() as u32 <=  T::ClaimLenLimit::get()   ,<Error<T>>::ClaimLenghtLimit );
			  ensure!(!Proofs::<T>::contains_key(&claim),<Error<T>>::ClaimExist);
				Proofs::<T>::insert(&claim,(who.clone(),frame_system::Module::<T>::block_number()));
			Self::deposit_event(RawEvent::Create(who, claim));
			Ok(())
		}

		#[weight = 0]
		pub fn Delete_Claim(origin, claim :Vec<u8>) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(Proofs::<T>::contains_key(&claim),<Error<T>>::ClaimNotExist);
			Proofs::<T>::remove(&claim);
			Self::deposit_event(RawEvent::Remove(who,claim));
			Ok(())
		}
		#[weight = 0]
		pub fn Transfer_Claim(origin,claim:Vec<u8>, dest:T::AccountId)-> dispatch::DispatchResult{
			let who = ensure_signed(origin)?;
			ensure!(Proofs::<T>::contains_key(&claim),<Error<T>>::ClaimNotExist);
			let (owner, _number) = 	Proofs::<T>::get(&claim);
				ensure!(owner == who,<Error<T>>::NotClaimOwner);
					Proofs::<T>::insert(&claim,(dest.clone(),frame_system::Module::<T>::block_number()));
			Self::deposit_event(RawEvent::Transfer(who,claim,dest));
					Ok(())
		}
	}
}
