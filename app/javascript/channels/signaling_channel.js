import consumer from "channels/consumer"
import WebRTCManager from "../webrtc_manager"

class CipherSignaling {
  constructor(userId) {
    this.userId = userId;
    this.subscription = null;
    this.webrtcManager = null;
    this.connect();
  }

  connect() {
    this.subscription = consumer.subscriptions.create(
      { channel: "SignalingChannel", user_id: this.userId },
      {
        connected: () => {
          console.log('Connected to signaling server');
          this.webrtcManager = new WebRTCManager(this.userId, this);
          this.onConnected();
        },

        disconnected: () => {
          console.log('Disconnected from signaling server');
          this.onDisconnected();
        },

        received: (data) => {
          // This will be handled by WebRTCManager
          if (this.webrtcManager) {
            this.webrtcManager.received(data);
          }
        }
      }
    );
  }

  send(data) {
    if (this.subscription) {
      this.subscription.perform(data.action, data);
    }
  }

  disconnect() {
    if (this.subscription) {
      this.subscription.unsubscribe();
    }
  }

  // Override these in your application
  onConnected() {
    console.log('Cipher signaling connected');
  }

  onDisconnected() {
    console.log('Cipher signaling disconnected');
  }
}

// Export for global use
window.CipherSignaling = CipherSignaling;

export default CipherSignaling;
