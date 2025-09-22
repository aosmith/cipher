class Friendship < ApplicationRecord
  belongs_to :requester, class_name: "User"
  belongs_to :addressee, class_name: "User"

  validates :status, inclusion: { in: [ "pending", "accepted", "declined", "blocked" ] }
  validates :requester_id, uniqueness: { scope: :addressee_id }
  validate :not_self_friendship

  after_commit :schedule_mutual_sync_if_accepted, on: [ :create, :update ]

  scope :accepted, -> { where(status: "accepted") }
  scope :pending, -> { where(status: "pending") }
  scope :involving_user, ->(user) { where("requester_id = ? OR addressee_id = ?", user.id, user.id) }

  def accept!
    update!(status: "accepted")
  end

  def decline!
    update!(status: "declined")
  end

  def block!
    update!(status: "blocked")
  end

  def friend_of(user)
    requester == user ? addressee : requester
  end

  def status_for(user)
    case status
    when "accepted"
      "friends"
    when "pending"
      requester == user ? "sent" : "received"
    else
      status
    end
  end

  private

  def not_self_friendship
    errors.add(:addressee, "can't be the same as requester") if requester_id == addressee_id
  end

  def schedule_mutual_sync_if_accepted
    status_change = previous_changes["status"]
    return unless status_change && status_change.last == "accepted"

    MutualFriendSyncService.new(self).schedule_initial_sync
  end
end
