#![allow(dead_code)]

use alloy::sol;

sol! {
    #[sol(rpc)]
    contract BasicToken {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
        function totalSupply() external view returns (uint256);
        function balanceOf(address account) external view returns (uint256);
        function mint(address account, uint256 amount) external;
        function transfer(address to, uint256 amount) external returns (bool);

        #[derive(Debug, PartialEq)]
        event Transfer(address indexed from, address indexed to, uint256 value);
    }
}
