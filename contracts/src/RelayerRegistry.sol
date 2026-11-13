// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

contract RelayerRegistry {
    error Unauthorized();
    address public owner;
    mapping(address => bool) public active;
    mapping(address => mapping(uint256 => bool)) public destinationAllowed;
    event RelayerUpdated(address indexed relayer, bool active);
    event DestinationPermissionUpdated(address indexed relayer, uint256 indexed chainId, bool allowed);

    constructor(address initialOwner) {
        if (initialOwner == address(0)) revert Unauthorized();
        owner = initialOwner;
    }

    modifier onlyOwner() { if (msg.sender != owner) revert Unauthorized(); _; }

    function setRelayer(address operator, bool enabled) external onlyOwner {
        active[operator] = enabled;
        emit RelayerUpdated(operator, enabled);
    }

    function setDestination(address operator, uint256 chainId, bool allowed) external onlyOwner {
        if (!active[operator]) revert Unauthorized();
        destinationAllowed[operator][chainId] = allowed;
        emit DestinationPermissionUpdated(operator, chainId, allowed);
    }
}

