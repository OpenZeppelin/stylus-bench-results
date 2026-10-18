//! This contract implements an upgradeable proxy. It is upgradeable because
//! calls are delegated to an implementation address that can be changed. This
//! address is stored in storage in the location specified by
//! [ERC-1967], so that it doesn't conflict with the storage layout of the
//! implementation behind the proxy.
//!
//! [ERC-1967]: https://eips.ethereum.org/EIPS/eip-1967
use alloc::{vec, vec::Vec};

use alloy_primitives::Address;
use stylus_sdk::{abi::Bytes, prelude::*};

use crate::proxy::IProxy;

pub mod utils;

pub use sol::*;
pub use utils::{Erc1967Utils, Error};

#[cfg_attr(coverage_nightly, coverage(off))]
mod sol {
    use alloy_sol_macro::sol;

    sol! {
        /// Emitted when the implementation is upgraded.
        #[derive(Debug)]
        #[allow(missing_docs)]
        event Upgraded(address indexed implementation);

        /// Emitted when the admin account has changed.
        #[derive(Debug)]
        #[allow(missing_docs)]
        event AdminChanged(address previous_admin, address new_admin);

        /// Emitted when the beacon is changed.
        #[derive(Debug)]
        #[allow(missing_docs)]
        event BeaconUpgraded(address indexed beacon);
    }
}

/// State of an [`Erc1967Proxy`] token.
#[storage]
pub struct Erc1967Proxy;

/// NOTE: Implementation of [`TopLevelStorage`] to be able use `&mut self` when
/// calling other contracts and not `&mut (impl TopLevelStorage +
/// BorrowMut<Self>)`. Should be fixed in the future by the Stylus team.
unsafe impl TopLevelStorage for Erc1967Proxy {}

impl Erc1967Proxy {
    /// Initializes the upgradeable proxy with an initial implementation
    /// specified by `implementation`.
    ///
    /// If `data` is nonempty, it's used as data in a delegate call to
    /// `implementation`. This will typically be an encoded function call,
    /// and allows initializing the storage of the proxy like a Solidity
    /// constructor.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `implementation` - Address of the implementation contract.
    /// * `data` - Data to pass to the implementation contract.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidImplementation`] - If the implementation is not a
    ///   valid implementation.
    /// * [`Error::NonPayable`] - If `data` is empty and the call value is
    ///   not [`U256::ZERO`][U256].
    /// * [`Error::FailedCall`] - If the call to the implementation fails.
    /// * [`Error::FailedCallWithReason`] - If the call to the implementation
    ///   fails with a revert reason.
    ///
    /// [U256]: alloy_primitives::U256
    pub fn constructor(
        &mut self,
        implementation: Address,
        data: &Bytes,
    ) -> Result<(), Error> {
        Erc1967Utils::upgrade_to_and_call(self, implementation, data)
    }
}

unsafe impl IProxy for Erc1967Proxy {
    fn implementation(&self) -> Result<Address, Vec<u8>> {
        Ok(Erc1967Utils::get_implementation(self))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::needless_pass_by_value)]

    use crate::test_support::{
        mock_code, test_address, TestContract as TestVMContract,
    };
    use alloy_sol_macro::sol;
    use alloy_sol_types::{SolCall, SolError, SolValue};
    use stylus_sdk::{
        alloy_primitives::{Address, U256},
        prelude::*,
        ArbResult,
    };

    use super::*;
    use crate::token::erc20::{self, Erc20, IErc20};

    #[entrypoint]
    #[storage]
    struct Erc1967ProxyExample {
        erc1967: Erc1967Proxy,
    }

    #[public]
    impl Erc1967ProxyExample {
        #[constructor]
        fn constructor(
            &mut self,
            implementation: Address,
            data: Bytes,
        ) -> Result<(), Error> {
            self.erc1967.constructor(implementation, &data)
        }

        fn implementation(&self) -> Result<Address, Vec<u8>> {
            self.erc1967.implementation()
        }

        #[fallback]
        fn fallback(&mut self, calldata: &[u8]) -> ArbResult {
            unsafe { self.erc1967.do_fallback(calldata) }
        }
    }

    #[storage]
    struct Erc20Example {
        erc20: Erc20,
    }

    #[public]
    #[implements(IErc20<Error = erc20::Error>)]
    impl Erc20Example {
        fn mint(
            &mut self,
            to: Address,
            value: U256,
        ) -> Result<(), erc20::Error> {
            self.erc20._mint(to, value)
        }
    }

    unsafe impl TopLevelStorage for Erc20Example {}

    #[public]
    impl IErc20 for Erc20Example {
        type Error = erc20::Error;

        fn balance_of(&self, account: Address) -> U256 {
            self.erc20.balance_of(account)
        }

        fn total_supply(&self) -> U256 {
            self.erc20.total_supply()
        }

        fn transfer(
            &mut self,
            to: Address,
            value: U256,
        ) -> Result<bool, Self::Error> {
            self.erc20.transfer(to, value)
        }

        fn transfer_from(
            &mut self,
            from: Address,
            to: Address,
            value: U256,
        ) -> Result<bool, Self::Error> {
            self.erc20.transfer_from(from, to, value)
        }

        fn allowance(&self, owner: Address, spender: Address) -> U256 {
            self.erc20.allowance(owner, spender)
        }

        fn approve(
            &mut self,
            spender: Address,
            value: U256,
        ) -> Result<bool, Self::Error> {
            self.erc20.approve(spender, value)
        }
    }

    sol! {
        interface IERC20 {
            function balanceOf(address account) external view returns (uint256);
            function totalSupply() external view returns (uint256);
            function mint(address to, uint256 value) external;
            function transfer(address to, uint256 value) external returns (bool);
        }
    }

    #[test]
    fn constructs() {
        let proxy =
            TestVMContract::<Erc1967ProxyExample>::new(stringify!(proxy));
        let erc20 = TestVMContract::<Erc20Example>::new(stringify!(erc20));
        let alice = test_address(stringify!(alice));
        mock_code(proxy.vm(), erc20.address());

        proxy
            .sender(alice)
            .constructor(erc20.address(), vec![].into())
            .expect("should be able to construct");

        let implementation = proxy
            .sender(alice)
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, erc20.address());
    }

    #[test]
    fn constructs_with_data() {
        let proxy =
            TestVMContract::<Erc1967ProxyExample>::new(stringify!(proxy));
        let erc20 = TestVMContract::<Erc20Example>::new(stringify!(erc20));
        let alice = test_address(stringify!(alice));

        let amount = U256::from(1000);

        let data = IERC20::mintCall { to: alice, value: amount }.abi_encode();
        mock_code(proxy.vm(), erc20.address());
        proxy.vm().mock_delegate_call(
            erc20.address(),
            data.clone(),
            Ok(Vec::new()),
        );

        proxy
            .sender(alice)
            .constructor(erc20.address(), data.into())
            .expect("should be able to construct");

        let implementation = proxy
            .sender(alice)
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, erc20.address());

        let balance_of_alice_call =
            IERC20::balanceOfCall { account: alice }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            balance_of_alice_call.clone(),
            Ok(amount.abi_encode()),
        );
        let balance = proxy
            .sender(alice)
            .fallback(&balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, amount.abi_encode());

        let total_supply_call = IERC20::totalSupplyCall {}.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            total_supply_call.clone(),
            Ok(amount.abi_encode()),
        );
        let total_supply = proxy
            .sender(alice)
            .fallback(&total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, amount.abi_encode());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fallback() {
        let proxy =
            TestVMContract::<Erc1967ProxyExample>::new(stringify!(proxy));
        let erc20 = TestVMContract::<Erc20Example>::new(stringify!(erc20));
        let alice = test_address(stringify!(alice));
        let bob = test_address(stringify!(bob));
        mock_code(proxy.vm(), erc20.address());

        proxy
            .sender(alice)
            .constructor(erc20.address(), vec![].into())
            .expect("should be able to construct");

        // verify initial balance is [`U256::ZERO`].
        let balance_of_alice_call =
            IERC20::balanceOfCall { account: alice }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            balance_of_alice_call.clone(),
            Ok(U256::ZERO.abi_encode()),
        );
        let balance = proxy
            .sender(alice)
            .fallback(&balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, U256::ZERO.abi_encode());

        let total_supply_call = IERC20::totalSupplyCall {}.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            total_supply_call.clone(),
            Ok(U256::ZERO.abi_encode()),
        );
        let total_supply = proxy
            .sender(alice)
            .fallback(&total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, U256::ZERO.abi_encode());

        // mint 1000 tokens.
        let amount = U256::from(1000);

        let mint_call =
            IERC20::mintCall { to: alice, value: amount }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            mint_call.clone(),
            Ok(Vec::new()),
        );
        proxy
            .sender(alice)
            .fallback(&mint_call)
            .expect("should be able to mint");
        proxy.vm().mock_delegate_call(
            erc20.address(),
            balance_of_alice_call.clone(),
            Ok(amount.abi_encode()),
        );

        // check that the balance can be accurately fetched through the proxy.
        let balance = proxy
            .sender(alice)
            .fallback(&balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, amount.abi_encode());

        proxy.vm().mock_delegate_call(
            erc20.address(),
            total_supply_call.clone(),
            Ok(amount.abi_encode()),
        );
        let total_supply = proxy
            .sender(alice)
            .fallback(&total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, amount.abi_encode());

        // check that the balance can be transferred through the proxy.
        let transfer_call =
            IERC20::transferCall { to: bob, value: amount }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            transfer_call.clone(),
            Ok(true.abi_encode()),
        );
        proxy
            .sender(alice)
            .fallback(&transfer_call)
            .expect("should be able to transfer");

        proxy.vm().mock_delegate_call(
            erc20.address(),
            balance_of_alice_call.clone(),
            Ok(U256::ZERO.abi_encode()),
        );

        let balance = proxy
            .sender(alice)
            .fallback(&balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, U256::ZERO.abi_encode());

        let balance_of_bob_call =
            IERC20::balanceOfCall { account: bob }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            balance_of_bob_call.clone(),
            Ok(amount.abi_encode()),
        );
        let balance = proxy
            .sender(alice)
            .fallback(&balance_of_bob_call)
            .expect("should be able to get balance");
        assert_eq!(balance, amount.abi_encode());

        proxy.vm().mock_delegate_call(
            erc20.address(),
            total_supply_call.clone(),
            Ok(amount.abi_encode()),
        );
        let total_supply = proxy
            .sender(alice)
            .fallback(&total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, amount.abi_encode());
    }

    #[test]
    fn fallback_returns_error() {
        let proxy =
            TestVMContract::<Erc1967ProxyExample>::new(stringify!(proxy));
        let erc20 = TestVMContract::<Erc20Example>::new(stringify!(erc20));
        let alice = test_address(stringify!(alice));
        let bob = test_address(stringify!(bob));
        mock_code(proxy.vm(), erc20.address());

        proxy
            .sender(alice)
            .constructor(erc20.address(), vec![].into())
            .expect("should be able to construct");

        let amount = U256::from(1000);
        let transfer_call =
            IERC20::transferCall { to: bob, value: amount }.abi_encode();
        let revert_data = erc20::ERC20InsufficientBalance {
            sender: alice,
            balance: U256::ZERO,
            needed: amount,
        }
        .abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            transfer_call.clone(),
            Err(revert_data.clone()),
        );
        let err = proxy
            .sender(alice)
            .fallback(&transfer_call)
            .expect_err("should revert on transfer");

        assert_eq!(err, revert_data);
    }
}
