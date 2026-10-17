//! Compare the public ERC20 behavior of v0.3.0 and the working tree on devnet.
//!
//! Run with the optimized WASM artifacts as positional arguments:
//! `cargo run -p benches --bin differential_v03 -- OLD.wasm NEW.wasm`.

use alloy::{
    primitives::{Address, Bytes, B256, U256},
    rpc::types::TransactionReceipt,
    sol,
};
use eyre::{ensure, ContextCompat, Result};
use stylus_test_support::{
    constructor, deploy_wasm_file, receipt, Account, Revert,
};

sol! {
    #[sol(rpc)]
    contract Erc20 {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
        function totalSupply() external view returns (uint256);
        function balanceOf(address account) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
        function cap() external view returns (uint256);
        function mint(address account, uint256 amount) external;
        function burn(uint256 amount) external;
        function burnFrom(address account, uint256 amount) external;
        function transfer(address recipient, uint256 amount) external returns (bool);
        function approve(address spender, uint256 amount) external returns (bool);
        function transferFrom(address sender, address recipient, uint256 amount) external returns (bool);
        function paused() external view returns (bool);
        function pause() external;
        function unpause() external;

        error ERC20InvalidReceiver(address receiver);
        error ERC20ExceededCap(uint256 increased_supply, uint256 cap);
        error EnforcedPause();
    }
}

const TOKEN_NAME: &str = "Differential Token";
const TOKEN_SYMBOL: &str = "DIF";
const CAP: U256 = U256::from_limbs([100, 0, 0, 0]);
const MINT: U256 = U256::from_limbs([60, 0, 0, 0]);
const TRANSFER: U256 = U256::from_limbs([20, 0, 0, 0]);
const FIVE: U256 = U256::from_limbs([5, 0, 0, 0]);

// Keeping the two deployments' operations adjacent makes state/event parity
// straightforward to audit as one ordered scenario.
#[allow(clippy::too_many_lines)]
#[tokio::main]
async fn main() -> Result<()> {
    let mut paths = std::env::args().skip(1);
    let old_path = paths.next().context("missing v0.3.0 WASM path")?;
    let new_path = paths.next().context("missing migrated WASM path")?;
    ensure!(paths.next().is_none(), "expected exactly two WASM paths");

    let owner = Account::new().await?;
    let carol_account = Account::new().await?;
    let bob = Address::from([0x22; 20]);
    let carol = carol_account.address();
    let constructor =
        constructor!(TOKEN_NAME.to_owned(), TOKEN_SYMBOL.to_owned(), CAP);

    let old_deployment =
        deploy_wasm_file(old_path, &owner.wallet, Some(&constructor)).await?;
    let new_deployment =
        deploy_wasm_file(new_path, &owner.wallet, Some(&constructor)).await?;
    let old = Erc20::new(old_deployment.contract_address, &owner.wallet);
    let new = Erc20::new(new_deployment.contract_address, &owner.wallet);
    let owner_address = owner.address();

    ensure!(old.name().call().await? == new.name().call().await?);
    ensure!(old.symbol().call().await? == new.symbol().call().await?);
    ensure!(old.decimals().call().await? == new.decimals().call().await?);
    ensure!(old.cap().call().await? == new.cap().call().await?);
    ensure!(old.totalSupply().call().await? == new.totalSupply().call().await?);

    compare_receipts(
        &receipt!(old.mint(owner_address, MINT))?,
        &receipt!(new.mint(owner_address, MINT))?,
        "mint",
    );
    compare_receipts(
        &receipt!(old.transfer(bob, TRANSFER))?,
        &receipt!(new.transfer(bob, TRANSFER))?,
        "transfer",
    );
    compare_receipts(
        &receipt!(old.approve(carol, U256::from(10)))?,
        &receipt!(new.approve(carol, U256::from(10)))?,
        "approve",
    );

    let old_carol =
        Erc20::new(old_deployment.contract_address, &carol_account.wallet);
    let new_carol =
        Erc20::new(new_deployment.contract_address, &carol_account.wallet);
    compare_receipts(
        &receipt!(old_carol.transferFrom(owner_address, bob, FIVE))?,
        &receipt!(new_carol.transferFrom(owner_address, bob, FIVE))?,
        "transferFrom",
    );
    compare_receipts(
        &receipt!(old.burn(FIVE))?,
        &receipt!(new.burn(FIVE))?,
        "burn",
    );
    compare_receipts(
        &receipt!(old_carol.burnFrom(owner_address, FIVE))?,
        &receipt!(new_carol.burnFrom(owner_address, FIVE))?,
        "burnFrom",
    );

    ensure!(old.totalSupply().call().await? == U256::from(50));
    ensure!(new.totalSupply().call().await? == U256::from(50));
    ensure!(old.balanceOf(owner_address).call().await? == U256::from(25));
    ensure!(new.balanceOf(owner_address).call().await? == U256::from(25));
    ensure!(old.balanceOf(bob).call().await? == U256::from(25));
    ensure!(new.balanceOf(bob).call().await? == U256::from(25));
    ensure!(old.allowance(owner_address, carol).call().await? == U256::ZERO);
    ensure!(new.allowance(owner_address, carol).call().await? == U256::ZERO);

    let old_invalid = capture_error(
        old.mint(Address::ZERO, U256::ONE).call().await,
        "v0.3.0 accepted mint to the zero address",
    )?;
    let new_invalid = capture_error(
        new.mint(Address::ZERO, U256::ONE).call().await,
        "migrated contract accepted mint to the zero address",
    )?;
    ensure!(old_invalid.reverted_with(Erc20::ERC20InvalidReceiver {
        receiver: Address::ZERO,
    }));
    ensure!(new_invalid.reverted_with(Erc20::ERC20InvalidReceiver {
        receiver: Address::ZERO,
    }));
    ensure!(
        old_invalid.as_revert_data() == new_invalid.as_revert_data(),
        "invalid receiver revert payload differs"
    );

    let old_cap = capture_error(
        old.mint(owner_address, U256::from(51)).call().await,
        "v0.3.0 exceeded the supply cap",
    )?;
    let new_cap = capture_error(
        new.mint(owner_address, U256::from(51)).call().await,
        "migrated contract exceeded the supply cap",
    )?;
    ensure!(old_cap.reverted_with(Erc20::ERC20ExceededCap {
        increased_supply: U256::from(101),
        cap: CAP,
    }));
    ensure!(new_cap.reverted_with(Erc20::ERC20ExceededCap {
        increased_supply: U256::from(101),
        cap: CAP,
    }));
    ensure!(
        old_cap.as_revert_data() == new_cap.as_revert_data(),
        "cap revert payload differs"
    );

    compare_receipts(&receipt!(old.pause())?, &receipt!(new.pause())?, "pause");
    ensure!(old.paused().call().await? && new.paused().call().await?);
    let old_paused = capture_error(
        old.transfer(bob, U256::ONE).call().await,
        "v0.3.0 allowed transfer while paused",
    )?;
    let new_paused = capture_error(
        new.transfer(bob, U256::ONE).call().await,
        "migrated contract allowed transfer while paused",
    )?;
    ensure!(old_paused.reverted_with(Erc20::EnforcedPause {}));
    ensure!(new_paused.reverted_with(Erc20::EnforcedPause {}));
    ensure!(
        old_paused.as_revert_data() == new_paused.as_revert_data(),
        "pause revert payload differs"
    );
    compare_receipts(
        &receipt!(old.unpause())?,
        &receipt!(new.unpause())?,
        "unpause",
    );

    println!(
        "v0.3.0 and migrated ERC20 state, event, and revert behavior match"
    );
    Ok(())
}

fn capture_error<T>(
    result: Result<T, alloy::contract::Error>,
    message: &str,
) -> Result<alloy::contract::Error> {
    match result {
        Err(error) => Ok(error),
        Ok(_) => eyre::bail!("{message}"),
    }
}

fn compare_receipts(
    old: &TransactionReceipt,
    new: &TransactionReceipt,
    operation: &str,
) {
    fn normalized_logs(
        receipt: &TransactionReceipt,
    ) -> Vec<(Vec<B256>, Bytes)> {
        receipt
            .logs()
            .iter()
            .map(|log| {
                let log: &alloy::primitives::Log = log.as_ref();
                (log.topics().to_vec(), log.data.data.clone())
            })
            .collect()
    }

    assert_eq!(
        normalized_logs(old),
        normalized_logs(new),
        "{operation} event logs differ",
    );
}
