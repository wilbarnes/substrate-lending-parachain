/// A runtime module template with necessary imports

/// Feel free to remove or edit this file as needed.
/// If you change the name of this file, make sure to update its references in runtime/src/lib.rs
/// If you remove this file, you can remove those references

/// For more guidance on Substrate modules, see the example module
/// https://github.com/paritytech/substrate/blob/master/srml/example/src/lib.rs

use support::{ 
    decl_module, 
    decl_storage, 
    decl_event, 
    StorageValue,
    StorageMap,
    dispatch::Result, 
    ensure,
    traits::Currency, 
};
use system::{ ensure_signed };
use parity_codec::{ Encode, Decode };
use runtime_primitives::traits::{ As, Hash, Zero };
use runtime_primitives::{ Perbill };

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Terms<Balance, BlockNumber> {
    supplying: bool,
    balance: Balance,
    interest_rate: Perbill,
    start_block: BlockNumber,
    end_block: u64,
}

/// The module's configuration trait.
pub trait Trait: system::Trait + balances::Trait {
	// TODO: Add other types and constants required configure this module.

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

/// This module's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as lending {
                LiquidityProvider get(liquidity_provider) config(): T::AccountId;

                UserBalance get(user_balance): map T::AccountId => Terms<T::Balance, T::BlockNumber>;

                // rumtime special purposed array
                UserArray get(user_array): map u64 => T::AccountId;
                UserCount get(user_count): u64;
                UserIndex: map T::AccountId => u64;

                AccruedInterest get(accrued_interest): T::Balance;
                InterestRate get(interest_rate): Perbill;

                Nonce: u64;
	}
}

decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		// Initializing events
		fn deposit_event<T>() = default;

                fn deposit(_origin, deposit_value: T::Balance) -> Result {
                    let sender = ensure_signed(_origin)?;

                    let liquidity_src = Self::liquidity_provider();
                    let nonce = <Nonce<T>>::get();

                    let interest_rate = Perbill::from_percent(1);
                    <InterestRate<T>>::put(interest_rate);

                    // let current_block = <system::Module<T>>::block_number();

                    let user_terms = Terms {
                        supplying: true,
                        balance: deposit_value,
                        interest_rate: interest_rate,
                        start_block: <system::Module<T>>::block_number(),
                        end_block: 0,
                    };

                    <UserBalance<T>>::insert(&sender, user_terms);

                    Self::increment_array(sender.clone())?;

                    // transfer currency to liquidity provider
                    <balances::Module<T> as Currency<_>>::transfer(
                        &sender,
                        &liquidity_src,
                        deposit_value,
                    )?;

                    Ok(())
                }

                fn withdraw_in_full(_origin) -> Result {
                    let sender = ensure_signed(_origin)?;
                    let liquidity_src = Self::liquidity_provider();

                    // retrieve user_data struct from storage
                    let mut user_data = Self::user_balance(&sender);
                    ensure!(user_data.supplying == true, "User has no supplied currency");

                    // store balance for transfer later
                    let outgoing_balance = user_data.balance;

                    // set user balance to zero
                    user_data.balance = <T::Balance as As<u64>>::sa(0);

                    // update struct in storage
                    <UserBalance<T>>::insert(&sender, user_data);

                    // transfer money from liquidity provider to sender
                    <balances::Module<T> as Currency<_>>::transfer(
                        &liquidity_src,
                        &sender,
                        outgoing_balance,
                    )?;

                    // "helper" function decrement array, promoting code cleanliness
                    Self::decrement_array(sender.clone())?;

                    Ok(())
                }

                fn borrow(_origin, borrow_value: T::Balance) -> Result {
                    let sender = ensure_signed(_origin)?;
                    let liquidity_src = Self::liquidity_provider();

                    // high interest rate for borrowers
                    let borrow_interest_rate = Perbill::from_percent(25);
                    let current_block = <system::Module<T>>::block_number();

                    let user_data = Terms {
                        supplying: false,
                        balance: borrow_value,
                        interest_rate: borrow_interest_rate,
                        start_block: current_block,
                        end_block: 0,
                    };

                    <UserBalance<T>>::insert(&sender, &user_data);

                    Self::increment_array(sender.clone())?;

                    // // retrieve current user count for rumtime-purposed array
                    // let user_count = Self::user_count();
                    // // check for overflows
                    // let new_user_count = user_count.checked_add(1)
                    //     .ok_or("Overflow adding a new user to total users")?;

                    // <UserBalance<T>>::insert(&sender, &user_data);

                    // <UserArray<T>>::insert(user_count, &sender);
                    // <UserCount<T>>::put(new_user_count);
                    // <UserIndex<T>>::insert(&sender, user_count);

                    <balances::Module<T> as Currency<_>>::transfer(
                        &liquidity_src,
                        &sender,
                        borrow_value,
                    )?;

                    Ok(())
                }

                fn repay_in_full(_origin) -> Result {
                    let sender = ensure_signed(_origin)?;

                    // retrieve user_data struct from storage
                    let mut user_data = Self::user_balance(&sender);
                    // check to ensure user has borrowed funds
                    ensure!(user_data.supplying == false, "user has not borrowed funds");

                    // store balance for transfer later
                    let outgoing_balance = user_data.balance;

                    // set user balance to zero
                    user_data.balance = <T::Balance as As<u64>>::sa(0);

                    Self::decrement_array(sender.clone())?;

                    // perform transfer of funds
                    Self::transfer_funds(
                        sender.clone(), 
                        Self::liquidity_provider(),
                        outgoing_balance,
                    )?;

                    // update struct in storage to reflect paid in full
                    <UserBalance<T>>::insert(&sender, user_data);

                    Ok(())

                }

                fn on_finalize() {
                    // retrieve user count to iterate over
                    let user_count = Self::user_count();

                    // iterate over open accounts, compounding interest
                    for each in 0..user_count {
                        let addr = Self::user_array(each);
                        Self::compound_interest(addr);
                    }
                }
	}
}

impl<T: Trait> Module<T> {
    fn calculate_and_render_interest(to: T::AccountId) -> Result {
        // TODO: add ensure statement here

        let mut user_data = Self::user_balance(&to);

        let end_block = <system::Module<T>>::block_number();
        // let end_block_prim = <T::BlockNumber as As<u64>>::as_(end_block);

        // compound interest time period
        let type_to_convert = end_block - user_data.start_block;
        let compounding_periods = <T::BlockNumber as As<u64>>::as_(type_to_convert);

        // grab substrate types
        let bal = user_data.balance;
        let int = user_data.interest_rate;

        // convert to rust primitive  
        let mut working_bal = <T::Balance as As<u64>>::as_(bal);

        // loop through the blocks and calculate compounding interest
        for _ in 0..compounding_periods {
            let compounded = int * working_bal;
            working_bal += compounded;
        }

        let updated_balance = <T::Balance as As<u64>>::sa(working_bal);
        user_data.balance = updated_balance;

        <UserBalance<T>>::insert(&to, user_data);

        Ok(())
    }

    fn compound_interest(account_to_compound: T::AccountId) -> Result {

        let mut user_data = Self::user_balance(&account_to_compound);
        let user_bal = user_data.balance;
        let user_int = user_data.interest_rate;
        let conv_bal = <T::Balance as As<u64>>::as_(user_bal);

        let accrual = Perbill::from_percent(1) * conv_bal;
        let upd_bal = conv_bal + &accrual;
        let rev_bal = <T::Balance as As<u64>>::sa(upd_bal);

        user_data.balance = rev_bal;
        user_data.interest_rate = user_int;

        // update storage to reflect compounded interest
        <UserBalance<T>>::insert(&account_to_compound, user_data);
        <AccruedInterest<T>>::put(&rev_bal);

        Ok(())
    }

    fn transfer_funds(
        outgoing: T::AccountId, 
        incoming: T::AccountId,
        transfer_value: T::Balance
    ) -> Result {
        <balances::Module<T> as Currency<_>>::transfer(
            &outgoing,
            &incoming,
            transfer_value,
        )?;

        Ok(())
    }

    fn increment_array(user_to_add: T::AccountId) -> Result {
        // retrieve current user count for rumtime-purposed array
        let user_count = Self::user_count();
        // check for overflows
        let new_user_count = user_count.checked_add(1)
            .ok_or("Overflow adding a new user to total users")?;

        // <UserBalance<T>>::insert(&user_to_add, user_terms);

        <UserArray<T>>::insert(user_count, &user_to_add);
        <UserCount<T>>::put(new_user_count);
        <UserIndex<T>>::insert(&user_to_add, user_count);

        Ok(())
    }

    fn decrement_array(user_to_remove: T::AccountId) -> Result {
        
        let user_count = Self::user_count();
        let new_user_count = user_count.checked_sub(1)
            .ok_or("Underflow subtracting a new user from total users")?;

        let user_index = <UserIndex<T>>::get(&user_to_remove);

        // if sender is not the last item in the list
        if user_index != user_count {
            // set last_user as the last user in the list
            let last_user = <UserArray<T>>::get(user_count);
            // 
            <UserArray<T>>::insert(&user_count, &last_user);
            <UserIndex<T>>::insert(&last_user, user_index);
        }
        <UserArray<T>>::remove(&user_count);
        <UserIndex<T>>::remove(&user_to_remove);
        <UserCount<T>>::put(new_user_count);
        <UserBalance<T>>::remove(user_to_remove);

        // // retrieve current user count for rumtime-purposed array
        // let user_count = Self::user_count();
        // // check for overflows
        // let new_user_count = user_count.checked_sub(1)
        //     .ok_or("Underflow subtracting a new user to total users")?;

        // <UserBalance<T>>::insert(&user_to_remove, user_terms);

        // <UserArray<T>>::insert(user_count, &user_to_remove);
        // <UserCount<T>>::put(new_user_count);
        // <UserIndex<T>>::insert(&user_to_remove, user_count);
        
        Ok(())
    }
}

decl_event!(
	pub enum Event<T> where AccountId = <T as system::Trait>::AccountId {
		// Just a dummy event.
		// Event `Something` is declared with a parameter of the type `u32` and `AccountId`
		// To emit this event, we call the deposit funtion, from our runtime funtions
		SomethingStored(u32, AccountId),
	}
);

/// tests for this module
#[cfg(test)]
mod tests {
	use super::*;

	use runtime_io::with_externalities;
	use primitives::{H256, Blake2Hasher};
	use support::{impl_outer_origin, assert_ok};
	use runtime_primitives::{
		BuildStorage,
		traits::{BlakeTwo256, IdentityLookup},
		testing::{Digest, DigestItem, Header}
	};

	impl_outer_origin! {
		pub enum Origin for Test {}
	}

	// For testing the module, we construct most of a mock runtime. This means
	// first constructing a configuration type (`Test`) which `impl`s each of the
	// configuration traits of modules we want to use.
	#[derive(Clone, Eq, PartialEq)]
	pub struct Test;
	impl system::Trait for Test {
		type Origin = Origin;
		type Index = u64;
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type Digest = Digest;
		type AccountId = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = ();
		type Log = DigestItem;
	}
	impl Trait for Test {
		type Event = ();
	}
	type lending = Module<Test>;

	// This function basically just builds a genesis storage key/value store according to
	// our desired mockup.
	fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
		system::GenesisConfig::<Test>::default().build_storage().unwrap().0.into()
	}

	#[test]
	fn it_works_for_default_value() {
		with_externalities(&mut new_test_ext(), || {
			// Just a dummy test for the dummy funtion `do_something`
			// calling the `do_something` function with a value 42
			assert_ok!(lending::do_something(Origin::signed(1), 42));
			// asserting that the stored value is equal to what we stored
			assert_eq!(lending::something(), Some(42));
		});
	}
}
