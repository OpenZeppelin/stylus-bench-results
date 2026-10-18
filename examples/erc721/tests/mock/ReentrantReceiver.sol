// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.24;

interface IERC721Mint {
    function safeMint(address to, uint256 tokenId, bytes calldata data) external;
}

interface IERC721Receiver {
    function onERC721Received(
        address operator,
        address from,
        uint256 tokenId,
        bytes calldata data
    ) external returns (bytes4);
}

contract ReentrantReceiver is IERC721Receiver {
    IERC721Mint private immutable _token;
    uint256 private immutable _reentrantTokenId;
    bool public nestedCallSucceeded;
    bytes public nestedCallError;
    bool private _entered;

    constructor(address token, uint256 reentrantTokenId) {
        _token = IERC721Mint(token);
        _reentrantTokenId = reentrantTokenId;
    }

    function startMint(uint256 tokenId) external {
        _token.safeMint(address(this), tokenId, "");
    }

    function onERC721Received(
        address,
        address,
        uint256,
        bytes calldata
    ) external returns (bytes4) {
        require(msg.sender == address(_token), "unexpected caller");
        if (!_entered) {
            _entered = true;
            try _token.safeMint(address(this), _reentrantTokenId, "") {
                nestedCallSucceeded = true;
            } catch (bytes memory reason) {
                nestedCallError = reason;
            }
        }
        return IERC721Receiver.onERC721Received.selector;
    }
}
