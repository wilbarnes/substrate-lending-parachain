use support::{ 
    decl_module, 
    decl_storage, 
    decl_event, 
    StorageValue,
    StorageMap,
    dispatch::Result, 
    ensure,
    traits::Currency, 
    traits::ReservableCurrency,
};
use system::{ ensure_signed };
use parity_codec::{ Encode, Decode };
use runtime_primitives::traits::{ As };
use runtime_primitives::{ Perbill };

#[derive(Encode, Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Terms<Balance, BlockNumber> {
    deposit: bool,
    balance: Balance,
    interest_rate: Perbill,
    start_block: BlockNumber,
    reserved: Balance,
}

pub trait Trait: system::Trait + balances::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_storage! {
	trait Store for Module<T: Trait> as Lending {
                // liquidity provider set in genesis config
                LiquidityProvider get(liquidity_provider) config(): T::AccountId;

                // Total Supply & Borrow
                // **not yet implemented**
                TotalSupply get(total_supply): u64;
                TotalBorrow get(total_borrow): u64;

                // Utilization Ratio = Borrows[a] / (Cash[a] + Borrows[a])
                // **not yet implemented**
                UtilRatio get(util_ratio): Perbill;

                // mapping of AccountId to Terms struct
                UserBalance get(user_balance): map T::AccountId => Terms<T::Balance, T::BlockNumber>;

                // rumtime special purposed array
                UserArray get(user_array): map u64 => T::AccountId;
                UserCount get(user_count): u64;
                UserIndex: map T::AccountId => u64;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		// Initializing events
		fn deposit_event<T>() = default;

                fn deposit(_origin, deposit_value: T::Balance) -> Result {
                    let sender = ensure_signed(_origin)?;

                    // user cannot deposit more to account, can only withdraw
                    ensure!(!<UserBalance<T>>::exists(&sender), 
                            "User has an existing deposit.");

                    // interest rate hard-coded at 1 percent
                    let interest_rate = Perbill::from_percent(1);

                    // set user supply terms
                    let user_terms = Terms {
                        deposit: true,
                        balance: deposit_value,
                        interest_rate: interest_rate,
                        start_block: <system::Module<T>>::block_number(),
                        reserved: <balances::Module<T>>::reserved_balance(&sender),
                    };

                    let incr_total_supply = Self::total_supply()
                        .checked_add(<T::Balance as As<u64>>::as_(deposit_value))
                        .ok_or("Overflow encourtered incrementing total supply")?;

                    // update TotalSupply to new value
                    <TotalSupply<T>>::put(incr_total_supply);

                    // insert user supply terms to storage
                    <UserBalance<T>>::insert(&sender, user_terms);

                    // increment chain specific array
                    Self::increment_array(sender.clone())?;

                    // transfer currency to liquidity provider
                    Self::transfer_funds(
                        sender.clone(),
                        Self::liquidity_provider(),
                        deposit_value,
                    )?;

                    // deposit 'CurrencySupplied' event
                    Self::deposit_event(RawEvent::CurrencySupplied(sender, deposit_value));

                    Ok(())
                }

                fn withdraw_in_full(_origin) -> Result {
                    let sender = ensure_signed(_origin)?;
                    
                    // check to make sure user has an account
                    ensure!(<UserBalance<T>>::exists(&sender), 
                            "User does not have an existing account.");

                    // retrieve user_data struct from storage
                    let mut user_data = Self::user_balance(&sender);
                    ensure!(user_data.deposit == true, 
                            "User has no supplied currency.");

                    // store balance for transfer later
                    let outgoing_balance = user_data.balance;

                    // set user balance to zero
                    user_data.balance = <T::Balance as As<u64>>::sa(0);

                    // update struct in storage
                    <UserBalance<T>>::insert(&sender, user_data);

                    // transfer money from liquidity provider to sender
                    Self::transfer_funds(
                        Self::liquidity_provider(),
                        sender.clone(),
                        outgoing_balance,
                    )?;

                    // decrement array, promoting code cleanliness
                    Self::decrement_array(sender.clone())?;

                    // deposit 'SupplyWithdrawn' event
                    Self::deposit_event(RawEvent::SupplyWithdrawn(sender, outgoing_balance));

                    Ok(())
                }

                fn borrow(_origin, borrow_value: T::Balance) -> Result {
                    let sender = ensure_signed(_origin)?;

                    // user cannot borrow more, this is a one shot loan
                    ensure!(!<UserBalance<T>>::exists(&sender), 
                            "User has an existing loan.");

                    // high interest rate for borrowers, hard-coded
                    let borrow_interest_rate = Perbill::from_percent(3);

                    let incr_total_borrow = Self::total_borrow()
                        .checked_add(<T::Balance as As<u64>>::as_(borrow_value))
                        .ok_or("Overflow encourtered incrementing total borrow")?;

                    <balances::Module<T>>::reserve(
                        &sender,
                        borrow_value,
                    )?;

                    // Update TotalSupply to new value
                    <TotalBorrow<T>>::put(incr_total_borrow);

                    // create Terms struct for user
                    let user_data = Terms {
                        deposit: false,
                        balance: borrow_value,
                        interest_rate: borrow_interest_rate,
                        start_block: <system::Module<T>>::block_number(),
                        reserved: <balances::Module<T>>::reserved_balance(&sender),
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

                    // check to make sure user has an account
                    ensure!(<UserBalance<T>>::exists(&sender), 
                            "User does not have an existing account.");

                    // retrieve user_data struct from storage
                    let mut user_data = Self::user_balance(&sender);

                    // check to ensure user has borrowed funds
                    ensure!(user_data.deposit == false, "user has not borrowed funds");

                    // store balance for transfer later
                    let outgoing_balance = user_data.balance;

                    // set user balance to zero
                    user_data.balance = <T::Balance as As<u64>>::sa(0);

                    // unreserve sender's currency
                    <balances::Module<T>>::unreserve(
                        &sender, 
                        <balances::Module<T>>::reserved_balance(&sender),
                    );

                    user_data.reserved = <balances::Module<T>>::reserved_balance(&sender);

                    // update struct in storage to reflect paid in full
                    <UserBalance<T>>::insert(&sender, user_data);

                    Self::decrement_array(sender.clone())?;

                    // perform transfer of funds
                    Self::transfer_funds(
                        sender.clone(), 
                        Self::liquidity_provider(),
                        outgoing_balance,
                    )?;

                    Self::deposit_event(RawEvent::BorrowRepaid(sender, outgoing_balance));

                    Ok(())

                }

                fn on_finalize() {
                    // existing only for the proof-of-concept
                    // in future, this will be replaced with
                    // an "Interest Rate Index" that gets updated
                    // upon any extrinsic to the runtime
                    // Index[a,n] = Index[a,n-1] * (1 + r * t)
                    
                    // this is computationally heavy, and 
                    // not good practice for blockchains

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
    // **below function not yet implemented / used**
    fn calculate_util_ratio(total_supply: u64, total_borrow: u64) -> Result {
        let mkt_liquidity = total_supply - total_borrow;
        let denominator = mkt_liquidity + total_borrow;
        let util_ratio: f64 = (total_borrow / denominator) as f64;
        // could not get the below to work
        // the below fails
        // <UtilRatio<T>>::put(Perbill::from_fraction(util_ratio));
        // the below compiles
        <UtilRatio<T>>::put(Perbill::from_percent(15));
        
        Ok(())
    }

    fn compound_interest(account_to_compound: T::AccountId) -> Result {
        let mut user_data = Self::user_balance(&account_to_compound);
        let user_balance = user_data.balance;
        let user_interest = user_data.interest_rate;

        // retrieve & update accrued interest
        let accrued = user_interest * <T::Balance as As<u64>>::as_(user_balance);
        let new_balance = <T::Balance as As<u64>>::as_(user_balance) + &accrued;

        // update terms struct to reflect updated balance
        user_data.balance = <T::Balance as As<u64>>::sa(new_balance);

        // update storage to reflect compounded interest
        <UserBalance<T>>::insert(&account_to_compound, user_data);

        Ok(())
    }

    fn transfer_funds(
        outgoing: T::AccountId, 
        incoming: T::AccountId,
        transfer_value: T::Balance
    ) -> Result {
        // while it generally takes up the same amount of space,
        // by moving to own function more logic can be added
        // to the transfer later, if needed 
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
        // retrieve current user count for runtime-purposed array
        let user_count = Self::user_count();
        let new_user_count = user_count.checked_sub(1)
            .ok_or("Underflow subtracting a new user from total users")?;

        let user_index = <UserIndex<T>>::get(&user_to_remove);

        // if sender is not the last item in the list
        if user_index != user_count {
            // set last_user as the last user in the list
            let last_user = <UserArray<T>>::get(user_count);
            // swap and pop method
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
            <T as system::Trait>::AccountId,
            <T as balances::Trait>::Balance,
        {
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
        fn if_this_fails_something_is_wrong() {
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
                             "User does not have an existing account.");
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

        #[test]
        fn user_cant_deposit_and_borrow() {
            with_externalities(&mut build(), || {
                assert_ok!(Lending::deposit(Origin::signed(2), 100));
                assert_noop!(Lending::borrow(Origin::signed(2), 100), 
                             "User has an existing loan.");

            })
        }

        #[test]
        fn user_cant_borrow_and_deposit() {
            with_externalities(&mut build(), || {
                assert_ok!(Lending::borrow(Origin::signed(2), 100));
                assert_noop!(Lending::deposit(Origin::signed(2), 100), 
                             "User has an existing deposit.");
            })
        }
}
