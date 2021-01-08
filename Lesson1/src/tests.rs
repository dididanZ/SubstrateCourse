use crate::{Error, mock::*};
use frame_support::{assert_ok, assert_noop,assert_err};
use sp_std::vec::Vec;
use sp_std::vec;
use super::*;


#[test]
fn create_claim_works() {
	new_test_ext().execute_with(|| {
		let claim = vec![0,1];
		assert_ok!(TemplateModule::Create_Claim(Origin::signed(1), claim.clone()));
		assert_eq!(Proofs::<Test>::get(&claim),(1, frame_system::Module::<Test>::block_number()));
	});
}

#[test]
fn create_claim_works_lenLimit() {
	new_test_ext().execute_with(|| {
		let claim = vec![0,1,3,4,5];
		assert_ok!(TemplateModule::Create_Claim(Origin::signed(1), claim.clone()));
	});
}


#[test]
fn delete_claim_works() {
	new_test_ext().execute_with(|| {
		let claim = vec![0,2];
		TemplateModule::Create_Claim(Origin::signed(2), claim.clone());
		assert_ok!(TemplateModule::Delete_Claim(Origin::signed(2), claim.clone()));
	});
}


#[test]
fn transfer_claim_works() {
	new_test_ext().execute_with(|| {
		let claim = vec![0,3];
		TemplateModule::Create_Claim(Origin::signed(1), claim.clone());
		assert_ok!(TemplateModule::Transfer_Claim(Origin::signed(1), claim.clone(), 2));
	});
}
