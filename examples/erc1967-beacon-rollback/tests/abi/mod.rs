#![allow(dead_code)]

use alloy::sol;

sol! {
    #[sol(rpc)]
    contract Erc1967BeaconRollbackExample {
        function implementation() external view returns (address implementation);
        function getBeacon() external view returns (address beacon);
        function upgradeBeaconToAndCall(address newBeacon, bytes data) external;
        function owner() external view returns (address);
        function initializeOwner(address initialOwner) external;
        function transferOwnership(address newOwner) external;
        function renounceOwnership() external;

        error ERC1967InvalidImplementation(address implementation);
        error OwnableUnauthorizedAccount(address account);
        error OwnableInvalidOwner(address owner);
    }
}
