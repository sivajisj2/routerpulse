// SPDX-License-Identifier: MIT
pragma solidity 0.8.30;

/// @notice Executes messages signed by one configured, permissioned relayer.
/// @dev This is a demo trust model, not a trustless bridge or audited protocol.
contract DestinationRouter {
    error Unauthorized();
    error InvalidSignature();
    error InvalidDomain();
    error Expired();
    error AlreadyProcessed();
    error Paused();
    error ExecutionFailed(bytes reason);

    struct Message {
        bytes32 messageId;
        uint256 sourceChainId;
        address sourceRouter;
        address sender;
        address receiver;
        uint256 nonce;
        uint256 value;
        bytes32 payloadHash;
        uint256 deadline;
        bytes payload;
    }

    bytes32 private constant MESSAGE_TYPEHASH = keccak256(
        "Message(bytes32 messageId,uint256 sourceChainId,address sourceRouter,address sender,address receiver,uint256 nonce,uint256 value,bytes32 payloadHash,uint256 deadline,uint256 destinationChainId,address destinationRouter)"
    );
    bytes32 private constant DOMAIN_TYPEHASH = keccak256(
        "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
    );
    bytes32 private constant NAME_HASH = keccak256("RouterPulse");
    bytes32 private constant VERSION_HASH = keccak256("1");

    address public owner;
    address public relayer;
    bool public paused;
    mapping(uint256 => mapping(address => bool)) public trustedSources;
    mapping(bytes32 => bool) public processed;

    event MessageExecuted(bytes32 indexed messageId, address indexed receiver, bool success);
    event RelayerUpdated(address indexed relayer);
    event TrustedSourceUpdated(uint256 indexed chainId, address indexed router, bool trusted);
    event PauseUpdated(bool paused);

    constructor(address initialOwner, address initialRelayer) {
        if (initialOwner == address(0) || initialRelayer == address(0)) revert Unauthorized();
        owner = initialOwner;
        relayer = initialRelayer;
    }

    modifier onlyOwner() {
        if (msg.sender != owner) revert Unauthorized();
        _;
    }

    function setRelayer(address next) external onlyOwner {
        if (next == address(0)) revert Unauthorized();
        relayer = next;
        emit RelayerUpdated(next);
    }

    function setTrustedSource(uint256 chainId, address router, bool trusted) external onlyOwner {
        trustedSources[chainId][router] = trusted;
        emit TrustedSourceUpdated(chainId, router, trusted);
    }

    function setPaused(bool value) external onlyOwner {
        paused = value;
        emit PauseUpdated(value);
    }

    function messageDigest(Message calldata m) public view returns (bytes32) {
        bytes32 domain = keccak256(abi.encode(DOMAIN_TYPEHASH, NAME_HASH, VERSION_HASH,
            block.chainid, address(this)));
        bytes32 body = keccak256(abi.encode(MESSAGE_TYPEHASH, m.messageId, m.sourceChainId,
            m.sourceRouter, m.sender, m.receiver, m.nonce, m.value, m.payloadHash,
            m.deadline, block.chainid, address(this)));
        return keccak256(abi.encodePacked("\x19\x01", domain, body));
    }

    function executeMessage(Message calldata m, bytes calldata signature) external {
        if (paused) revert Paused();
        if (m.sourceChainId == block.chainid || !trustedSources[m.sourceChainId][m.sourceRouter]) {
            revert InvalidDomain();
        }
        if (block.timestamp > m.deadline) revert Expired();
        if (keccak256(m.payload) != m.payloadHash) revert InvalidDomain();
        if (processed[m.messageId]) revert AlreadyProcessed();
        bytes32 digest = messageDigest(m);
        if (signature.length != 65) revert InvalidSignature();
        bytes32 r; bytes32 s; uint8 v;
        assembly {
            r := calldataload(signature.offset)
            s := calldataload(add(signature.offset, 32))
            v := byte(0, calldataload(add(signature.offset, 64)))
        }
        if (v < 27) v += 27;
        if (v != 27 && v != 28) revert InvalidSignature();
        if (ecrecover(digest, v, r, s) != relayer) revert InvalidSignature();

        // Effects before interaction: a callback cannot re-enter this message.
        processed[m.messageId] = true;
        (bool ok, bytes memory reason) = m.receiver.call{value: m.value}(m.payload);
        if (!ok) revert ExecutionFailed(reason);
        emit MessageExecuted(m.messageId, m.receiver, true);
    }

    receive() external payable {}
}

