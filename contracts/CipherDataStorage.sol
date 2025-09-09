// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/security/Pausable.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/**
 * @title CipherDataStorage
 * @dev Smart contract managing data uploads, downloads, and host rewards
 * Implements 1 CPH per KB pricing for both uploads and downloads
 */
contract CipherDataStorage is ReentrancyGuard, Ownable, Pausable {
    using SafeERC20 for IERC20;
    
    IERC20 public immutable cipherToken;
    
    // Protocol fee percentages (out of 100)
    uint256 public constant HOST_SHARE_PERCENT = 80;
    uint256 public constant PROTOCOL_SHARE_PERCENT = 20;
    
    // Cost per KB in CPH tokens (18 decimals)
    uint256 public constant COST_PER_KB = 1e18;
    
    // Minimum stake required to become a host
    uint256 public minimumStake = 1000e18; // 1000 CPH
    
    // Protocol treasury address
    address public protocolTreasury;
    
    // File record structure
    struct FileRecord {
        address owner;           // File owner
        uint256 sizeKB;         // File size in KB
        uint256 uploadCost;     // Cost paid for upload
        uint256 totalDownloads; // Number of downloads
        uint256 uploadTime;     // Timestamp of upload
        bool exists;            // Whether file exists
        address[] hosts;        // Addresses hosting this file
        mapping(address => bool) isHost; // Quick lookup for hosts
    }
    
    // Host information structure
    struct HostInfo {
        uint256 stakedAmount;     // CPH tokens staked
        uint256 storageCapacityKB; // Available storage in KB
        uint256 usedStorageKB;    // Used storage in KB
        uint256 totalEarnings;    // Total CPH earned
        uint256 reliabilityScore; // Score out of 100
        uint256 registrationTime; // When host registered
        bool isActive;           // Whether host is active
    }
    
    // User balance structure (for prepaid usage)
    struct UserBalance {
        uint256 depositedAmount; // CPH deposited for future use
        uint256 totalSpent;      // Total CPH spent on uploads/downloads
        uint256 totalUploaded;   // Total KB uploaded
        uint256 totalDownloaded; // Total KB downloaded
    }
    
    // Mappings
    mapping(bytes32 => FileRecord) public files;
    mapping(address => HostInfo) public hosts;
    mapping(address => UserBalance) public userBalances;
    mapping(address => bytes32[]) public userFiles; // Files owned by user
    
    // Host arrays for selection
    address[] public activeHosts;
    mapping(address => uint256) private hostIndex; // Index in activeHosts array
    
    // Events
    event FileUploaded(
        bytes32 indexed fileHash,
        address indexed owner,
        uint256 sizeKB,
        uint256 cost,
        address[] hosts
    );
    
    event FileDownloaded(
        bytes32 indexed fileHash,
        address indexed downloader,
        uint256 cost
    );
    
    event HostRegistered(
        address indexed host,
        uint256 stakedAmount,
        uint256 storageCapacityKB
    );
    
    event HostUnregistered(address indexed host, uint256 refundedStake);
    
    event UserDeposit(address indexed user, uint256 amount);
    event UserWithdrawal(address indexed user, uint256 amount);
    
    event HostReward(address indexed host, uint256 amount, string rewardType);
    event ProtocolFee(uint256 amount);
    
    constructor(
        address _cipherToken,
        address _protocolTreasury
    ) {
        require(_cipherToken != address(0), "Invalid token address");
        require(_protocolTreasury != address(0), "Invalid treasury address");
        
        cipherToken = IERC20(_cipherToken);
        protocolTreasury = _protocolTreasury;
    }
    
    /**
     * @dev Register as a storage host
     * @param storageCapacityKB Available storage capacity in KB
     */
    function registerHost(uint256 storageCapacityKB) external nonReentrant {
        require(storageCapacityKB > 0, "Storage capacity must be positive");
        require(!hosts[msg.sender].isActive, "Host already registered");
        require(
            cipherToken.balanceOf(msg.sender) >= minimumStake,
            "Insufficient balance for staking"
        );
        
        // Transfer stake to contract
        cipherToken.safeTransferFrom(msg.sender, address(this), minimumStake);
        
        hosts[msg.sender] = HostInfo({
            stakedAmount: minimumStake,
            storageCapacityKB: storageCapacityKB,
            usedStorageKB: 0,
            totalEarnings: 0,
            reliabilityScore: 100,
            registrationTime: block.timestamp,
            isActive: true
        });
        
        // Add to active hosts array
        activeHosts.push(msg.sender);
        hostIndex[msg.sender] = activeHosts.length - 1;
        
        emit HostRegistered(msg.sender, minimumStake, storageCapacityKB);
    }
    
    /**
     * @dev Unregister as a host and reclaim stake
     */
    function unregisterHost() external nonReentrant {
        require(hosts[msg.sender].isActive, "Host not registered");
        
        HostInfo storage host = hosts[msg.sender];
        uint256 stakeToRefund = host.stakedAmount;
        
        // Remove from active hosts array
        uint256 indexToRemove = hostIndex[msg.sender];
        uint256 lastIndex = activeHosts.length - 1;
        
        if (indexToRemove != lastIndex) {
            address lastHost = activeHosts[lastIndex];
            activeHosts[indexToRemove] = lastHost;
            hostIndex[lastHost] = indexToRemove;
        }
        
        activeHosts.pop();
        delete hostIndex[msg.sender];
        
        // Mark host as inactive
        host.isActive = false;
        
        // Refund stake
        cipherToken.safeTransfer(msg.sender, stakeToRefund);
        
        emit HostUnregistered(msg.sender, stakeToRefund);
    }
    
    /**
     * @dev Deposit CPH tokens for future uploads/downloads
     * @param amount Amount of CPH to deposit
     */
    function deposit(uint256 amount) external nonReentrant {
        require(amount > 0, "Amount must be positive");
        
        cipherToken.safeTransferFrom(msg.sender, address(this), amount);
        userBalances[msg.sender].depositedAmount += amount;
        
        emit UserDeposit(msg.sender, amount);
    }
    
    /**
     * @dev Withdraw unused CPH tokens
     * @param amount Amount to withdraw
     */
    function withdraw(uint256 amount) external nonReentrant {
        require(amount > 0, "Amount must be positive");
        require(
            userBalances[msg.sender].depositedAmount >= amount,
            "Insufficient deposited balance"
        );
        
        userBalances[msg.sender].depositedAmount -= amount;
        cipherToken.safeTransfer(msg.sender, amount);
        
        emit UserWithdrawal(msg.sender, amount);
    }
    
    /**
     * @dev Upload a file to the network
     * @param fileHash Unique hash of the file
     * @param sizeKB File size in KB
     */
    function uploadFile(
        bytes32 fileHash,
        uint256 sizeKB
    ) external nonReentrant whenNotPaused {
        require(sizeKB > 0, "File size must be positive");
        require(!files[fileHash].exists, "File already exists");
        require(activeHosts.length >= 3, "Insufficient hosts available");
        
        uint256 cost = sizeKB * COST_PER_KB;
        
        // Check user has enough balance (either deposited or direct)
        uint256 userDeposit = userBalances[msg.sender].depositedAmount;
        if (userDeposit >= cost) {
            // Use deposited balance
            userBalances[msg.sender].depositedAmount -= cost;
        } else {
            // Need to pay the difference directly
            uint256 directPayment = cost - userDeposit;
            if (userDeposit > 0) {
                userBalances[msg.sender].depositedAmount = 0;
            }
            cipherToken.safeTransferFrom(msg.sender, address(this), directPayment);
        }
        
        // Select hosts for this file (simplified: use first 3 available hosts)
        address[] memory selectedHosts = selectHosts(sizeKB);
        
        // Create file record
        FileRecord storage file = files[fileHash];
        file.owner = msg.sender;
        file.sizeKB = sizeKB;
        file.uploadCost = cost;
        file.totalDownloads = 0;
        file.uploadTime = block.timestamp;
        file.exists = true;
        file.hosts = selectedHosts;
        
        // Mark hosts as storing this file
        for (uint256 i = 0; i < selectedHosts.length; i++) {
            file.isHost[selectedHosts[i]] = true;
            hosts[selectedHosts[i]].usedStorageKB += sizeKB;
        }
        
        // Add to user's files
        userFiles[msg.sender].push(fileHash);
        
        // Update user stats
        userBalances[msg.sender].totalSpent += cost;
        userBalances[msg.sender].totalUploaded += sizeKB;
        
        // Distribute payments
        distributeUploadPayment(cost, selectedHosts);
        
        emit FileUploaded(fileHash, msg.sender, sizeKB, cost, selectedHosts);
    }
    
    /**
     * @dev Download a file from the network
     * @param fileHash Hash of the file to download
     */
    function downloadFile(bytes32 fileHash) external nonReentrant whenNotPaused {
        FileRecord storage file = files[fileHash];
        require(file.exists, "File does not exist");
        
        uint256 cost = file.sizeKB * COST_PER_KB;
        
        // Check user has enough balance
        uint256 userDeposit = userBalances[msg.sender].depositedAmount;
        if (userDeposit >= cost) {
            userBalances[msg.sender].depositedAmount -= cost;
        } else {
            uint256 directPayment = cost - userDeposit;
            if (userDeposit > 0) {
                userBalances[msg.sender].depositedAmount = 0;
            }
            cipherToken.safeTransferFrom(msg.sender, address(this), directPayment);
        }
        
        // Update file stats
        file.totalDownloads++;
        
        // Update user stats
        userBalances[msg.sender].totalSpent += cost;
        userBalances[msg.sender].totalDownloaded += file.sizeKB;
        
        // Distribute payments to bandwidth providers (hosts of this file)
        distributeDownloadPayment(cost, file.hosts);
        
        emit FileDownloaded(fileHash, msg.sender, cost);
    }
    
    /**
     * @dev Select hosts for file storage
     * @param sizeKB Size of file in KB
     * @return Array of selected host addresses
     */
    function selectHosts(uint256 sizeKB) internal view returns (address[] memory) {
        require(activeHosts.length >= 3, "Not enough active hosts");
        
        address[] memory selected = new address[](3);
        uint256 selectedCount = 0;
        
        // Simple selection: first 3 hosts with enough capacity
        for (uint256 i = 0; i < activeHosts.length && selectedCount < 3; i++) {
            address hostAddr = activeHosts[i];
            HostInfo storage host = hosts[hostAddr];
            
            if (host.usedStorageKB + sizeKB <= host.storageCapacityKB) {
                selected[selectedCount] = hostAddr;
                selectedCount++;
            }
        }
        
        require(selectedCount == 3, "Could not select enough hosts");
        return selected;
    }
    
    /**
     * @dev Distribute payment for file upload
     * @param totalCost Total cost paid by user
     * @param selectedHosts Array of hosts storing the file
     */
    function distributeUploadPayment(
        uint256 totalCost,
        address[] memory selectedHosts
    ) internal {
        uint256 protocolFee = (totalCost * PROTOCOL_SHARE_PERCENT) / 100;
        uint256 hostReward = (totalCost * HOST_SHARE_PERCENT) / 100;
        uint256 rewardPerHost = hostReward / selectedHosts.length;
        
        // Distribute to hosts
        for (uint256 i = 0; i < selectedHosts.length; i++) {
            hosts[selectedHosts[i]].totalEarnings += rewardPerHost;
            emit HostReward(selectedHosts[i], rewardPerHost, "StorageReward");
        }
        
        // Send protocol fee to treasury
        cipherToken.safeTransfer(protocolTreasury, protocolFee);
        emit ProtocolFee(protocolFee);
    }
    
    /**
     * @dev Distribute payment for file download
     * @param totalCost Total cost paid by user
     * @param hostAddresses Array of hosts providing bandwidth
     */
    function distributeDownloadPayment(
        uint256 totalCost,
        address[] memory hostAddresses
    ) internal {
        uint256 protocolFee = (totalCost * PROTOCOL_SHARE_PERCENT) / 100;
        uint256 hostReward = (totalCost * HOST_SHARE_PERCENT) / 100;
        uint256 rewardPerHost = hostReward / hostAddresses.length;
        
        // Distribute to hosts
        for (uint256 i = 0; i < hostAddresses.length; i++) {
            hosts[hostAddresses[i]].totalEarnings += rewardPerHost;
            emit HostReward(hostAddresses[i], rewardPerHost, "BandwidthReward");
        }
        
        // Send protocol fee to treasury
        cipherToken.safeTransfer(protocolTreasury, protocolFee);
        emit ProtocolFee(protocolFee);
    }
    
    /**
     * @dev Host claims their earned rewards
     */
    function claimRewards() external nonReentrant {
        HostInfo storage host = hosts[msg.sender];
        require(host.isActive, "Host not active");
        require(host.totalEarnings > 0, "No rewards to claim");
        
        uint256 rewards = host.totalEarnings;
        host.totalEarnings = 0;
        
        cipherToken.safeTransfer(msg.sender, rewards);
        emit HostReward(msg.sender, rewards, "RewardsClaimed");
    }
    
    /**
     * @dev Get user's files
     * @param user Address of the user
     * @return Array of file hashes owned by user
     */
    function getUserFiles(address user) external view returns (bytes32[] memory) {
        return userFiles[user];
    }
    
    /**
     * @dev Get active hosts count
     */
    function getActiveHostsCount() external view returns (uint256) {
        return activeHosts.length;
    }
    
    /**
     * @dev Get file info
     */
    function getFileInfo(bytes32 fileHash) external view returns (
        address owner,
        uint256 sizeKB,
        uint256 uploadCost,
        uint256 totalDownloads,
        uint256 uploadTime,
        address[] memory fileHosts
    ) {
        FileRecord storage file = files[fileHash];
        require(file.exists, "File does not exist");
        
        return (
            file.owner,
            file.sizeKB,
            file.uploadCost,
            file.totalDownloads,
            file.uploadTime,
            file.hosts
        );
    }
    
    // Admin functions
    function updateProtocolTreasury(address _newTreasury) external onlyOwner {
        require(_newTreasury != address(0), "Invalid treasury address");
        protocolTreasury = _newTreasury;
    }
    
    function updateMinimumStake(uint256 _newMinimumStake) external onlyOwner {
        minimumStake = _newMinimumStake;
    }
    
    function pause() external onlyOwner {
        _pause();
    }
    
    function unpause() external onlyOwner {
        _unpause();
    }
}