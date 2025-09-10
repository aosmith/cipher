class Api::V1::UsersController < ApplicationController
  protect_from_forgery with: :null_session

  def by_public_key
    public_key = params[:public_key]
    
    if public_key.blank?
      return render json: { error: 'public_key required' }, status: 400
    end
    
    user = User.find_by(public_key: public_key)
    
    if user
      render json: {
        id: user.id,
        username: user.username,
        display_name: user.display_name,
        public_key: user.public_key,
        created_at: user.created_at
      }
    else
      render json: { error: 'User not found' }, status: 404
    end
  end
end