class Api::V1::BlockchainController < ApplicationController
  protect_from_forgery with: :null_session

  # Get contract configuration and addresses
  def config
    render json: {
      network: Rails.env.production? ? "polygon-mainnet" : "polygon-mumbai",
      contracts: {
        token: {
          address: ENV["CIPHER_TOKEN_ADDRESS"] || "0x742d35Cc6935C5532a2EaD8d7C2A6d8EfB4e8Fc6",
          name: "CipherToken",
          symbol: "CPH"
        },
        storage: {
          address: ENV["CIPHER_STORAGE_ADDRESS"] || "0x8B7F2A3E9D4C5B6A7E8F9D0C1B2A3E4D5C6B7A8E",
          name: "CipherDataStorage"
        }
      },
      settings: {
        cost_per_kb: 1,
        minimum_stake: 1000,
        host_share_percent: 80,
        protocol_share_percent: 20
      }
    }
  end

  # Calculate cost for file upload/download
  def calculate_cost
    file_size_bytes = params[:file_size_bytes]&.to_i

    if file_size_bytes.nil? || file_size_bytes <= 0
      return render json: { error: "Valid file_size_bytes required" }, status: 400
    end

    # Calculate cost: 1 CPH per KB, rounded up
    size_kb = (file_size_bytes + 1023) / 1024
    cost_cph = size_kb

    render json: {
      file_size_bytes: file_size_bytes,
      file_size_kb: size_kb,
      cost_cph: cost_cph,
      cost_wei: (cost_cph * 10**18).to_s, # Convert to wei (18 decimals)
      formatted_cost: format_cost(cost_cph)
    }
  end

  # Get network statistics
  def network_stats
    # In a real implementation, these would come from blockchain queries
    # For now, return mock data that matches the expected structure
    render json: {
      active_hosts: 12,
      total_files_stored: 1458,
      total_storage_kb: 2847629,
      total_bandwidth_served: 8736291,
      network_utilization: 67.3,
      average_reliability_score: 94.2,
      token_info: {
        name: "Cipher Token",
        symbol: "CPH",
        total_supply: "1000000000000000000000000000", # 1B tokens in wei
        circulating_supply: "250000000000000000000000000" # 250M tokens in wei
      }
    }
  end

  # Get file information from blockchain
  def file_info
    file_hash = params[:file_hash]

    if file_hash.blank?
      return render json: { error: "file_hash required" }, status: 400
    end

    # In a real implementation, this would query the blockchain
    # For now, return mock data structure
    render json: {
      file_hash: file_hash,
      owner: "0x742d35Cc6935C5532a2EaD8d7C2A6d8EfB4e8Fc6",
      size_kb: 245,
      upload_cost: "245000000000000000000", # 245 CPH in wei
      total_downloads: 12,
      upload_time: 2.days.ago.to_i,
      hosts: [
        "0x1234567890123456789012345678901234567890",
        "0x2345678901234567890123456789012345678901",
        "0x3456789012345678901234567890123456789012"
      ],
      exists: true
    }
  end

  # Record successful blockchain transaction
  def record_transaction
    transaction_params = params.require(:transaction).permit(
      :hash, :type, :file_hash, :amount, :wallet_address, :block_number, :gas_used
    )

    # Log the transaction for our records
    Rails.logger.info "Blockchain transaction recorded: #{transaction_params.to_h}"

    # In a full implementation, you might store these in a database table
    # for analytics, backup, or audit purposes

    render json: {
      status: "recorded",
      transaction_hash: transaction_params[:hash],
      timestamp: Time.current.to_i
    }
  end

  # Get user's blockchain activity summary
  def user_activity
    wallet_address = params[:wallet_address]

    if wallet_address.blank?
      return render json: { error: "wallet_address required" }, status: 400
    end

    # In a real implementation, this would query blockchain events
    # For now, return mock activity data
    render json: {
      wallet_address: wallet_address,
      summary: {
        total_files_uploaded: 23,
        total_files_downloaded: 67,
        total_spent_cph: "1245.67",
        total_earned_cph: "892.34", # If user is also a host
        storage_used_kb: 15023,
        bandwidth_served_kb: 45892
      },
      recent_activity: [
        {
          type: "upload",
          file_hash: "QmTyKrGBK4VtyX7pXQ8JwRq3bNVYqB9pZ8CgFm7YnA3xPv",
          cost_cph: "12.5",
          timestamp: 2.hours.ago.to_i,
          transaction_hash: "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
        },
        {
          type: "download",
          file_hash: "QmP8d4J6QbXzRv5tYhJmNkL2WqE9DgFs3CrTx7oVuH4GnM",
          cost_cph: "8.2",
          timestamp: 6.hours.ago.to_i,
          transaction_hash: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        }
      ]
    }
  end

  private

  def format_cost(cost_cph)
    if cost_cph >= 1_000_000
      "#{(cost_cph / 1_000_000.0).round(2)}M CPH"
    elsif cost_cph >= 1_000
      "#{(cost_cph / 1_000.0).round(2)}K CPH"
    else
      "#{cost_cph} CPH"
    end
  end
end
