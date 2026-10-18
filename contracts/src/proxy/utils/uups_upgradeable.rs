//! An upgradeability mechanism designed for UUPS (Universal Upgradeable Proxy
//! Standard) proxies as defined in [ERC-1822].
//!
//! [ERC-1822]: https://eips.ethereum.org/EIPS/eip-1822
//!
//! The functions included here can perform an upgrade of an
//! [`Erc1967Proxy`], when this contract is set as the implementation
//! behind such a proxy.
//!
//! [`Erc1967Proxy`]: crate::proxy::erc1967::Erc1967Proxy

pub use alloc::{string::String, vec, vec::Vec};

use alloy_primitives::{aliases::B256, uint, Address, U256, U32};
use alloy_sol_types::SolCall;
pub use sol::*;
use stylus_sdk::{
    abi::Bytes,
    prelude::*,
    storage::{StorageBool, StorageU32},
    stylus_core::calls::errors::MethodError,
};

use crate::{
    proxy::{
        erc1967::{
            self,
            utils::{
                ERC1967InvalidAdmin, ERC1967InvalidBeacon,
                ERC1967InvalidImplementation, ERC1967NonPayable, Erc1967Utils,
                IMPLEMENTATION_SLOT,
            },
        },
        utils::erc1822::{Erc1822ProxiableInterface, IErc1822Proxiable},
    },
    utils::{
        address::{self, AddressUtils},
        storage_slot::StorageSlot,
    },
};

/// The version of the upgrade interface of the contract.
pub const UPGRADE_INTERFACE_VERSION: &str = "5.0.0";

/// The version number of the logic contract.
pub const VERSION_NUMBER: U32 = uint!(1_U32);

/// A sentinel storage slot used by the implementation to distinguish
/// implementation vs. proxy ([`stylus_sdk::call::delegate_call`])
/// contexts.
///
/// The slot key is derived from `keccak256("Stylus.uups.logic.flag") - 1`,
/// chosen to avoid storage collisions with application state.
///
/// Behavior:
/// - When called directly on the implementation, `logic_flag == true`.
/// - When called via a proxy ([`stylus_sdk::call::delegate_call`]), `logic_flag ==
///   false` (i.e., the proxy’s storage does not contain this
///   implementation-only flag).
///
/// Security notes:
/// - This boolean flag replaces Solidity’s `immutable __self` pattern.
///
pub const LOGIC_FLAG_SLOT: B256 = {
    const HASH: [u8; 32] = keccak_const::Keccak256::new()
        .update(b"Stylus.uups.logic.flag")
        .finalize();
    let slot = U256::from_be_bytes(HASH)
        .wrapping_sub(uint!(1_U256))
        .to_be_bytes::<32>();

    B256::new(slot)
};

#[cfg_attr(coverage_nightly, coverage(off))]
mod sol {
    use alloy_sol_macro::sol;

    sol! {
        /// The call is from an unauthorized context.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error UUPSUnauthorizedCallContext();

        /// The storage `slot` is unsupported as a UUID.
        ///
        /// * `slot` - The unsupported UUID returned by the implementation.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error UUPSUnsupportedProxiableUUID(bytes32 slot);

        /// The contract is already initialized.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error InvalidInitialization();

        /// The version is not greater than the current version.
        ///
        /// * `current_version` - The current version.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error InvalidVersion(uint32 current_version);

        interface UUPSUpgradeableInterface {
            function setVersion() external;
        }
    }
}

/// An [`UUPSUpgradeable`] error.
#[derive(SolidityError, Debug)]
pub enum Error {
    /// The call is from an unauthorized context.
    UnauthorizedCallContext(UUPSUnauthorizedCallContext),
    /// The storage `slot` is unsupported as a UUID.
    UnsupportedProxiableUUID(UUPSUnsupportedProxiableUUID),
    /// Indicates an error related to the fact that the `implementation`
    /// of the proxy is invalid.
    InvalidImplementation(ERC1967InvalidImplementation),
    /// Indicates an error related to the fact that the `admin` of the
    /// proxy is invalid.
    InvalidAdmin(ERC1967InvalidAdmin),
    /// Indicates an error related to the fact that the `beacon`
    /// of the proxy is invalid.
    InvalidBeacon(ERC1967InvalidBeacon),
    /// Indicates an error related to the fact that an upgrade function
    /// receives nonzero transaction value that may be lost.
    NonPayable(ERC1967NonPayable),
    /// There's no code at `target` (it is not a contract).
    EmptyCode(address::AddressEmptyCode),
    /// A call to an address target failed. The target may have reverted.
    FailedCall(address::FailedCall),
    /// Indicates an error related to the fact that the
    /// [`stylus_sdk::call::delegate_call`] failed.
    FailedCallWithReason(address::FailedCallWithReason),
    /// Indicates an error related to the fact that the contract is already
    /// initialized.
    InvalidInitialization(InvalidInitialization),
    /// Indicates an error related to the fact that the version is not greater
    /// than the current version.
    InvalidVersion(InvalidVersion),
}

impl From<erc1967::utils::Error> for Error {
    fn from(e: erc1967::utils::Error) -> Self {
        match e {
            erc1967::utils::Error::InvalidImplementation(e) => {
                Error::InvalidImplementation(e)
            }
            erc1967::utils::Error::InvalidAdmin(e) => Error::InvalidAdmin(e),
            erc1967::utils::Error::InvalidBeacon(e) => Error::InvalidBeacon(e),
            erc1967::utils::Error::NonPayable(e) => Error::NonPayable(e),
            erc1967::utils::Error::EmptyCode(e) => Error::EmptyCode(e),
            erc1967::utils::Error::FailedCall(e) => Error::FailedCall(e),
            erc1967::utils::Error::FailedCallWithReason(e) => {
                Error::FailedCallWithReason(e)
            }
        }
    }
}

impl MethodError for Error {
    fn encode(self) -> alloc::vec::Vec<u8> {
        self.into()
    }
}

/// NOTE: Implementation of [`TopLevelStorage`] to be able use `&mut self` when
/// calling other contracts and not `&mut (impl TopLevelStorage +
/// BorrowMut<Self>)`. Should be fixed in the future by the Stylus team.
unsafe impl TopLevelStorage for UUPSUpgradeable {}

/// Interface for a UUPS (Universal Upgradeable Proxy Standard) upgradeable
/// contract.
#[allow(missing_docs)]
#[public]
pub trait IUUPSUpgradeable: IErc1822Proxiable {
    /// Returns the version of the upgrade interface of the contract.
    ///
    /// NOTE: Make sure to set proper selector in order to make the function
    /// compatible with Solidity version.
    ///
    /// ```rust,ignore
    /// #[selector(name = "UPGRADE_INTERFACE_VERSION")]
    /// fn upgrade_interface_version(&self) -> String {
    ///     self.uups.upgrade_interface_version()
    /// }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    fn upgrade_interface_version(&self) -> String;

    /// Upgrade the implementation of the proxy to `new_implementation`, and
    /// subsequently execute the function call encoded in `data`.
    ///
    /// Note: This primitive does not include an authorization mechanism. To
    /// restrict who can upgrade, enforce access control in your contract
    /// before delegating to this function.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `new_implementation` - The address of the new implementation contract.
    /// * `data` - Additional data to be passed to the new implementation.
    ///
    /// # Errors
    ///
    /// * [`Error::UnauthorizedCallContext`] - If the call is not made through a
    ///   valid proxy context.
    /// * [`Error::InvalidImplementation`] - If the new implementation address
    ///   is invalid or doesn't implement the required interface.
    /// * [`Error::UnsupportedProxiableUUID`] - If the new implementation
    ///   returns an unsupported UUID.
    /// * [`Error::NonPayable`] - If the upgrade function receives ETH but is
    ///   not designed to handle it.
    /// * [`Error::EmptyCode`] - If there's no code at the new implementation
    ///   address.
    /// * [`Error::FailedCall`] - If the [`stylus_sdk::call::delegate_call`] to the
    ///   new implementation fails.
    /// * [`Error::FailedCallWithReason`] - If the
    ///   [`stylus_sdk::call::delegate_call`] fails with a specific reason.
    ///
    /// # Events
    ///
    /// * [`crate::proxy::erc1967::Upgraded`]: Emitted when the implementation
    ///   is upgraded.
    ///
    /// [delegate_call]: stylus_sdk::call::delegate_call
    fn upgrade_to_and_call(
        &mut self,
        new_implementation: Address,
        data: Bytes,
    ) -> Result<(), Vec<u8>>;
}

/// A contract that implements the UUPS (Universal Upgradeable Proxy Standard)
/// pattern for upgradeable contracts.
///
/// # Overview
///
/// This implementation provides upgradeability functionality for proxy
/// contracts while maintaining storage compatibility across upgrades. It
/// follows the UUPS pattern defined in [EIP-1822], where the upgrade
/// logic is included in the implementation contract rather than the proxy.
///
/// This Stylus version contains some architectural differences compared to
/// Solidity's implementation, while maintaining the same security model and
/// upgrade flow.
///
/// # Design Rationale & Differences from Solidity
///
/// ## Storage-based Context Detection
///
/// Solidity often relies on an `immutable` self-address to detect context. In
/// Stylus, this contract uses a boolean `logic_flag` stored in a dedicated
/// slot:
/// - When executing directly on the implementation, the constructor has set
///   `logic_flag = true` in the implementation’s storage.
/// - When executing via proxy ([`stylus_sdk::call::delegate_call`]), the proxy’s
///   storage does not have this flag, so the check reads as `false`.
///
/// ## Initialization Pattern
///
/// - Constructor: runs once on implementation deployment, sets `logic_flag`.
/// - Runtime initializer: `set_version()` must be invoked via the proxy to
///   write this logic’s `VERSION_NUMBER` into the proxy’s storage. This aligns
///   the proxy’s version with the logic and enables upgrade paths guarded by
///   `only_proxy()`.
///
/// ## Proxy Safety Checks
///
/// The [`UUPSUpgradeable::only_proxy`] function ensures that:
///
/// 1. The call is being made through a [`stylus_sdk::call::delegate_call`].
/// 2. The caller is a valid [ERC-1967] proxy.
/// 3. The `VERSION_NUMBER` in implementation equals to the version stored in
///    the proxy.
///
/// **Security Note:** Bypassing these checks could allow unauthorized upgrades
/// or break the proxy pattern, potentially leading to storage collisions or
/// unauthorized upgrades.
///
/// # Edge Cases & Pitfalls
///
/// ## Common Mistakes
///
/// * The new implementation doesn't expose `set_version()` or the version
///   number constant does not increase.
/// * Calling upgrade entrypoints directly on the implementation.
/// * Using a non-ERC-1967 proxy.
/// * Incorrectly implementing `proxiable_uuid()` in derived contracts.
///
/// ## Security Considerations
///
/// * Always use the `only_proxy` modifier for upgrade functions
/// * Never expose `_upgrade_to_and_call_uups` directly
/// * Ensure all storage variables are append-only in upgrades
/// * Test upgrades thoroughly on testnet before mainnet deployment
///
/// # Usage
///
/// ```rust
/// extern crate alloc;
///
/// use openzeppelin_stylus::proxy::utils::uups_upgradeable::UUPSUpgradeable;
/// use stylus_sdk::{prelude::*, storage::StorageU256};
///
/// #[storage]
/// #[entrypoint]
/// pub struct MyUpgradeableContract {
///     // Must include UUPSUpgradeable as the first field
///     uups: UUPSUpgradeable,
///
///     // Your contract's state variables
///     value: StorageU256,
///     // ...
/// }
///
/// #[public]
/// impl MyUpgradeableContract {
///     // Call this via the proxy once to align the proxy's version with the logic.
///     pub fn set_version(&mut self) -> Result<(), Vec<u8>> {
///         self.uups.set_version().map_err(Into::into)
///     }
///
///     // Your contract's functions
/// }
/// ```
///
/// [EIP-1822]: https://eips.ethereum.org/EIPS/eip-1822
/// [ERC-1967]: https://eips.ethereum.org/EIPS/eip-1967
#[storage]
pub struct UUPSUpgradeable {
    /// Logic version number stored in the proxy contract.
    pub version: StorageU32,
}

#[public]
#[implements(IUUPSUpgradeable, IErc1822Proxiable)]
impl UUPSUpgradeable {
    /// Initializes implementation-only state used for context checks.
    ///
    /// Sets `logic_flag = true` in the implementation’s storage to indicate
    /// a direct (non-delegated) execution context.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    #[constructor]
    pub fn constructor(&mut self) {
        self.logic_flag().set(true);
    }

    /// Sets the proxy-stored runtime `version` for this logic
    /// (initializer-like).
    ///
    /// Intended to be called via a proxy ([`stylus_sdk::call::delegate_call`]) to
    /// record `VERSION_NUMBER` in the proxy’s storage.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    ///
    /// # Errors
    ///
    /// * [`Error::UnauthorizedCallContext`] - If not called via proxy
    ///   [`stylus_sdk::call::delegate_call`].
    /// * [`Error::InvalidVersion`] - If the proxy's stored `version` is greater
    ///   than this logic's `VERSION_NUMBER`.
    ///
    /// [delegate_call]: stylus_sdk::call::delegate_call
    pub fn set_version(&mut self) -> Result<(), Error> {
        if self.not_delegated().is_ok() {
            return Err(Error::UnauthorizedCallContext(
                UUPSUnauthorizedCallContext {},
            ));
        }
        if self.version.get() > VERSION_NUMBER {
            return Err(Error::InvalidVersion(InvalidVersion {
                current_version: self.version.get().to(),
            }));
        }

        self.version.set(VERSION_NUMBER);
        Ok(())
    }
}

#[public]
impl IUUPSUpgradeable for UUPSUpgradeable {
    fn upgrade_interface_version(&self) -> String {
        UPGRADE_INTERFACE_VERSION.into()
    }

    #[payable]
    fn upgrade_to_and_call(
        &mut self,
        new_implementation: Address,
        data: Bytes,
    ) -> Result<(), Vec<u8>> {
        self.only_proxy()?;
        self._upgrade_to_and_call_uups(new_implementation, &data)?;

        let data_set_version =
            UUPSUpgradeableInterface::setVersionCall {}.abi_encode();
        AddressUtils::function_delegate_call(
            self,
            new_implementation,
            &data_set_version,
        )?;

        Ok(())
    }
}

impl UUPSUpgradeable {
    /// Get the logic contract version.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    #[must_use]
    pub fn get_version(&self) -> U32 {
        VERSION_NUMBER
    }

    /// Get the logic flag from the appropriate storage slot.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    #[must_use]
    pub fn logic_flag(&self) -> StorageBool {
        StorageSlot::get_slot::<StorageBool>(
            LOGIC_FLAG_SLOT,
            (*self.vm()).clone(),
        )
    }

    /// Return the value stored in the logic flag slot.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    #[must_use]
    pub fn is_logic(&self) -> bool {
        self.logic_flag().get()
    }

    /// Ensures the call is being made through a valid [ERC-1967] proxy.
    ///
    /// Checks:
    /// 1. Execution is happening via [`stylus_sdk::call::delegate_call`] (checked
    ///    via `!self.is_logic()`).
    /// 2. The caller is a valid [ERC-1967] proxy (implementation slot is
    ///    non-zero).
    /// 3. The proxy state is consistent for this logic (the proxy-stored
    ///    version equals this logic's `VERSION_NUMBER`).
    ///
    /// [`onlyProxy`]: https://github.com/OpenZeppelin/openzeppelin-contracts/blob/c64a1edb67b6e3f4a15cca8909c9482ad33a02b0/contracts/proxy/utils/UUPSUpgradeable.sol#L50
    ///
    /// # Security Implications
    ///
    /// This check prevents direct calls to upgrade functions on the
    /// implementation contract.
    ///
    /// Note: This is not a reentrancy guard. Use a dedicated mechanism if
    /// reentrancy protection is required.
    ///
    /// # Edge Cases
    ///
    /// * Calls from [ERC-1167] minimal proxies are not guaranteed to pass this
    ///   check
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    ///
    /// # Errors
    ///
    /// * [`Error::UnauthorizedCallContext`] - If any of the above conditions is
    ///   not met
    ///
    /// [ERC-1967]: https://eips.ethereum.org/EIPS/eip-1967
    /// [ERC-1167]: https://eips.ethereum.org/EIPS/eip-1167
    /// [delegate_call]: stylus_sdk::call::delegate_call
    pub fn only_proxy(&self) -> Result<(), Error> {
        if self.is_logic()
            || Erc1967Utils::get_implementation(self) == Address::ZERO
            || U32::from(self.get_version()) != self.version.get()
        {
            Err(Error::UnauthorizedCallContext(UUPSUnauthorizedCallContext {}))
        } else {
            Ok(())
        }
    }

    /// Check that the execution is not being performed through a
    /// [`stylus_sdk::call::delegate_call`].
    ///
    /// This allows a function to be callable on the implementing contract
    /// but not through proxies.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    ///
    /// # Errors
    ///
    /// * [`Error::UnauthorizedCallContext`] - If the execution is performed via
    ///   [`stylus_sdk::call::delegate_call`].
    ///
    /// [`stylus_sdk::call::delegate_call`]: stylus_sdk::call::delegate_call
    pub fn not_delegated(&self) -> Result<(), Error> {
        if self.is_logic() {
            Ok(())
        } else {
            Err(Error::UnauthorizedCallContext(UUPSUnauthorizedCallContext {}))
        }
    }
}

#[public]
impl IErc1822Proxiable for UUPSUpgradeable {
    fn proxiable_uuid(&self) -> Result<B256, Vec<u8>> {
        self.not_delegated()?;
        Ok(IMPLEMENTATION_SLOT)
    }
}

impl UUPSUpgradeable {
    /// Performs an implementation upgrade with a security check for UUPS
    /// proxies, and additional setup call.
    ///
    /// As a security check, [`IErc1822Proxiable::proxiable_uuid`] is invoked
    /// in the new implementation, and the return value is expected to be the
    /// implementation slot in ERC-1967.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `new_implementation` - The address of the new implementation.
    /// * `data` - The data to pass to the new implementation.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidImplementation`] - If the new implementation doesn't
    ///   support the required interface or is invalid.
    /// * [`Error::UnsupportedProxiableUUID`] - If the new implementation
    ///   returns an unsupported UUID.
    /// * [`Error::NonPayable`] - If the upgrade function receives ETH but is
    ///   not designed to handle it.
    /// * [`Error::EmptyCode`] - If there's no code at the new implementation
    ///   address.
    /// * [`Error::FailedCall`] - If the [`stylus_sdk::call::delegate_call`] to the
    ///   new implementation fails.
    /// * [`Error::FailedCallWithReason`] - If the
    ///   [`stylus_sdk::call::delegate_call`] fails with a specific reason.
    ///
    /// # Events
    ///
    /// * [`crate::proxy::erc1967::Erc1967Proxy::Upgraded`]: Emitted when the
    ///   implementation is upgraded.
    ///
    /// [delegate_call]: stylus_sdk::call::delegate_call
    fn _upgrade_to_and_call_uups(
        &mut self,
        new_implementation: Address,
        data: &Bytes,
    ) -> Result<(), Error> {
        let slot = Erc1822ProxiableInterface::new(new_implementation)
            .proxiable_uuid(self.vm(), Call::new())
            .map_err(|_e| {
                Error::InvalidImplementation(ERC1967InvalidImplementation {
                    implementation: new_implementation,
                })
            })?;

        if slot == IMPLEMENTATION_SLOT {
            Erc1967Utils::upgrade_to_and_call(self, new_implementation, data)
                .map_err(Error::from)
        } else {
            Err(Error::UnsupportedProxiableUUID(UUPSUnsupportedProxiableUUID {
                slot,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::*;
    use alloy_primitives::U256;
    use alloy_sol_types::{SolCall, SolError, SolValue};
    use test_contracts::*;

    use super::*;
    use crate::token::erc20;

    #[cfg_attr(coverage_nightly, coverage(off))]
    mod test_contracts {
        use alloy_sol_macro::sol;
        use stylus_sdk::{alloy_primitives::Address, prelude::*, ArbResult};

        use super::*;
        use crate::{
            proxy::{self, erc1967::Erc1967Proxy, IProxy},
            token::erc20::{Erc20, IErc20},
        };

        #[entrypoint]
        #[storage]
        pub struct Erc1967ProxyExample {
            erc1967: Erc1967Proxy,
        }

        #[public]
        impl Erc1967ProxyExample {
            #[constructor]
            pub(super) fn constructor(
                &mut self,
                implementation: Address,
            ) -> Result<(), proxy::erc1967::utils::Error> {
                let data = ERC20Interface::setVersionCall {}.abi_encode();
                self.erc1967.constructor(implementation, &data.into())
            }

            pub(super) fn implementation(&self) -> Result<Address, Vec<u8>> {
                self.erc1967.implementation()
            }

            #[fallback]
            pub(super) fn fallback(&mut self, calldata: &[u8]) -> ArbResult {
                unsafe { self.erc1967.do_fallback(calldata) }
            }
        }

        #[storage]
        pub struct UUPSErc20Example {
            erc20: Erc20,
            uups: UUPSUpgradeable,
        }

        #[public]
        #[implements(IErc20<Error = erc20::Error>, IUUPSUpgradeable, IErc1822Proxiable)]
        impl UUPSErc20Example {
            #[constructor]
            pub(super) fn constructor(&mut self) {
                self.uups.constructor();
            }

            pub(super) fn mint(
                &mut self,
                to: Address,
                value: U256,
            ) -> Result<(), erc20::Error> {
                self.erc20._mint(to, value)
            }

            /// Initializes the contract.
            pub(super) fn set_version(&mut self) -> Result<(), Error> {
                self.uups.set_version()
            }
        }

        unsafe impl TopLevelStorage for UUPSErc20Example {}

        #[public]
        impl IErc20 for UUPSErc20Example {
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

        #[public]
        impl IUUPSUpgradeable for UUPSErc20Example {
            #[selector(name = "UPGRADE_INTERFACE_VERSION")]
            fn upgrade_interface_version(&self) -> String {
                self.uups.upgrade_interface_version()
            }

            fn upgrade_to_and_call(
                &mut self,
                new_implementation: Address,
                data: Bytes,
            ) -> Result<(), Vec<u8>> {
                self.uups.upgrade_to_and_call(new_implementation, data)
            }
        }

        #[public]
        impl IErc1822Proxiable for UUPSErc20Example {
            #[selector(name = "proxiableUUID")]
            fn proxiable_uuid(&self) -> Result<B256, Vec<u8>> {
                self.uups.proxiable_uuid()
            }
        }

        #[storage]
        pub struct FakeImplementation {}

        #[public]
        #[implements(IErc1822Proxiable)]
        impl FakeImplementation {}

        #[public]
        impl IErc1822Proxiable for FakeImplementation {
            /// Returns an incorrect UUID to simulate an invalid UUPS upgrade
            /// target.
            #[selector(name = "proxiableUUID")]
            fn proxiable_uuid(&self) -> Result<B256, Vec<u8>> {
                // Return a UUID that is NOT equal to IMPLEMENTATION_SLOT
                Ok(B256::from([0xFF; 32])) // Invalid slot
            }
        }

        unsafe impl TopLevelStorage for FakeImplementation {}

        sol! {
            interface ERC20Interface {
                function balanceOf(address account) external view returns (uint256);
                function totalSupply() external view returns (uint256);
                function mint(address to, uint256 value) external;
                function transfer(address to, uint256 value) external returns (bool);

                // Initializer function.
                function setVersion() external;
            }

            interface UUPSUpgradeableInterface {
                function upgradeToAndCall(address newImplementation, bytes calldata data) external payable;
            }

            interface ProxiableUUIDInterface {
                function proxiableUUID() external view returns (bytes32);
            }

        }
    }

    fn mock_proxy_constructor(
        proxy: &Contract<Erc1967ProxyExample>,
        logic: Address,
    ) {
        proxy.vm().mock_delegate_call(
            logic,
            ERC20Interface::setVersionCall {}.abi_encode(),
            Ok(vec![]),
        );
    }

    fn encoded_implementation(implementation: Address) -> B256 {
        let mut encoded = [0; 32];
        encoded[12..].copy_from_slice(implementation.as_slice());
        B256::from(encoded)
    }

    fn set_implementation_slot(
        vm: &stylus_sdk::testing::TestVM,
        implementation: Address,
    ) {
        vm.set_storage(
            U256::from_be_bytes(IMPLEMENTATION_SLOT.0),
            encoded_implementation(implementation),
        );
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn constructs(
        proxy: Contract<Erc1967ProxyExample>,
        logic: Contract<UUPSErc20Example>,
        alice: Address,
    ) {
        logic.sender(alice).constructor();
        mock_proxy_constructor(&proxy, logic.address());
        proxy.vm().mock_delegate_call(
            logic.address(),
            ERC20Interface::totalSupplyCall {}.abi_encode(),
            Ok(U256::ZERO.abi_encode()),
        );

        proxy
            .sender(alice)
            .constructor(logic.address())
            .expect("should be able to construct");

        let implementation = proxy
            .sender(alice)
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, logic.address());

        let total_supply_call = ERC20Interface::totalSupplyCall {}.abi_encode();
        let total_supply = proxy
            .sender(alice)
            .fallback(&total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, U256::ZERO.abi_encode());

        assert_eq!(
            UPGRADE_INTERFACE_VERSION,
            logic.sender(alice).upgrade_interface_version()
        );
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn fallback(
        proxy: Contract<Erc1967ProxyExample>,
        logic: Contract<UUPSErc20Example>,
        alice: Address,
        bob: Address,
    ) {
        logic.sender(alice).constructor();

        let balance_of_alice_call =
            ERC20Interface::balanceOfCall { account: alice }.abi_encode();
        let balance_of_bob_call =
            ERC20Interface::balanceOfCall { account: bob }.abi_encode();
        let total_supply_call = ERC20Interface::totalSupplyCall {}.abi_encode();
        let mint_call =
            ERC20Interface::mintCall { to: alice, value: U256::from(1000) }
                .abi_encode();
        let transfer_call =
            ERC20Interface::transferCall { to: bob, value: U256::from(1000) }
                .abi_encode();

        mock_proxy_constructor(&proxy, logic.address());
        proxy
            .sender(alice)
            .constructor(logic.address())
            .expect("should be able to construct");

        let amount = U256::from(1000);
        proxy.vm().mock_delegate_call(
            logic.address(),
            balance_of_alice_call.clone(),
            Ok(amount.abi_encode()),
        );
        let balance = proxy
            .sender(alice)
            .fallback(&balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, amount.abi_encode());

        proxy.vm().mock_delegate_call(
            logic.address(),
            total_supply_call.clone(),
            Ok(amount.abi_encode()),
        );
        let total_supply = proxy
            .sender(alice)
            .fallback(&total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, amount.abi_encode());

        proxy.vm().mock_delegate_call(
            logic.address(),
            mint_call.clone(),
            Ok(vec![]),
        );
        proxy
            .sender(alice)
            .fallback(&mint_call)
            .expect("should be able to mint");

        proxy.vm().mock_delegate_call(
            logic.address(),
            transfer_call.clone(),
            Ok(true.abi_encode()),
        );
        let transfer_result = proxy
            .sender(alice)
            .fallback(&transfer_call)
            .expect("should be able to transfer");
        assert_eq!(transfer_result, true.abi_encode());

        proxy.vm().mock_delegate_call(
            logic.address(),
            balance_of_bob_call.clone(),
            Ok(amount.abi_encode()),
        );
        let balance = proxy
            .sender(alice)
            .fallback(&balance_of_bob_call)
            .expect("should be able to get balance");
        assert_eq!(balance, amount.abi_encode());
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn upgrade_via_direct_call_reverts(
        logic: Contract<UUPSErc20Example>,
        logic_v2: Contract<UUPSErc20Example>,
        alice: Address,
    ) {
        logic.sender(alice).constructor();

        let err = logic
            .sender(alice)
            .upgrade_to_and_call(logic_v2.address(), vec![].into())
            .expect_error("should revert on upgrade");

        assert_eq!(err, UUPSUnauthorizedCallContext {}.abi_encode());
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn proxiable_uuid_direct_check(
        logic: Contract<UUPSErc20Example>,
        alice: Address,
    ) {
        logic.sender(alice).constructor();

        let result = logic
            .sender(alice)
            .proxiable_uuid()
            .expect("should be able to get proxiable uuid");
        assert_eq!(result, IMPLEMENTATION_SLOT);
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn get_version_number_v2(uups: Contract<UUPSUpgradeable>, alice: Address) {
        uups.sender(alice).constructor();
        assert_eq!(VERSION_NUMBER, uups.sender(alice).get_version());
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn constructor_sets_implementation_flag(
        uups: Contract<UUPSUpgradeable>,
        alice: Address,
    ) {
        assert!(!uups.sender(alice).is_logic());

        uups.sender(alice).constructor();

        assert!(uups.sender(alice).logic_flag().get());
        assert!(uups.sender(alice).is_logic());
        assert!(uups.sender(alice).not_delegated().is_ok());
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn delegated_context_rejects_implementation_only_calls(
        uups: Contract<UUPSUpgradeable>,
        alice: Address,
    ) {
        let context_error = UUPSUnauthorizedCallContext {}.abi_encode();

        assert!(matches!(
            uups.sender(alice)
                .not_delegated()
                .expect_error("proxy context must be rejected"),
            Error::UnauthorizedCallContext(_)
        ));
        assert_eq!(
            uups.sender(alice).proxiable_uuid().expect_error(
                "proxiable UUID must be unavailable through a proxy"
            ),
            context_error
        );
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn set_version_requires_proxy_context_and_monotonic_version(
        uups: Contract<UUPSUpgradeable>,
        alice: Address,
    ) {
        uups.sender(alice).constructor();
        assert!(matches!(
            uups.sender(alice)
                .set_version()
                .expect_error("implementation cannot initialize proxy version"),
            Error::UnauthorizedCallContext(_)
        ));

        let proxy_context = Contract::<UUPSUpgradeable>::new();
        proxy_context.sender(alice).version.set(VERSION_NUMBER + uint!(1_U32));
        assert!(matches!(
            proxy_context
                .sender(alice)
                .set_version()
                .expect_error("older logic cannot downgrade proxy version"),
            Error::InvalidVersion(InvalidVersion { current_version })
                if current_version
                    == (VERSION_NUMBER + uint!(1_U32)).to::<u32>()
        ));

        let current = Contract::<UUPSUpgradeable>::new();
        current.sender(alice).set_version().unwrap();
        assert_eq!(current.sender(alice).version.get(), VERSION_NUMBER);
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn only_proxy_checks_logic_flag_implementation_and_version(
        uups: Contract<UUPSUpgradeable>,
        alice: Address,
        implementation: Address,
    ) {
        let direct = Contract::<UUPSUpgradeable>::new();
        direct.sender(alice).constructor();
        direct.sender(alice).version.set(VERSION_NUMBER);
        set_implementation_slot(&direct.vm(), implementation);
        assert!(direct.sender(alice).only_proxy().is_err());

        assert!(uups.sender(alice).only_proxy().is_err());
        set_implementation_slot(&uups.vm(), implementation);
        assert!(uups.sender(alice).only_proxy().is_err());

        uups.sender(alice).version.set(VERSION_NUMBER + uint!(1_U32));
        assert!(uups.sender(alice).only_proxy().is_err());

        uups.sender(alice).version.set(VERSION_NUMBER);
        assert!(uups.sender(alice).only_proxy().is_ok());
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn uups_upgrade_rejects_invalid_and_unsupported_implementations(
        uups: Contract<UUPSUpgradeable>,
        invalid: Contract<FakeImplementation>,
        alice: Address,
    ) {
        let calldata =
            ProxiableUUIDInterface::proxiableUUIDCall {}.abi_encode();
        let data: Bytes = vec![].into();
        uups.vm().mock_static_call(
            invalid.address(),
            calldata.clone(),
            Err(vec![]),
        );
        assert!(matches!(
            uups.sender(alice)
                ._upgrade_to_and_call_uups(invalid.address(), &data),
            Err(Error::InvalidImplementation(_))
        ));

        uups.vm().mock_static_call(
            invalid.address(),
            calldata,
            Ok(B256::from([0xFF; 32]).abi_encode()),
        );
        assert!(matches!(
            uups.sender(alice)
                ._upgrade_to_and_call_uups(invalid.address(), &data)
                .unwrap_err(),
            Error::UnsupportedProxiableUUID(UUPSUnsupportedProxiableUUID { slot })
                if slot == B256::from([0xFF; 32])
        ));
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn uups_upgrade_sets_the_erc1967_implementation(
        uups: Contract<UUPSUpgradeable>,
        logic: Contract<UUPSErc20Example>,
        alice: Address,
    ) {
        let calldata =
            ProxiableUUIDInterface::proxiableUUIDCall {}.abi_encode();
        uups.vm().mock_static_call(
            logic.address(),
            calldata,
            Ok(IMPLEMENTATION_SLOT.abi_encode()),
        );
        let data: Bytes = vec![].into();

        uups.sender(alice)
            ._upgrade_to_and_call_uups(logic.address(), &data)
            .expect("valid UUPS implementation should be accepted");

        assert_eq!(
            uups.vm().get_storage(U256::from_be_bytes(IMPLEMENTATION_SLOT.0)),
            encoded_implementation(logic.address())
        );
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn upgrade_to_and_call_updates_implementation_and_runs_setup_calls(
        logic: Contract<UUPSErc20Example>,
        next_logic: Contract<UUPSErc20Example>,
        alice: Address,
    ) {
        // This fixture is intentionally in proxy context: no constructor has
        // set its implementation-only logic flag.
        logic.sender(alice).set_version().unwrap();
        set_implementation_slot(&logic.vm(), logic.address());

        let setup = ERC20Interface::totalSupplyCall {}.abi_encode();
        let initialize = ERC20Interface::setVersionCall {}.abi_encode();
        logic.vm().mock_static_call(
            next_logic.address(),
            ProxiableUUIDInterface::proxiableUUIDCall {}.abi_encode(),
            Ok(IMPLEMENTATION_SLOT.abi_encode()),
        );
        logic.vm().mock_delegate_call(
            next_logic.address(),
            setup.clone(),
            Ok(U256::from(42).abi_encode()),
        );
        logic.vm().mock_delegate_call(
            next_logic.address(),
            initialize,
            Ok(vec![]),
        );

        logic
            .sender(alice)
            .upgrade_to_and_call(next_logic.address(), setup.into())
            .expect("authorized upgrade and setup should succeed");

        assert_eq!(
            logic.vm().get_storage(U256::from_be_bytes(IMPLEMENTATION_SLOT.0)),
            encoded_implementation(next_logic.address())
        );
        logic.assert_emitted(&crate::proxy::erc1967::Upgraded {
            implementation: next_logic.address(),
        });
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn upgrade_rejects_value_when_setup_data_is_empty(
        logic: Contract<UUPSErc20Example>,
        next_logic: Contract<UUPSErc20Example>,
        alice: Address,
    ) {
        logic.sender(alice).set_version().unwrap();
        set_implementation_slot(&logic.vm(), logic.address());
        logic.vm().mock_static_call(
            next_logic.address(),
            ProxiableUUIDInterface::proxiableUUIDCall {}.abi_encode(),
            Ok(IMPLEMENTATION_SLOT.abi_encode()),
        );

        let error = logic
            .sender_and_value(alice, U256::from(1))
            .upgrade_to_and_call(next_logic.address(), vec![].into())
            .expect_error("empty setup data must not trap attached value");
        assert_eq!(error, ERC1967NonPayable {}.abi_encode());
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn upgrade_rejects_target_without_code_after_uuid_check(
        logic: Contract<UUPSErc20Example>,
        alice: Address,
    ) {
        let target = Address::from([0xBA; 20]);
        logic.sender(alice).set_version().unwrap();
        set_implementation_slot(&logic.vm(), logic.address());
        logic.vm().mock_static_call(
            target,
            ProxiableUUIDInterface::proxiableUUIDCall {}.abi_encode(),
            Ok(IMPLEMENTATION_SLOT.abi_encode()),
        );

        let error = logic
            .sender(alice)
            .upgrade_to_and_call(target, vec![].into())
            .expect_error("implementation slot must point to deployed code");
        assert_eq!(
            error,
            ERC1967InvalidImplementation { implementation: target }
                .abi_encode()
        );
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn upgrade_bubbles_delegate_failure_without_revert_data(
        logic: Contract<UUPSErc20Example>,
        next_logic: Contract<UUPSErc20Example>,
        alice: Address,
    ) {
        let setup = ERC20Interface::totalSupplyCall {}.abi_encode();
        logic.sender(alice).set_version().unwrap();
        set_implementation_slot(&logic.vm(), logic.address());
        logic.vm().mock_static_call(
            next_logic.address(),
            ProxiableUUIDInterface::proxiableUUIDCall {}.abi_encode(),
            Ok(IMPLEMENTATION_SLOT.abi_encode()),
        );
        logic.vm().mock_delegate_call(
            next_logic.address(),
            setup.clone(),
            Err(vec![]),
        );

        let error = logic
            .sender(alice)
            .upgrade_to_and_call(next_logic.address(), setup.into())
            .expect_error("delegate failure should abort upgrade");
        assert_eq!(error, address::FailedCall {}.abi_encode());
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn upgrade_bubbles_delegate_revert_reason(
        logic: Contract<UUPSErc20Example>,
        next_logic: Contract<UUPSErc20Example>,
        alice: Address,
    ) {
        let setup = ERC20Interface::totalSupplyCall {}.abi_encode();
        let reason = b"setup rejected".to_vec();
        logic.sender(alice).set_version().unwrap();
        set_implementation_slot(&logic.vm(), logic.address());
        logic.vm().mock_static_call(
            next_logic.address(),
            ProxiableUUIDInterface::proxiableUUIDCall {}.abi_encode(),
            Ok(IMPLEMENTATION_SLOT.abi_encode()),
        );
        logic.vm().mock_delegate_call(
            next_logic.address(),
            setup.clone(),
            Err(reason.clone()),
        );

        let error = logic
            .sender(alice)
            .upgrade_to_and_call(next_logic.address(), setup.into())
            .expect_error("delegate revert reason should abort upgrade");
        let expected =
            Error::FailedCallWithReason(address::FailedCallWithReason {
                reason: stylus_sdk::testing::Error::Revert(reason)
                    .encode()
                    .into(),
            })
            .encode();
        assert_eq!(error, expected);
    }
}
