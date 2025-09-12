import consumer from "./consumer"

// Only create subscription if we have a current user
const currentUser = document.querySelector('meta[name="current-user-id"]');
const currentUserId = currentUser ? currentUser.content : null;

if (currentUserId) {
  consumer.subscriptions.create({ channel: "MessagesChannel", user_id: currentUserId }, {
    connected() {
      console.log("Connected to MessagesChannel");
    },

    disconnected() {
      console.log("Disconnected from MessagesChannel");
    },

    received(data) {
      if (data.action === "append" && data.target && data.html) {
        const target = document.getElementById(data.target);
        if (target) {
          target.insertAdjacentHTML('beforeend', data.html);
          // Auto-scroll to bottom after new message
          target.scrollTop = target.scrollHeight;
        }
      }
    }
  });
}
