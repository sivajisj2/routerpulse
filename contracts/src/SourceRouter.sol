// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

/// @notice Emits domain-separated messages for a permissioned relayer demo.
contract SourceRouter {
    error Unauthorized();
    error InvalidDestination();
    error InvalidMessage();
    error PayloadTooLarge();
    error Expired();
    error Paused();

    uint256 public constant MAX_PAYLOAD_BYTES = 4096;
    address public owner;
    bool public paused;
    mapping(address => bool) public allowedSenders;
    mapping(uint256 => bool) public allowedDestinations;
    mapping(address => uint256) public nonces;
    mapping(bytes32 => bool) public commitments;

    event MessageSent(
        bytes32 indexed messageId,
        uint256 indexed destinationChainId,
        address indexed sender,
        address receiver,
        uint256 nonce,
        uint256 value,
        bytes32 payloadHash,
        uint256 deadline,
        bytes payload
    );
    event SenderPermissionUpdated(address indexed sender, bool allowed);
    event DestinationUpdated(uint256 indexed chainId, bool allowed);
    event PauseUpdated(bool paused);

    constructor(address initialOwner) {
        if (initialOwner == address(0)) revert Unauthorized();
        owner = initialOwner;
        allowedSenders[initialOwner] = true;
    }

    modifier onlyOwner() {
        if (msg.sender != owner) revert Unauthorized();
        _;
    }

    function setSender(address sender, bool allowed) external onlyOwner {
        allowedSenders[sender] = allowed;
        emit SenderPermissionUpdated(sender, allowed);
    }

    function setDestination(uint256 chainId, bool allowed) external onlyOwner {
        if (chainId == block.chainid) revert InvalidDestination();
        allowedDestinations[chainId] = allowed;
        emit DestinationUpdated(chainId, allowed);
    }

    function setPaused(bool value) external onlyOwner {
        paused = value;
        emit PauseUpdated(value);
    }

    function sendMessage(
        uint256 destinationChainId,
        address receiver,
        uint256 value,
        bytes calldata payload,
        uint256 deadline
    ) external returns (bytes32 messageId) {
        if (paused) revert Paused();
        if (!allowedSenders[msg.sender]) revert Unauthorized();
        if (!allowedDestinations[destinationChainId] || destinationChainId == 0) revert InvalidDestination();
        if (receiver == address(0) || deadline <= block.timestamp) revert InvalidMessage();
        if (payload.length > MAX_PAYLOAD_BYTES) revert PayloadTooLarge();
        uint256 nonce = nonces[msg.sender]++;
        bytes32 payloadHash = keccak256(payload);
        messageId = keccak256(abi.encode(block.chainid, address(this), msg.sender, nonce,
            destinationChainId, receiver, value, payloadHash, deadline));
        commitments[messageId] = true;
        emit MessageSent(messageId, destinationChainId, msg.sender, receiver, nonce, value,
            payloadHash, deadline, payload);
    }
}

