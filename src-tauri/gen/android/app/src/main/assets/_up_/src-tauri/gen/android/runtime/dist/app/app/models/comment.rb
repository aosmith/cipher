class Comment < ApplicationRecord
  belongs_to :user
  belongs_to :post

  scope :recent, -> { order(timestamp: :desc) }

  validates :content, presence: true
  validates :timestamp, presence: true

  before_validation :set_timestamp, on: :create

  private

  def set_timestamp
    self.timestamp = Time.current unless timestamp.present?
  end
end
