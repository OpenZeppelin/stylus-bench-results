//! This is a low-level set of contracts implementing different proxy patterns
//! with and without upgradeability.
use alloc::vec::Vec;

use alloy_primitives::Address;
use stylus_sdk::{
    call, host::VM, prelude::*, stylus_core::calls::errors::Error,
};

pub mod beacon;
pub mod erc1967;
pub mod utils;

/// This trait provides a fallback function that delegates all calls to another
/// contract using the Stylus [`delegate_call`] function. We refer to the second
/// contract as the _implementation_ behind the proxy, and it has to be
/// specified by overriding the virtual [`IProxy::implementation`] function.
///
/// Additionally, delegation to the implementation can be triggered manually
/// through the [`IProxy::do_fallback`] function, or to a different contract
/// through the [`IProxy::delegate`] function.
///
/// The success and return data of the delegated call will be returned back
/// to the caller of the proxy.
///
/// The proxy reaches the host environment through [`HostAccess::vm`], so
/// implementors have to be [`HostAccess`] with the [`VM`] as their host, like
/// every `#[storage]` type is.
///
/// # Safety
///
/// This trait is unsafe to implement because it uses the `unsafe`
/// [`delegate_call`] function.
///
/// The caller must ensure that `self` is a valid contract storage context.
///
/// The caller must ensure that the implementation contract is a valid contract
/// address.
pub unsafe trait IProxy:
    TopLevelStorage + HostAccess<Host = VM> + Sized
{
    /// Delegates the current call to [`IProxy::implementation`].
    ///
    /// This function does not return to its internal call site, it will
    /// return directly to the external caller.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `implementation` - The address of the implementation contract.
    /// * `calldata` - The calldata to delegate to the implementation contract.
    ///
    /// # Errors
    ///
    /// Returns a [`stylus_sdk::stylus_core::calls::errors::Error`] if the
    /// delegate call fails. This error may represent:
    /// * A revert from the implementation contract, containing the revert data.
    /// * Failure to decode the return data from the implementation contract.
    /// * Other low-level call failures as defined by the Stylus SDK.
    #[allow(clippy::missing_safety_doc)]
    unsafe fn delegate(
        &mut self,
        implementation: Address,
        calldata: &[u8],
    ) -> Result<Vec<u8>, Error> {
        // `Call::new_mutating` only needs `self` to prove exclusive access to
        // the top-level storage. Its borrow ends right away, which lets
        // `self.vm()` be borrowed for the call itself.
        let call = Call::new_mutating(self);
        call::delegate_call(self.vm(), call, implementation, calldata)
    }

    /// This is a virtual function that should be overridden so it
    /// returns the address to which the fallback function and
    /// [`IProxy::do_fallback`] should delegate.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    ///
    /// # Errors
    ///
    /// Implementing contracts should define their own error types for this
    /// function. Typically, errors may include:
    /// * The implementation address is invalid (e.g., not a contract).
    /// * The implementation is not a contract.
    ///
    /// The error should be encoded as a `Vec<u8>`.
    fn implementation(&self) -> Result<Address, Vec<u8>>;

    /// Fallback function that delegates calls to the address returned
    /// by [`IProxy::implementation`]. Will run if no other function in the
    /// contract matches the call data.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `calldata` - The calldata to delegate to the implementation contract.
    ///
    /// # Errors
    ///
    /// Implementing contracts should define their own error types for this
    /// function. Typically, errors may include:
    /// * The implementation address is invalid (e.g., not a contract).
    /// * The implementation is not a contract.
    /// * The implementation reverted with a reason.
    ///
    /// The error should be encoded as a `Vec<u8>`.
    #[allow(clippy::missing_safety_doc)]
    unsafe fn do_fallback(
        &mut self,
        calldata: &[u8],
    ) -> Result<Vec<u8>, Vec<u8>> {
        Ok(self.delegate(self.implementation()?, calldata)?)
    }
}

// The unit tests of the proxies are not built with the `export-abi` feature.
// For the ABI export, `#[public]` defines items at the level of the module
// (like `print_from_args`), that collide as soon as a module defines several
// contracts, as the tests of the proxies do. They have no use for the export.

/// Helpers shared by the unit tests of the proxies.
///
/// [`TestVM`](stylus_sdk::testing::TestVM) holds the state of a single contract
/// and doesn't run the contracts a call is made to: the outcome of such a call
/// is whatever has been mocked for it. The helpers run those contracts, so that
/// their outcome can be mocked.
#[cfg(all(test, not(feature = "export-abi")))]
mod test_helpers {
    use core::{borrow::BorrowMut, fmt::Debug};

    use alloy_primitives::Address;
    use alloy_sol_types::SolEvent;
    use stylus_sdk::{
        abi::{router_entrypoint, Router},
        host::VM,
        prelude::*,
        testing::TestVM,
        ArbResult,
    };

    /// Gives some code to `account`, so that `vm` sees a contract there.
    pub(crate) fn deploy(vm: &TestVM, account: Address) {
        vm.set_code(account, vec![0x60, 0x00]);
    }

    /// Runs `calldata` on the contract `C` over the state held by `vm`, the
    /// way the contract's entrypoint does.
    ///
    /// A delegate call runs the code of the callee over the storage of the
    /// caller, so the outcome of a delegate call is the one of running the
    /// callee over the state of the caller's `vm`. The state changes and the
    /// logs of the callee end up in `vm` as well.
    pub(crate) fn execute<C>(vm: &TestVM, calldata: &[u8]) -> ArbResult
    where
        C: Router<C>
            + StorageType
            + TopLevelStorage
            + ValueDenier
            + BorrowMut<<C as Router<C>>::Storage>,
    {
        router_entrypoint::<C, C>(
            calldata.to_vec(),
            VM { host: Box::new(vm.clone()) },
        )
    }

    /// Asserts that `event` was emitted at least once.
    #[track_caller]
    pub(crate) fn assert_emitted<E: SolEvent + Debug>(vm: &TestVM, event: &E) {
        let log = event.encode_log_data();
        let expected = (log.topics().to_vec(), log.data.to_vec());
        let emitted = vm.get_emitted_logs();
        assert!(
            emitted.contains(&expected),
            "event was not emitted: {event:?}, emitted logs: {emitted:?}"
        );
    }
}

// Not built with `export-abi`, see `proxy::test_helpers`.
#[cfg(all(test, not(feature = "export-abi")))]
mod tests {
    use alloy_sol_macro::sol;
    use alloy_sol_types::{SolCall, SolError, SolValue};
    use stylus_sdk::{
        alloy_primitives::{address, Address, U256},
        prelude::*,
        storage::{StorageAddress, StorageBool},
        testing::TestVM,
        ArbResult,
    };

    use super::*;
    use crate::{
        proxy::test_helpers::{assert_emitted, execute},
        test_utils::{ALICE, BOB},
        token::erc20::{self, Erc20, IErc20},
    };

    /// The implementation the proxy is configured with.
    const ERC20: Address =
        address!("0x000000000000000000000000000000000000e200");
    /// Another implementation, the proxy is not configured with.
    const ERC20_2: Address =
        address!("0x000000000000000000000000000000000000e201");

    #[entrypoint]
    #[storage]
    struct ProxyExample {
        implementation: StorageAddress,
        error_on_implementation: StorageBool,
    }

    #[public]
    impl ProxyExample {
        #[constructor]
        fn constructor(&mut self, implementation: Address) {
            self.implementation.set(implementation);
        }

        fn set_error_on_implementation(
            &mut self,
            error_on_implementation: bool,
        ) {
            self.error_on_implementation.set(error_on_implementation);
        }

        fn implementation(&self) -> Result<Address, Vec<u8>> {
            if self.error_on_implementation.get() {
                return Err("implementation error".abi_encode());
            }
            Ok(self.implementation.get())
        }

        #[fallback]
        fn fallback(&mut self, calldata: &[u8]) -> ArbResult {
            unsafe { self.do_fallback(calldata) }
        }
    }

    unsafe impl IProxy for ProxyExample {
        fn implementation(&self) -> Result<Address, Vec<u8>> {
            Self::implementation(self)
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
        proxy: &mut ProxyExample,
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
        let mut proxy = ProxyExample::from(&vm);
        vm.set_sender(ALICE);

        proxy.constructor(ERC20);

        let implementation = proxy
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, ERC20);
    }

    #[test]
    fn delegate() {
        let vm = TestVM::default();
        let mut proxy = ProxyExample::from(&vm);
        vm.set_sender(ALICE);

        proxy.constructor(ERC20);

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
    fn delegate_returns_error() {
        let vm = TestVM::default();
        let mut proxy = ProxyExample::from(&vm);
        vm.set_sender(ALICE);

        proxy.constructor(ERC20);

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

    #[test]
    fn direct_delegate_to_different_implementation() {
        let vm = TestVM::default();
        let mut proxy = ProxyExample::from(&vm);
        vm.set_sender(ALICE);

        proxy.constructor(ERC20);

        // mint tokens. The second implementation runs over the storage of the
        // proxy, so this is where its balances have to be.
        let amount = U256::from(500);
        Erc20Example::from(&vm)
            .mint(ALICE, amount)
            .expect("should be able to mint");

        // use direct delegate to call the second contract. Delegating to the
        // implementation the proxy is configured with would fail.
        let balance_of_call =
            IERC20::balanceOfCall { account: ALICE }.abi_encode();
        vm.mock_delegate_call(
            ERC20,
            balance_of_call.clone(),
            Err("wrong implementation".abi_encode()),
        );
        vm.mock_delegate_call(
            ERC20_2,
            balance_of_call.clone(),
            execute::<Erc20Example>(&vm, &balance_of_call),
        );
        let balance = unsafe {
            proxy.delegate(ERC20_2, &balance_of_call).expect(
                "should be able to delegate to different implementation",
            )
        };
        assert_eq!(balance, amount.abi_encode());
    }

    #[test]
    fn fallback_reverts_on_implementation_error() {
        let vm = TestVM::default();
        let mut proxy = ProxyExample::from(&vm);
        vm.set_sender(ALICE);

        // create proxy with [`Address::ZERO`] as implementation (invalid).
        proxy.set_error_on_implementation(true);

        let balance_of_call =
            IERC20::balanceOfCall { account: ALICE }.abi_encode();
        let err = proxy
            .fallback(&balance_of_call)
            .expect_err("should fail when implementation is zero address");

        assert_eq!(err, "implementation error".abi_encode());
    }
}
