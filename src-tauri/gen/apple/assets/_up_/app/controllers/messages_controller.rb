class MessagesController < ApplicationController
  before_action :require_current_user_session
  before_action :set_current_user_for_decryption
  before_action :set_message, only: [:destroy]
  before_action :set_recipient, only: [:show, :create]
  
  # GET /messages - List all conversations
  def index
    @conversations = current_user_session.conversations
    @unread_count = current_user_session.unread_messages_count
  end
  
  # GET /messages/:id - Show conversation with a specific user
  def show
    return redirect_to messages_path, alert: "User not found" unless @recipient
    
    @messages = current_user_session.messages_with(@recipient).includes(:sender, :recipient)
    @new_message = Message.new
    
    # Mark messages from this user as read
    current_user_session.received_messages.where(sender: @recipient, read_at: nil).update_all(read_at: Time.current)
  end
  
  # POST /messages - Create a new message
  def create
    @message = current_user_session.sent_messages.build(message_params)
    @message.recipient = @recipient
    
    respond_to do |format|
      if @message.save
        # Broadcast to recipient via Turbo Stream (for real-time updates)
        broadcast_message_to_recipient(@message)
        
        format.turbo_stream do
          render turbo_stream: [
            turbo_stream.append("messages-list", partial: "messages/message", locals: { message: @message, current_user: current_user_session }),
            turbo_stream.update("new-message-form", partial: "messages/message_form", locals: { message: Message.new, recipient: @recipient })
          ]
        end
        format.html { redirect_to user_message_path(@recipient) }
      else
        format.turbo_stream do
          render turbo_stream: turbo_stream.update("new-message-form", partial: "messages/message_form", locals: { message: @message, recipient: @recipient })
        end
        format.html { redirect_to user_message_path(@recipient), alert: "Failed to send message" }
      end
    end
  end
  
  # DELETE /messages/:id - Delete a message
  def destroy
    if @message.sender == current_user_session
      @message.destroy
      respond_to do |format|
        format.turbo_stream { render turbo_stream: turbo_stream.remove(@message) }
        format.html { redirect_back(fallback_location: messages_path) }
      end
    else
      redirect_back(fallback_location: messages_path, alert: "You can only delete your own messages")
    end
  end
  
  private
  
  def set_message
    @message = Message.find(params[:id])
  end
  
  def set_recipient
    @recipient = User.find_by(id: params[:user_id] || params[:recipient_id])
  end
  
  def message_params
    params.require(:message).permit(:content)
  end
  
  def broadcast_message_to_recipient(message)
    # Broadcast to recipient's personal channel
    MessagesChannel.broadcast_to(message.recipient, {
      action: "append",
      target: "messages-list",
      html: render_to_string(partial: "messages/message", locals: { message: message, current_user: message.recipient })
    })
    
    # Also broadcast to sender for their own message confirmation
    MessagesChannel.broadcast_to(message.sender, {
      action: "append", 
      target: "messages-list",
      html: render_to_string(partial: "messages/message", locals: { message: message, current_user: message.sender })
    })
  end
  
  def require_current_user_session
    redirect_to root_path, alert: "Please sign in first" unless current_user_session
  end
  
  def set_current_user_for_decryption
    # Set current user in thread-local storage for message decryption
    Thread.current[:current_user_for_decryption] = current_user_session
  end
end
