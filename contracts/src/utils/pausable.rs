//! Pausable Contract.
//!
//! Contract module which allows implementing an emergency stop mechanism
//! that can be triggered by an authorized account.
//!
//! It provides functions [`Pausable::when_not_paused`]
//! and [`Pausable::when_paused`],
//! which can be added to the functions of your contract.
//!
//! Note that your contract will not be pausable by simply including this
//! module, only once and where you use [`Pausable::when_not_paused`].
//!
//! Note that [`Pausable::pause`] and [`Pausable::unpause`] methods are not
//! exposed by default.
//! You should expose them manually in your contract's abi.

use alloc::{vec, vec::Vec};

pub use sol::*;
use stylus_sdk::{
    prelude::*, storage::StorageBool, stylus_core::calls::errors::MethodError,
};

#[cfg_attr(coverage_nightly, coverage(off))]
mod sol {
    use alloy_sol_macro::sol;

    sol! {
        /// Emitted when pause is triggered by `account`.
        #[derive(Debug)]
        #[allow(missing_docs)]
        event Paused(address account);

        /// Emitted when the pause is lifted by `account`.
        #[derive(Debug)]
        #[allow(missing_docs)]
        event Unpaused(address account);
    }

    sol! {
        /// Indicates an error related to the operation that failed
        /// because the contract is paused.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error EnforcedPause();

        /// Indicates an error related to the operation that failed
        /// because the contract is not paused.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ExpectedPause();
    }
}

/// A Pausable error.
#[derive(SolidityError, Debug)]
pub enum Error {
    /// Indicates an error related to the operation that failed
    /// because the contract had been in `Paused` state.
    EnforcedPause(EnforcedPause),
    /// Indicates an error related to the operation that failed
    /// because the contract had been in `Unpaused` state.
    ExpectedPause(ExpectedPause),
}

impl MethodError for Error {
    fn encode(self) -> alloc::vec::Vec<u8> {
        self.into()
    }
}

/// State of a [`Pausable`] Contract.
#[storage]
pub struct Pausable {
    /// Indicates whether the contract is `Paused`.
    pub(crate) paused: StorageBool,
}

/// Pausable interface.
#[public]
pub trait IPausable {
    /// Returns true if the contract is paused, and false otherwise.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    fn paused(&self) -> bool;
}

#[public]
#[implements(IPausable)]
impl Pausable {}

#[public]
impl IPausable for Pausable {
    fn paused(&self) -> bool {
        self.paused.get()
    }
}

impl Pausable {
    /// Triggers `Paused` state.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    ///
    /// # Errors
    ///
    /// * [`Error::EnforcedPause`] - If the contract is in `Paused` state.
    pub fn pause(&mut self) -> Result<(), Error> {
        self.when_not_paused()?;
        self.paused.set(true);
        self.vm().log(Paused { account: self.vm().msg_sender() });
        Ok(())
    }

    /// Triggers `Unpaused` state.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    ///
    /// # Errors
    ///
    /// * [`Error::ExpectedPause`] - If the contract is in `Unpaused` state.
    pub fn unpause(&mut self) -> Result<(), Error> {
        self.when_paused()?;
        self.paused.set(false);
        self.vm().log(Unpaused { account: self.vm().msg_sender() });
        Ok(())
    }

    /// Helper to make a function callable only when the contract is NOT
    /// paused.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    ///
    /// # Errors
    ///
    /// * [`Error::EnforcedPause`] - If the contract is in the `Paused` state.
    pub fn when_not_paused(&self) -> Result<(), Error> {
        if self.paused.get() {
            return Err(Error::EnforcedPause(EnforcedPause {}));
        }
        Ok(())
    }

    /// Helper to make a function callable
    /// only when the contract is paused.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    ///
    /// # Errors
    ///
    /// * [`Error::ExpectedPause`] - If the contract is in `Unpaused` state.
    pub fn when_paused(&self) -> Result<(), Error> {
        if !self.paused.get() {
            return Err(Error::ExpectedPause(ExpectedPause {}));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;
    use stylus_sdk::prelude::*;
    use stylus_sdk::testing::TestVM;

    use crate::utils::pausable::{Error, IPausable, Pausable};

    unsafe impl TopLevelStorage for Pausable {}

    #[test]
    fn paused_works() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Pausable::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused
        .set(true);
        assert!(({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused());

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused
        .set(false);
        assert!(!({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused());
    }

    #[test]
    fn when_not_paused_works() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Pausable::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused
        .set(false);

        let result = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .when_not_paused();
        assert!(result.is_ok());
    }

    #[test]
    fn when_not_paused_errors_when_paused() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Pausable::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused
        .set(true);
        assert!(({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused());

        let result = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .when_not_paused();
        assert!(matches!(result, Err(Error::EnforcedPause(_))));
    }

    #[test]
    fn when_paused_works() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Pausable::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .pause()
        .unwrap();
        assert!(({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused());

        let result = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .when_paused();
        assert!(result.is_ok());
    }

    #[test]
    fn when_paused_errors_when_not_paused() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Pausable::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused
        .set(false);
        assert!(!({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused());

        let result = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .when_paused();
        assert!(matches!(result, Err(Error::ExpectedPause(_))));
    }

    #[test]
    fn pause_works() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Pausable::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused
        .set(false);
        assert!(!({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused());

        // Pause the contract
        let res = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .pause();
        assert!(res.is_ok());
        assert!(({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused());
    }

    #[test]
    fn pause_errors_when_already_paused() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Pausable::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused
        .set(true);
        assert!(({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused());

        let result = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .pause();
        assert!(matches!(result, Err(Error::EnforcedPause(_))));
        assert!(({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused());
    }

    #[test]
    fn unpause_works() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Pausable::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused
        .set(true);
        assert!(({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused());

        // Unpause the paused contract
        let res = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .unpause();
        assert!(res.is_ok());
        assert!(!({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused());
    }

    #[test]
    fn unpause_errors_when_already_unpaused() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Pausable::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused
        .set(false);
        assert!(!({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused());

        // Unpause the unpaused contract
        let result = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .unpause();
        assert!(matches!(result, Err(Error::ExpectedPause(_))));
        assert!(!({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .paused());
    }
}
