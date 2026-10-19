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
    /// * [`Error::NonPayable`] - If `data` is empty and `self.vm().msg_value()`
    ///   is not [`U256::ZERO`][U256].
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

// Not built with `export-abi`, see `proxy::test_helpers`.
#[cfg(all(test, not(feature = "export-abi")))]
mod tests {
    #![allow(clippy::needless_pass_by_value)]

    use alloy_sol_macro::sol;
    use alloy_sol_types::{SolCall, SolError, SolValue};
    use stylus_sdk::{
        alloy_primitives::{address, Address, U256},
        prelude::*,
        testing::TestVM,
        ArbResult,
    };

    use super::*;
    use crate::{
        proxy::test_helpers::{assert_emitted, deploy, execute},
        test_utils::{ALICE, BOB},
        token::erc20::{self, Erc20, IErc20},
    };

    /// The implementation the proxy is upgraded to.
    const ERC20: Address =
        address!("0x000000000000000000000000000000000000e200");

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
        ) -> Result<bool, erc20::Error> {
            self.erc20.transfer(to, value)
        }

        fn transfer_from(
            &mut self,
            from: Address,
            to: Address,
            value: U256,
        ) -> Result<bool, erc20::Error> {
            self.erc20.transfer_from(from, to, value)
        }

        fn allowance(&self, owner: Address, spender: Address) -> U256 {
            self.erc20.allowance(owner, spender)
        }

        fn approve(
            &mut self,
            spender: Address,
            value: U256,
        ) -> Result<bool, erc20::Error> {
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

    /// Calls the proxy with `calldata`.
    ///
    /// The proxy delegates the call to [`ERC20`], so the call is mocked with
    /// the outcome of running [`Erc20Example`] over the storage of the proxy.
    fn call_proxy(
        vm: &TestVM,
        proxy: &mut Erc1967ProxyExample,
        calldata: &[u8],
    ) -> ArbResult {
        vm.mock_delegate_call(
            ERC20,
            calldata.to_vec(),
            execute::<Erc20Example>(vm, calldata),
        );
        proxy.fallback(calldata)
    }

    #[test]
    fn constructs() {
        let vm = TestVM::default();
        let mut proxy = Erc1967ProxyExample::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, ERC20);

        proxy
            .constructor(ERC20, vec![].into())
            .expect("should be able to construct");

        let implementation = proxy
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, ERC20);
    }

    #[test]
    fn constructs_with_data() {
        let vm = TestVM::default();
        let mut proxy = Erc1967ProxyExample::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, ERC20);

        let amount = U256::from(1000);

        let data = IERC20::mintCall { to: ALICE, value: amount }.abi_encode();

        // the constructor delegates the data to the implementation.
        vm.mock_delegate_call(
            ERC20,
            data.clone(),
            execute::<Erc20Example>(&vm, &data),
        );
        proxy
            .constructor(ERC20, data.into())
            .expect("should be able to construct");

        let implementation = proxy
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, ERC20);

        let balance_of_alice_call =
            IERC20::balanceOfCall { account: ALICE }.abi_encode();
        let balance = call_proxy(&vm, &mut proxy, &balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, amount.abi_encode());

        let total_supply_call = IERC20::totalSupplyCall {}.abi_encode();
        let total_supply = call_proxy(&vm, &mut proxy, &total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, amount.abi_encode());
    }

    #[test]
    fn fallback() {
        let vm = TestVM::default();
        let mut proxy = Erc1967ProxyExample::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, ERC20);

        proxy
            .constructor(ERC20, vec![].into())
            .expect("should be able to construct");

        // verify initial balance is [`U256::ZERO`].
        let balance_of_alice_call =
            IERC20::balanceOfCall { account: ALICE }.abi_encode();
        let balance = call_proxy(&vm, &mut proxy, &balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, U256::ZERO.abi_encode());

        let total_supply_call = IERC20::totalSupplyCall {}.abi_encode();
        let total_supply = call_proxy(&vm, &mut proxy, &total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, U256::ZERO.abi_encode());

        // mint 1000 tokens.
        let amount = U256::from(1000);

        let mint_call =
            IERC20::mintCall { to: ALICE, value: amount }.abi_encode();
        call_proxy(&vm, &mut proxy, &mint_call)
            .expect("should be able to mint");
        // the implementation runs in the context of the proxy, so the proxy is
        // the one that emits the event.
        assert_emitted(
            &vm,
            &erc20::Transfer { from: Address::ZERO, to: ALICE, value: amount },
        );

        // check that the balance can be accurately fetched through the proxy.
        let balance = call_proxy(&vm, &mut proxy, &balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, amount.abi_encode());

        let total_supply = call_proxy(&vm, &mut proxy, &total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, amount.abi_encode());

        // check that the balance can be transferred through the proxy.
        let transfer_call =
            IERC20::transferCall { to: BOB, value: amount }.abi_encode();
        call_proxy(&vm, &mut proxy, &transfer_call)
            .expect("should be able to transfer");

        assert_emitted(
            &vm,
            &erc20::Transfer { from: ALICE, to: BOB, value: amount },
        );

        let balance = call_proxy(&vm, &mut proxy, &balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, U256::ZERO.abi_encode());

        let balance_of_bob_call =
            IERC20::balanceOfCall { account: BOB }.abi_encode();
        let balance = call_proxy(&vm, &mut proxy, &balance_of_bob_call)
            .expect("should be able to get balance");
        assert_eq!(balance, amount.abi_encode());

        let total_supply = call_proxy(&vm, &mut proxy, &total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, amount.abi_encode());
    }

    #[test]
    fn fallback_returns_error() {
        let vm = TestVM::default();
        let mut proxy = Erc1967ProxyExample::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, ERC20);

        proxy
            .constructor(ERC20, vec![].into())
            .expect("should be able to construct");

        let amount = U256::from(1000);
        let transfer_call =
            IERC20::transferCall { to: BOB, value: amount }.abi_encode();
        let err = call_proxy(&vm, &mut proxy, &transfer_call)
            .expect_err("should revert on transfer");

        assert_eq!(
            err,
            erc20::ERC20InsufficientBalance {
                sender: ALICE,
                balance: U256::ZERO,
                needed: amount,
            }
            .abi_encode()
        );
    }
}
