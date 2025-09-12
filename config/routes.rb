Rails.application.routes.draw do
  # Messages routes
  resources :messages, only: [:index, :create, :destroy, :show] do
    collection do
      get 'with/:user_id', to: 'messages#show', as: :conversation_with
    end
  end
  
  # Shortcut route for messaging a specific user
  get 'messages/:user_id', to: 'messages#show', as: :user_message
  
  resources :users, only: [:index, :show, :create, :new] do
    collection do
      get :import_keys
      post :import_keys
      get :export_keys  
      post :export_keys
      get :host_dashboard
      get :local_hosting
      get :friends
      get :dashboard
    end
  end
  resources :posts do
    resources :attachments, only: [:show, :create]
    resources :comments, only: [:create, :destroy]
  end
  
  get 'feed', to: 'feed#index'
  
  # WebSocket mount for Action Cable
  mount ActionCable.server => '/cable'
  
  # API endpoints for peer communication
  namespace :api do
    namespace :v1 do
      resources :peers, only: [:index, :create, :update, :destroy]
      post 'sync', to: 'sync#sync_messages'
      post 'verify_identity', to: 'auth#verify_identity'
      post 'login', to: 'auth#login'
      
      # Friend-based sync endpoints
      get 'users/:user_id/friends', to: 'sync#friends'
      get 'users/:user_id/posts/for_sync', to: 'sync#posts_for_sync' 
      get 'posts/my_posts_for_friends', to: 'sync#my_posts_for_friends'
      get 'posts/:id/sync_data', to: 'sync#post_sync_data'
      post 'posts/sync_store', to: 'sync#sync_store'
      get 'content/:content_hash/exists', to: 'sync#content_exists'
      get 'sync/stats', to: 'sync#sync_stats'
      
      # Friends management
      resources :friends, only: [:index, :show, :create, :update, :destroy] do
        collection do
          post :send_request
          post :respond_to_request
          get :search_by_public_key
        end
      end
      
      post 'users/by_public_key', to: 'users#by_public_key'
      get 'users/current_with_private_key', to: 'users#current_user_with_private_key'
      
      # Blockchain endpoints
      get 'blockchain/config', to: 'blockchain#config'
      post 'blockchain/calculate_cost', to: 'blockchain#calculate_cost'
      get 'blockchain/network_stats', to: 'blockchain#network_stats'
      get 'blockchain/file_info/:file_hash', to: 'blockchain#file_info'
      post 'blockchain/record_transaction', to: 'blockchain#record_transaction'
      get 'blockchain/user_activity/:wallet_address', to: 'blockchain#user_activity'
      
      # File upload with blockchain integration
      resources :attachments, only: [:show, :create] do
        member do
          post :upload_with_payment
          post :download_with_payment
        end
      end
    end
  end

  # Reveal health status on /up that returns 200 if the app boots with no exceptions, otherwise 500.
  # Can be used by load balancers and uptime monitors to verify that the app is live.
  get "up" => "rails/health#show", as: :rails_health_check

  # Defines the root path route ("/")
  root "users#index"
end
