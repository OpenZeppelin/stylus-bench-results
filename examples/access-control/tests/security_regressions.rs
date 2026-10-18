#![cfg(feature = "e2e")]
#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{
    constructor, send, watch, Account, Constructor,
};
use abi::AccessControl;
use alloy::hex;
use eyre::Result;
use stylus_sdk::alloy_primitives::Address;

const DEFAULT_ADMIN_ROLE: [u8; 32] =
    openzeppelin_stylus::access::control::AccessControl::DEFAULT_ADMIN_ROLE;
const ROLE: [u8; 32] = access_control_example::TRANSFER_ROLE;
const NEW_ADMIN_ROLE: [u8; 32] =
    hex!("879ce0d4bfd332649ca3552efe772a38d64a315eb70ab69689fd309c735946b5");

mod abi;

fn ctr(admin: Address) -> Constructor {
    constructor!(admin)
}

#[tokio::test]
async fn non_admin_cannot_escalate_through_role_admin_mutation() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let alice_contract = AccessControl::new(contract_addr, &alice.wallet);
    let bob_contract = AccessControl::new(contract_addr, &bob.wallet);
    let bob_addr = bob.address();

    // Give Bob a routine role. He must not be able to turn it into the admin
    // role for `DEFAULT_ADMIN_ROLE`, then use it to promote himself.
    watch!(alice_contract.grantRole(ROLE.into(), bob_addr))?;

    let changed_role_admin = send!(
        bob_contract.setRoleAdmin(DEFAULT_ADMIN_ROLE.into(), ROLE.into())
    )
    .is_ok();
    let became_default_admin = if changed_role_admin {
        watch!(bob_contract.grantRole(DEFAULT_ADMIN_ROLE.into(), bob_addr))?;
        bob_contract.hasRole(DEFAULT_ADMIN_ROLE.into(), bob_addr).call().await?
    } else {
        false
    };
    let granted_another_privileged_role = if became_default_admin {
        watch!(bob_contract.grantRole(NEW_ADMIN_ROLE.into(), bob_addr))?;
        bob_contract.hasRole(NEW_ADMIN_ROLE.into(), bob_addr).call().await?
    } else {
        false
    };

    assert!(
        !became_default_admin && !granted_another_privileged_role,
        "non-admin role member escalated: default admin={became_default_admin}, additional admin role={granted_another_privileged_role}"
    );

    Ok(())
}
