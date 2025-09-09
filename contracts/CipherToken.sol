// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/security/Pausable.sol";

/**
 * @title CipherToken (CPH)
 * @dev ERC-20 token for the Cipher decentralized social network
 * Used for paying data storage and bandwidth costs (1 CPH = 1 KB)
 */
contract CipherToken is ERC20, ERC20Burnable, Ownable, Pausable {
    // Total supply: 1 billion tokens
    uint256 private constant TOTAL_SUPPLY = 1_000_000_000 * 10**18;
    
    // Distribution addresses
    address public networkRewardsPool;
    address public developmentFund;
    address public teamWallet;
    
    // Token allocations
    uint256 private constant NETWORK_REWARDS = 400_000_000 * 10**18; // 40%
    uint256 private constant PUBLIC_DISTRIBUTION = 250_000_000 * 10**18; // 25%
    uint256 private constant DEVELOPMENT_FUND = 200_000_000 * 10**18; // 20%
    uint256 private constant TEAM_ALLOCATION = 100_000_000 * 10**18; // 10%
    uint256 private constant INITIAL_LIQUIDITY = 50_000_000 * 10**18; // 5%
    
    // Events
    event TokensDistributed(address indexed recipient, uint256 amount, string category);
    event NetworkConfigUpdated(address indexed networkRewards, address indexed devFund, address indexed team);
    
    constructor(
        address _networkRewardsPool,
        address _developmentFund,
        address _teamWallet
    ) ERC20("Cipher Token", "CPH") {
        require(_networkRewardsPool != address(0), "Invalid network rewards address");
        require(_developmentFund != address(0), "Invalid development fund address");
        require(_teamWallet != address(0), "Invalid team wallet address");
        
        networkRewardsPool = _networkRewardsPool;
        developmentFund = _developmentFund;
        teamWallet = _teamWallet;
        
        // Initial token distribution
        _distributeTokens();
    }
    
    function _distributeTokens() private {
        // Network rewards pool (40%)
        _mint(networkRewardsPool, NETWORK_REWARDS);
        emit TokensDistributed(networkRewardsPool, NETWORK_REWARDS, "NetworkRewards");
        
        // Public distribution to owner for airdrops/sales (25%)
        _mint(owner(), PUBLIC_DISTRIBUTION);
        emit TokensDistributed(owner(), PUBLIC_DISTRIBUTION, "PublicDistribution");
        
        // Development fund (20%)
        _mint(developmentFund, DEVELOPMENT_FUND);
        emit TokensDistributed(developmentFund, DEVELOPMENT_FUND, "Development");
        
        // Team allocation (10%)
        _mint(teamWallet, TEAM_ALLOCATION);
        emit TokensDistributed(teamWallet, TEAM_ALLOCATION, "Team");
        
        // Initial liquidity to owner (5%)
        _mint(owner(), INITIAL_LIQUIDITY);
        emit TokensDistributed(owner(), INITIAL_LIQUIDITY, "InitialLiquidity");
    }
    
    /**
     * @dev Update network configuration addresses
     * @param _networkRewardsPool New network rewards pool address
     * @param _developmentFund New development fund address
     * @param _teamWallet New team wallet address
     */
    function updateNetworkConfig(
        address _networkRewardsPool,
        address _developmentFund,
        address _teamWallet
    ) external onlyOwner {
        require(_networkRewardsPool != address(0), "Invalid network rewards address");
        require(_developmentFund != address(0), "Invalid development fund address");
        require(_teamWallet != address(0), "Invalid team wallet address");
        
        networkRewardsPool = _networkRewardsPool;
        developmentFund = _developmentFund;
        teamWallet = _teamWallet;
        
        emit NetworkConfigUpdated(_networkRewardsPool, _developmentFund, _teamWallet);
    }
    
    /**
     * @dev Pause token transfers (emergency only)
     */
    function pause() external onlyOwner {
        _pause();
    }
    
    /**
     * @dev Unpause token transfers
     */
    function unpause() external onlyOwner {
        _unpause();
    }
    
    /**
     * @dev Batch transfer tokens to multiple addresses
     * @param recipients Array of recipient addresses
     * @param amounts Array of amounts to transfer
     */
    function batchTransfer(
        address[] calldata recipients,
        uint256[] calldata amounts
    ) external {
        require(recipients.length == amounts.length, "Arrays length mismatch");
        require(recipients.length <= 100, "Too many recipients");
        
        for (uint256 i = 0; i < recipients.length; i++) {
            transfer(recipients[i], amounts[i]);
        }
    }
    
    /**
     * @dev Get token info for frontend integration
     */
    function getTokenInfo() external view returns (
        string memory name,
        string memory symbol,
        uint8 decimals,
        uint256 totalSupply,
        address networkRewards,
        address devFund,
        address team
    ) {
        return (
            name(),
            symbol(),
            decimals(),
            totalSupply(),
            networkRewardsPool,
            developmentFund,
            teamWallet
        );
    }
    
    /**
     * @dev Calculate cost for file size in bytes
     * @param fileSizeBytes File size in bytes
     * @return Cost in CPH tokens (1 CPH per KB)
     */
    function calculateCost(uint256 fileSizeBytes) external pure returns (uint256) {
        // Round up to nearest KB (1024 bytes)
        uint256 sizeInKB = (fileSizeBytes + 1023) / 1024;
        return sizeInKB * 10**18; // Return in wei (18 decimals)
    }
    
    // Override required by Solidity
    function _beforeTokenTransfer(
        address from,
        address to,
        uint256 amount
    ) internal override whenNotPaused {
        super._beforeTokenTransfer(from, to, amount);
    }
}