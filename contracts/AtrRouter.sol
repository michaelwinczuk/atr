// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title IGasback — Shape Network Gasback registration interface
interface IGasback {
    function register(address _nftRecipient, address _smartContract) external returns (uint256 tokenId);
}

/// @title AtrRouter — Revenue-collecting transaction router for ATR on Shape
/// @notice Logs intents, collects protocol fees, and registers for Shape Gasback
contract AtrRouter {
    address public owner;
    address public constant GASBACK = 0xdF329d59bC797907703F7c198dDA2d770fC45034;

    uint256 public protocolFeeBps = 50; // 0.5% default (50 basis points)
    uint256 public totalFeesCollected;
    uint256 public intentCount;

    event IntentRouted(bytes32 indexed intentId, address indexed sender, uint256 value, uint256 fee);
    event FeeUpdated(uint256 oldBps, uint256 newBps);
    event Withdrawn(address indexed to, uint256 amount);
    event GasbackRegistered(uint256 tokenId);

    modifier onlyOwner() {
        require(msg.sender == owner, "not owner");
        _;
    }

    constructor() {
        owner = msg.sender;
    }

    /// @notice Register this contract for Shape Gasback (80% of sequencer fees returned)
    function registerForGasback() external onlyOwner returns (uint256) {
        uint256 tokenId = IGasback(GASBACK).register(owner, address(this));
        emit GasbackRegistered(tokenId);
        return tokenId;
    }

    /// @notice Route an intent through ATR — logs on-chain and collects protocol fee
    /// @param intentId The unique intent identifier
    function routeIntent(bytes32 intentId) external payable {
        uint256 fee = (msg.value * protocolFeeBps) / 10000;
        totalFeesCollected += fee;
        intentCount++;
        emit IntentRouted(intentId, msg.sender, msg.value, fee);

        // Forward remaining value to sender (self-transfer pattern for logging)
        if (msg.value > fee) {
            (bool ok, ) = msg.sender.call{value: msg.value - fee}("");
            require(ok, "forward failed");
        }
    }

    /// @notice Update the protocol fee (max 2%)
    function setProtocolFee(uint256 newBps) external onlyOwner {
        require(newBps <= 200, "max 2%");
        emit FeeUpdated(protocolFeeBps, newBps);
        protocolFeeBps = newBps;
    }

    /// @notice Withdraw collected fees to owner
    function withdraw() external onlyOwner {
        uint256 balance = address(this).balance;
        require(balance > 0, "no balance");
        (bool ok, ) = owner.call{value: balance}("");
        require(ok, "withdraw failed");
        emit Withdrawn(owner, balance);
    }

    /// @notice Transfer ownership
    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "zero address");
        owner = newOwner;
    }

    receive() external payable {}
}
