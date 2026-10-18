#![cfg(feature = "e2e")]
#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{
    constructor, receipt, send, watch, Account, Constructor, EventExt, Revert,
    DEFAULT_DEPLOYER_ADDRESS, DEPLOYER_ADDRESS,
};
use abi::AccessControl::{
    self, AccessControlBadConfirmation, AccessControlUnauthorizedAccount,
    RoleAdminChanged, RoleGranted, RoleRevoked,
};
use alloy::hex;
use eyre::Result;
use stylus_sdk::alloy_primitives::{Address, U256};

mod abi;

const DEFAULT_ADMIN_ROLE: [u8; 32] =
    openzeppelin_stylus::access::control::AccessControl::DEFAULT_ADMIN_ROLE;
const ROLE: [u8; 32] = access_control_example::TRANSFER_ROLE;
const NEW_ADMIN_ROLE: [u8; 32] =
    hex!("879ce0d4bfd332649ca3552efe772a38d64a315eb70ab69689fd309c735946b5");

fn ctr(admin: Address) -> Constructor {
    constructor!(admin)
}

// ============================================================================
// Integration Tests: AccessControl
// ============================================================================

#[tokio::test]
async fn constructs() -> Result<()> {
    let alice = Account::new().await?;

    let alice_addr = alice.address();
    let receipt = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?;
    let contract = AccessControl::new(receipt.contract_address, &alice.wallet);

    // `StylusDeployer` is the message sender, so currently it becomes the
    // `sender` in the `RoleGranted` event
    let stylus_deployer = std::env::var(DEPLOYER_ADDRESS)
        .unwrap_or_else(|_| DEFAULT_DEPLOYER_ADDRESS.to_owned())
        .parse()?;

    assert!(receipt.emits(RoleGranted {
        role: DEFAULT_ADMIN_ROLE.into(),
        account: alice_addr,
        sender: stylus_deployer
    }));

    let has_role =
        contract.hasRole(DEFAULT_ADMIN_ROLE.into(), alice_addr).call().await?;
    assert!(has_role);

    let member = contract
        .getRoleMember(DEFAULT_ADMIN_ROLE.into(), U256::ZERO)
        .call()
        .await?;
    assert_eq!(member, alice_addr);

    let count =
        contract.getRoleMemberCount(DEFAULT_ADMIN_ROLE.into()).call().await?;
    assert_eq!(count, U256::from(1));

    let members =
        contract.getRoleMembers(DEFAULT_ADMIN_ROLE.into()).call().await?;
    assert_eq!(members, vec![alice_addr]);

    Ok(())
}

#[tokio::test]
async fn other_roles_admin_is_the_default_admin_role() -> Result<()> {
    let alice = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    let role = contract.getRoleAdmin(ROLE.into()).call().await?;
    assert_eq!(*role, DEFAULT_ADMIN_ROLE);

    Ok(())
}

#[tokio::test]
async fn default_role_is_default_admin() -> Result<()> {
    let alice = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    let role = contract.getRoleAdmin(ROLE.into()).call().await?;
    assert_eq!(*role, DEFAULT_ADMIN_ROLE);

    let role = contract.getRoleAdmin(DEFAULT_ADMIN_ROLE.into()).call().await?;
    assert_eq!(*role, DEFAULT_ADMIN_ROLE);

    Ok(())
}

#[tokio::test]
async fn error_when_non_admin_grants_role() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &bob.wallet);

    let err = send!(contract.grantRole(ROLE.into(), alice.address()))
        .expect_err("should not have permission to grant roles");
    assert!(err.reverted_with(
        AccessControl::AccessControlUnauthorizedAccount {
            account: bob.address(),
            neededRole: DEFAULT_ADMIN_ROLE.into()
        }
    ));

    Ok(())
}

#[tokio::test]
async fn accounts_can_be_granted_roles_multiple_times() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    let alice_addr = alice.address();
    let bob_addr = bob.address();

    let receipt = receipt!(contract.grantRole(ROLE.into(), bob_addr))?;
    assert!(receipt.emits(RoleGranted {
        role: ROLE.into(),
        account: bob_addr,
        sender: alice_addr
    }));
    let receipt = receipt!(contract.grantRole(ROLE.into(), bob_addr))?;
    assert!(!receipt.emits(RoleGranted {
        role: ROLE.into(),
        account: bob_addr,
        sender: alice_addr
    }));

    let member = contract.getRoleMember(ROLE.into(), U256::ZERO).call().await?;
    assert_eq!(member, bob_addr);

    let count = contract.getRoleMemberCount(ROLE.into()).call().await?;
    assert_eq!(count, U256::from(1));

    let members = contract.getRoleMembers(ROLE.into()).call().await?;
    assert_eq!(members, vec![bob_addr]);

    Ok(())
}

#[tokio::test]
async fn not_granted_roles_can_be_revoked() -> Result<()> {
    let alice = Account::new().await?;

    let alice_addr = alice.address();
    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    let has_role = contract.hasRole(ROLE.into(), alice_addr).call().await?;
    assert!(!has_role);

    let receipt = receipt!(contract.revokeRole(ROLE.into(), alice_addr))?;
    assert!(!receipt.emits(RoleRevoked {
        role: ROLE.into(),
        account: alice_addr,
        sender: alice_addr
    }));

    Ok(())
}

#[tokio::test]
async fn admin_can_revoke_role() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    let alice_addr = alice.address();
    let bob_addr = bob.address();

    watch!(contract.grantRole(ROLE.into(), bob_addr))?;

    let receipt = receipt!(contract.revokeRole(ROLE.into(), bob_addr))?;
    assert!(receipt.emits(RoleRevoked {
        role: ROLE.into(),
        account: bob_addr,
        sender: alice_addr
    }));

    let count = contract.getRoleMemberCount(ROLE.into()).call().await?;
    assert!(count.is_zero());

    let members = contract.getRoleMembers(ROLE.into()).call().await?;
    assert!(members.is_empty());

    Ok(())
}

#[tokio::test]
async fn error_when_non_admin_revokes_role() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    let alice_addr = alice.address();
    let bob_addr = bob.address();

    watch!(contract.grantRole(ROLE.into(), alice_addr))?;

    let contract = AccessControl::new(contract_addr, &bob.wallet);
    let err = send!(contract.revokeRole(ROLE.into(), alice_addr))
        .expect_err("non-admin should not be able to revoke role");
    assert!(err.reverted_with(AccessControlUnauthorizedAccount {
        account: bob_addr,
        neededRole: DEFAULT_ADMIN_ROLE.into()
    }));

    Ok(())
}

#[tokio::test]
async fn roles_can_be_revoked_multiple_times() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    let alice_addr = alice.address();
    let bob_addr = bob.address();

    watch!(contract.revokeRole(ROLE.into(), bob_addr))?;
    let receipt = receipt!(contract.revokeRole(ROLE.into(), bob_addr))?;
    assert!(!receipt.emits(RoleRevoked {
        role: ROLE.into(),
        account: bob_addr,
        sender: alice_addr
    }));

    let count = contract.getRoleMemberCount(ROLE.into()).call().await?;
    assert!(count.is_zero());

    let members = contract.getRoleMembers(ROLE.into()).call().await?;
    assert!(members.is_empty());

    Ok(())
}

#[tokio::test]
async fn not_granted_roles_can_be_renounced() -> Result<()> {
    let alice = Account::new().await?;

    let alice_addr = alice.address();
    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    let receipt = receipt!(contract.renounceRole(ROLE.into(), alice_addr))?;
    assert!(!receipt.emits(RoleRevoked {
        role: ROLE.into(),
        account: alice_addr,
        sender: alice_addr
    }));

    Ok(())
}

#[tokio::test]
async fn bearer_can_renounce_role() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let bob_addr = bob.address();
    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    watch!(contract.grantRole(ROLE.into(), bob_addr))?;

    let contract = AccessControl::new(contract_addr, &bob.wallet);
    let receipt = receipt!(contract.renounceRole(ROLE.into(), bob_addr))?;
    assert!(receipt.emits(RoleRevoked {
        role: ROLE.into(),
        account: bob_addr,
        sender: bob_addr
    }));

    let count = contract.getRoleMemberCount(ROLE.into()).call().await?;
    assert!(count.is_zero());

    let members = contract.getRoleMembers(ROLE.into()).call().await?;
    assert!(members.is_empty());
    Ok(())
}

#[tokio::test]
async fn error_when_the_one_renouncing_is_not_the_sender() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    let alice_addr = alice.address();
    let bob_addr = bob.address();

    watch!(contract.grantRole(ROLE.into(), bob_addr))?;

    let contract = AccessControl::new(contract_addr, &bob.wallet);
    let err = send!(contract.renounceRole(ROLE.into(), alice_addr))
        .expect_err("only sender should be able to renounce");
    assert!(err.reverted_with(AccessControlBadConfirmation {}));

    Ok(())
}

#[tokio::test]
async fn roles_can_be_renounced_multiple_times() -> Result<()> {
    let alice = Account::new().await?;

    let alice_addr = alice.address();
    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    watch!(contract.renounceRole(ROLE.into(), alice_addr))?;
    let receipt = receipt!(contract.renounceRole(ROLE.into(), alice_addr))?;
    assert!(!receipt.emits(RoleRevoked {
        role: ROLE.into(),
        account: alice_addr,
        sender: alice_addr
    }));

    Ok(())
}

#[tokio::test]
async fn a_roles_admin_role_can_change() -> Result<()> {
    let alice = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    let receipt =
        receipt!(contract.setRoleAdmin(ROLE.into(), NEW_ADMIN_ROLE.into()))?;
    assert!(receipt.emits(RoleAdminChanged {
        role: ROLE.into(),
        previousAdminRole: DEFAULT_ADMIN_ROLE.into(),
        newAdminRole: NEW_ADMIN_ROLE.into()
    }));

    let role = contract.getRoleAdmin(ROLE.into()).call().await?;
    assert_eq!(*role, NEW_ADMIN_ROLE);

    Ok(())
}

#[tokio::test]
async fn the_new_admin_can_grant_roles() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    let alice_addr = alice.address();
    let bob_addr = bob.address();

    let receipt =
        receipt!(contract.setRoleAdmin(ROLE.into(), NEW_ADMIN_ROLE.into()))?;
    assert!(receipt.emits(RoleAdminChanged {
        role: ROLE.into(),
        previousAdminRole: DEFAULT_ADMIN_ROLE.into(),
        newAdminRole: NEW_ADMIN_ROLE.into()
    }));

    watch!(contract.grantRole(NEW_ADMIN_ROLE.into(), bob_addr))?;

    let contract = AccessControl::new(contract_addr, &bob.wallet);
    let receipt = receipt!(contract.grantRole(ROLE.into(), alice_addr))?;
    assert!(receipt.emits(RoleGranted {
        role: ROLE.into(),
        account: alice_addr,
        sender: bob_addr
    }));

    let member = contract.getRoleMember(ROLE.into(), U256::ZERO).call().await?;
    assert_eq!(member, alice_addr);

    let count = contract.getRoleMemberCount(ROLE.into()).call().await?;
    assert_eq!(count, U256::from(1));

    let members = contract.getRoleMembers(ROLE.into()).call().await?;
    assert_eq!(members, vec![alice_addr]);

    Ok(())
}

#[tokio::test]
async fn the_new_admin_can_revoke_roles() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    let alice_addr = alice.address();
    let bob_addr = bob.address();

    let receipt =
        receipt!(contract.setRoleAdmin(ROLE.into(), NEW_ADMIN_ROLE.into()))?;
    assert!(receipt.emits(RoleAdminChanged {
        role: ROLE.into(),
        previousAdminRole: DEFAULT_ADMIN_ROLE.into(),
        newAdminRole: NEW_ADMIN_ROLE.into()
    }));

    watch!(contract.grantRole(NEW_ADMIN_ROLE.into(), bob_addr))?;

    let contract = AccessControl::new(contract_addr, &bob.wallet);
    watch!(contract.grantRole(ROLE.into(), alice_addr))?;
    let receipt = receipt!(contract.revokeRole(ROLE.into(), alice_addr))?;
    assert!(receipt.emits(RoleRevoked {
        role: ROLE.into(),
        account: alice_addr,
        sender: bob_addr
    }));

    let count = contract.getRoleMemberCount(ROLE.into()).call().await?;
    assert!(count.is_zero());

    let members = contract.getRoleMembers(ROLE.into()).call().await?;
    assert!(members.is_empty());

    Ok(())
}

#[tokio::test]
async fn error_when_previous_admin_grants_roles() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    let alice_addr = alice.address();
    let bob_addr = bob.address();

    let receipt =
        receipt!(contract.setRoleAdmin(ROLE.into(), NEW_ADMIN_ROLE.into()))?;
    assert!(receipt.emits(RoleAdminChanged {
        role: ROLE.into(),
        previousAdminRole: DEFAULT_ADMIN_ROLE.into(),
        newAdminRole: NEW_ADMIN_ROLE.into()
    }));

    let err = send!(contract.grantRole(ROLE.into(), bob_addr))
        .expect_err("previous admins can't grant roles after admin change");
    assert!(err.reverted_with(AccessControlUnauthorizedAccount {
        account: alice_addr,
        neededRole: NEW_ADMIN_ROLE.into()
    }));

    Ok(())
}

#[tokio::test]
async fn error_when_previous_admin_revokes_roles() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = AccessControl::new(contract_addr, &alice.wallet);

    let alice_addr = alice.address();
    let bob_addr = bob.address();

    let receipt =
        receipt!(contract.setRoleAdmin(ROLE.into(), NEW_ADMIN_ROLE.into()))?;
    assert!(receipt.emits(RoleAdminChanged {
        role: ROLE.into(),
        previousAdminRole: DEFAULT_ADMIN_ROLE.into(),
        newAdminRole: NEW_ADMIN_ROLE.into()
    }));

    let err = send!(contract.revokeRole(ROLE.into(), bob_addr))
        .expect_err("previous admins can't revoke roles after admin change");
    assert!(err.reverted_with(AccessControlUnauthorizedAccount {
        account: alice_addr,
        neededRole: NEW_ADMIN_ROLE.into()
    }));

    Ok(())
}
