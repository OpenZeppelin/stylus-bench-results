// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

interface IERC721Minter {
    function mint(address to, uint256 tokenId) external;
}

contract ReentrantReceiver {
    IERC721Minter private immutable token;
    uint256 private immutable reentrantTokenId;
    bool private entered;
    bool public nestedCallSucceeded;

    constructor(address token_, uint256 reentrantTokenId_) {
        token = IERC721Minter(token_);
        reentrantTokenId = reentrantTokenId_;
    }

    function onERC721Received(
        address,
        address,
        uint256,
        bytes calldata
    ) external returns (bytes4) {
        if (!entered) {
            entered = true;
            try token.mint(address(this), reentrantTokenId) {
                nestedCallSucceeded = true;
            } catch {
                nestedCallSucceeded = false;
            }
        }
        return 0x150b7a02;
    }
}
