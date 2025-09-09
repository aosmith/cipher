class Friendship < ApplicationRecord
  belongs_to :requester, class_name: 'User'
  belongs_to :addressee, class_name: 'User'

  validates :status, inclusion: { in: ['pending', 'accepted', 'declined', 'blocked'] }
  validates :requester_id, uniqueness: { scope: :addressee_id }
  validate :not_self_friendship
  
  scope :accepted, -> { where(status: 'accepted') }
  scope :pending, -> { where(status: 'pending') }
  scope :involving_user, ->(user) { where('requester_id = ? OR addressee_id = ?', user.id, user.id) }

  def accept!
    update!(status: 'accepted')
  end

  def decline!
    update!(status: 'declined')
  end

  def block!
    update!(status: 'blocked')
  end

  def friend_of(user)
    requester == user ? addressee : requester
  end

  def status_for(user)
    case status
    when 'accepted'
      'friends'
    when 'pending'
      requester == user ? 'sent' : 'received'
    else
      status
    end
  end

  private

  def not_self_friendship
    errors.add(:addressee, "can't be the same as requester") if requester_id == addressee_id
  end
end