#![allow(dead_code)]

use alloy::sol;

sol! {
    #[sol(rpc)]
    contract MerkleProofs {
        function verify(bytes32[] proof, bytes32 root, bytes32 leaf) external view returns (bool);
    }
}
