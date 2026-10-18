#![allow(dead_code)]

use alloy::sol;

sol! {
    #[sol(rpc)]
    contract Erc1967BeaconRollbackExample {
        function implementation() external view returns (address implementation);
        function getBeacon() external view returns (address beacon);
        function upgradeBeaconToAndCall(address newBeacon, bytes data) external;

        error ERC1967InvalidImplementation(address implementation);
    }
}
