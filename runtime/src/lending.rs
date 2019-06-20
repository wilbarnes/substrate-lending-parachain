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
use system::{ ensure_signed, ensure_root };
use parity_codec::{ Encode, Decode };
use runtime_primitives::traits::{ As };
use runtime_primitives::{ Perbill };

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Terms<Balance, BlockNumber> {
    supplying: bool,
    balance: Balance,
    interest_rate: Perbill,
    start_block: BlockNumber,
}

/// The module's configuration trait.
pub trait Trait: system::Trait + balances::Trait {
	// TODO: Add other types and constants required configure this module.

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

/// This module's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as Lending {
                LiquidityProvider get(liquidity_provider) config(): T::AccountId;

                UserBalance get(user_balance): map T::AccountId => Terms<T::Balance, T::BlockNumber>;

                // rumtime special purposed array
                UserArray get(user_array): map u64 => T::AccountId;
                UserCount get(user_count): u64;
                UserIndex: map T::AccountId => u64;

                TotalSupply get(total_supply): u64;
                TotalBorrow get(total_borrow): u64;

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

                    let user_terms = Terms {
                        supplying: true,
                        balance: deposit_value,
                        interest_rate: interest_rate,
                        start_block: <system::Module<T>>::block_number(),
                    };

                    <UserBalance<T>>::insert(&sender, user_terms);

                    Self::increment_array(sender.clone())?;

                    // transfer currency to liquidity provider
                    <balances::Module<T> as Currency<_>>::transfer(
                        &sender,
                        &liquidity_src,
                        deposit_value,
                    )?;

                    // deposit 'CurrencySupplied' event
                    Self::deposit_event(RawEvent::CurrencySupplied(sender, deposit_value));

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

                    // deposit 'SupplyWithdrawn' event
                    Self::deposit_event(RawEvent::SupplyWithdrawn(sender, outgoing_balance));

                    Ok(())
                }

                fn borrow(_origin, borrow_value: T::Balance) -> Result {
                    let sender = ensure_signed(_origin)?;

                    // high interest rate for borrowers, hard-coded
                    let borrow_interest_rate = Perbill::from_percent(25);
                    let current_block = <system::Module<T>>::block_number();

                    let user_data = Terms {
                        supplying: false,
                        balance: borrow_value,
                        interest_rate: borrow_interest_rate,
                        start_block: current_block,
                    };

                    // add struct to storage
                    <UserBalance<T>>::insert(&sender, &user_data);

                    // increment chain specific array
                    Self::increment_array(sender.clone())?;

                    // perform transfer of funds
                    Self::transfer_funds(
                        Self::liquidity_provider(),
                        sender.clone(),
                        borrow_value,
                    )?;

                    Self::deposit_event(RawEvent::CurrencyBorrowed(sender, borrow_value));

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

                    Self::deposit_event(RawEvent::BorrowRepaid(sender, outgoing_balance));

                    Ok(())

                }

                fn on_finalize() {
                    // retrieve user count to iterate over
                    let user_count = Self::user_count();

                    // iterate over open accounts
                    for each in 0..user_count {
                        // retrieve address
                        let addr = Self::user_array(each);
                        // compound interest of each account
                        Self::compound_interest(addr);
                    }
                }
	}
}

impl<T: Trait> Module<T> {
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

        Ok(())
    }
}

decl_event!(
	pub enum Event<T> 
        where 
            // AccountId = <T as system::Trait>::AccountId 
            <T as system::Trait>::AccountId,
            <T as balances::Trait>::Balance,
        {
                LiquidityProvidedChanged(AccountId, AccountId),
                CurrencySupplied(AccountId, Balance),
                CurrencyBorrowed(AccountId, Balance),
                SupplyWithdrawn(AccountId, Balance),
                BorrowRepaid(AccountId, Balance),
	}
);

/// tests for this module
#[cfg(test)]
mod tests {
	use super::*;

	use runtime_io::with_externalities;
	use primitives::{H256, Blake2Hasher};
	use support::{ 
            impl_outer_origin, 
            assert_ok, 
            assert_noop 
        };
	use runtime_primitives::{
		BuildStorage,
		traits::{BlakeTwo256, IdentityLookup},
		testing::{Digest, DigestItem, Header}
	};

	impl_outer_origin! {
		pub enum Origin for Test {}
	}

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

	impl super::Trait for Test {
		type Event = ();
        }

        impl balances::Trait for Test {
                type Balance = u128;
                type OnFreeBalanceZero = ();
                type OnNewAccount = ();
                type Event = ();

                type TransactionPayment = ();
                type DustRemoval = ();
                type TransferPayment = ();
	}

	type Lending = Module<Test>;

	fn build() -> runtime_io::TestExternalities<Blake2Hasher> {
		let mut t = system::GenesisConfig::<Test>::default()
                    .build_storage()
                    .unwrap()
                    .0;
                t.extend(balances::GenesisConfig::<Test> {
                    transaction_base_fee: 0,
                    transaction_byte_fee: 0,
                    existential_deposit: 0,
                    transfer_fee: 0,
                    creation_fee: 0,
                    balances: vec![
                        (1, 1_000_000), // Alice in 'chain_spec.rs' (figuratively)
                        (2, 1_000_000), // Bob ''
                        (3, 1_000_000), // Charlie ''
                        (4, 1_000_000)], // Dave ''
                    vesting: vec![],
                    }
                    .build_storage()
                    .unwrap()
                    .0,
                    );

                t.extend(
                    GenesisConfig::<Test> {
                        liquidity_provider: 1,
                    }
                    .build_storage()
                    .unwrap()
                    .0,
                );
                t.into()
	}

        #[test]
        fn kicking_the_tires() {
            with_externalities(&mut build(), || {
                assert!(true);
            })
        }

	#[test]
	fn user_can_make_a_deposit() {
            with_externalities(&mut build(), || { 
                assert_ok!(Lending::deposit(Origin::signed(2), 100));
                assert_ok!(Lending::deposit(Origin::signed(3), 100));
            });
	}

        #[test]
        fn user_can_make_a_withdraw() {
            with_externalities(&mut build(), || {
                assert_ok!(Lending::deposit(Origin::signed(2), 100));
                assert_ok!(Lending::withdraw_in_full(Origin::signed(2)));
            });
        }

        #[test]
        fn user_cant_withraw_without_deposit() {
            with_externalities(&mut build(), || {
                assert_noop!(Lending::withdraw_in_full(Origin::signed(2)), 
                             "User has no supplied currency");
            });
        }

        #[test]
        fn check_liquidity_provider() {
            with_externalities(&mut build(), || {
                assert_eq!(Lending::liquidity_provider(), 1);
            });
        }

        #[test]
        fn user_can_borrow() {
            with_externalities(&mut build(), || {
                assert_ok!(Lending::borrow(Origin::signed(2), 100));
            });
        }

        #[test]
        fn user_count_increments_when_supplying() {
            with_externalities(&mut build(), || {
                assert_ok!(Lending::deposit(Origin::signed(2), 100));
                assert_eq!(Lending::user_count(), 1);
            });
        }

        #[test]
        fn user_count_decrements_when_withdrawing() {
            with_externalities(&mut build(), || {
                assert_ok!(Lending::deposit(Origin::signed(2), 100));
                assert_eq!(Lending::user_count(), 1);
                assert_ok!(Lending::withdraw_in_full(Origin::signed(2)));
                assert_eq!(Lending::user_count(), 0);
            });
        }

        #[test]
        fn user_count_increments_when_borrowing() {
            with_externalities(&mut build(), || {
                assert_ok!(Lending::borrow(Origin::signed(2), 100));
                assert_eq!(Lending::user_count(), 1);
            });
        }

        #[test]
        fn user_count_decrements_when_repaid() {
            with_externalities(&mut build(), || {
                assert_ok!(Lending::borrow(Origin::signed(2), 100));
                assert_eq!(Lending::user_count(), 1);
                assert_ok!(Lending::repay_in_full(Origin::signed(2)));
                assert_eq!(Lending::user_count(), 0);
            });
        }

}
