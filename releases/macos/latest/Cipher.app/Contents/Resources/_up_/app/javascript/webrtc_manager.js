class WebRTCManager {
  constructor(userId, signalingChannel) {
    this.userId = userId;
    this.signalingChannel = signalingChannel;
    this.peerConnections = new Map();
    this.dataChannels = new Map();
    this.config = {
      iceServers: [
        { urls: 'stun:stun.l.google.com:19302' },
        { urls: 'stun:stun1.l.google.com:19302' },
        { urls: 'stun:stun.cloudflare.com:3478' }
        // Add TURN servers here when available
      ],
      iceCandidatePoolSize: 10,
      bundlePolicy: 'max-bundle',
      rtcpMuxPolicy: 'require'
    };
    
    this.setupSignalingHandlers();
  }

  setupSignalingHandlers() {
    this.signalingChannel.received = (data) => {
      switch (data.type) {
        case 'offer':
          this.handleOffer(data);
          break;
        case 'answer':
          this.handleAnswer(data);
          break;
        case 'ice_candidate':
          this.handleIceCandidate(data);
          break;
        case 'peer_list':
          this.handlePeerList(data);
          break;
        default:
          console.log('Unknown signaling message:', data);
      }
    };
  }

  async connectToPeer(peerId) {
    if (this.peerConnections.has(peerId)) {
      console.log('Already connected to peer:', peerId);
      return;
    }

    const peerConnection = new RTCPeerConnection(this.config);
    this.peerConnections.set(peerId, peerConnection);

    // Create data channel for encrypted messages
    const dataChannel = peerConnection.createDataChannel('messages', {
      ordered: true
    });
    
    this.setupDataChannel(dataChannel, peerId);
    
    // Handle ICE candidates
    peerConnection.onicecandidate = (event) => {
      if (event.candidate) {
        this.signalingChannel.send({
          action: 'send_ice_candidate',
          recipient_id: peerId,
          candidate: event.candidate
        });
      }
    };

    // Handle connection state changes
    peerConnection.onconnectionstatechange = () => {
      console.log(`Connection with ${peerId}:`, peerConnection.connectionState);
      
      if (peerConnection.connectionState === 'connected') {
        this.onPeerConnected(peerId);
      } else if (peerConnection.connectionState === 'disconnected') {
        this.onPeerDisconnected(peerId);
      }
    };

    // Create and send offer
    const offer = await peerConnection.createOffer();
    await peerConnection.setLocalDescription(offer);
    
    this.signalingChannel.send({
      action: 'send_offer',
      recipient_id: peerId,
      offer: offer
    });
  }

  async handleOffer(data) {
    const { sender_id, offer } = data;
    
    if (this.peerConnections.has(sender_id)) {
      console.log('Peer connection already exists:', sender_id);
      return;
    }

    const peerConnection = new RTCPeerConnection(this.config);
    this.peerConnections.set(sender_id, peerConnection);

    // Handle incoming data channel
    peerConnection.ondatachannel = (event) => {
      const dataChannel = event.channel;
      this.setupDataChannel(dataChannel, sender_id);
    };

    // Handle ICE candidates
    peerConnection.onicecandidate = (event) => {
      if (event.candidate) {
        this.signalingChannel.send({
          action: 'send_ice_candidate',
          recipient_id: sender_id,
          candidate: event.candidate
        });
      }
    };

    // Set remote description and create answer
    await peerConnection.setRemoteDescription(offer);
    const answer = await peerConnection.createAnswer();
    await peerConnection.setLocalDescription(answer);
    
    this.signalingChannel.send({
      action: 'send_answer',
      sender_id: sender_id,
      answer: answer
    });
  }

  async handleAnswer(data) {
    const { sender_id, answer } = data;
    const peerConnection = this.peerConnections.get(sender_id);
    
    if (peerConnection) {
      await peerConnection.setRemoteDescription(answer);
    }
  }

  async handleIceCandidate(data) {
    const { sender_id, candidate } = data;
    const peerConnection = this.peerConnections.get(sender_id);
    
    if (peerConnection) {
      await peerConnection.addIceCandidate(candidate);
    }
  }

  handlePeerList(data) {
    console.log('Available peers:', data.peers);
    this.onPeerListReceived(data.peers);
  }

  setupDataChannel(dataChannel, peerId) {
    this.dataChannels.set(peerId, dataChannel);
    
    dataChannel.onopen = () => {
      console.log(`Data channel with ${peerId} opened`);
      this.onDataChannelOpen(peerId);
    };
    
    dataChannel.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);
        this.onEncryptedMessage(peerId, message);
      } catch (error) {
        console.error('Failed to parse message:', error);
      }
    };
    
    dataChannel.onclose = () => {
      console.log(`Data channel with ${peerId} closed`);
      this.dataChannels.delete(peerId);
    };
  }

  sendEncryptedMessage(peerId, encryptedMessage) {
    const dataChannel = this.dataChannels.get(peerId);
    if (dataChannel && dataChannel.readyState === 'open') {
      dataChannel.send(JSON.stringify(encryptedMessage));
      return true;
    }
    return false;
  }

  broadcastMessage(encryptedMessage) {
    let successCount = 0;
    for (const [peerId, dataChannel] of this.dataChannels) {
      if (dataChannel.readyState === 'open') {
        dataChannel.send(JSON.stringify(encryptedMessage));
        successCount++;
      }
    }
    return successCount;
  }

  disconnectFromPeer(peerId) {
    const peerConnection = this.peerConnections.get(peerId);
    if (peerConnection) {
      peerConnection.close();
      this.peerConnections.delete(peerId);
    }
    
    const dataChannel = this.dataChannels.get(peerId);
    if (dataChannel) {
      dataChannel.close();
      this.dataChannels.delete(peerId);
    }
  }

  discoverPeers() {
    this.signalingChannel.send({
      action: 'discover_peers'
    });
  }

  getConnectedPeers() {
    return Array.from(this.peerConnections.keys()).filter(peerId => {
      const connection = this.peerConnections.get(peerId);
      return connection.connectionState === 'connected';
    });
  }

  // Override these methods in your application
  onPeerConnected(peerId) {
    console.log('Peer connected:', peerId);
  }

  onPeerDisconnected(peerId) {
    console.log('Peer disconnected:', peerId);
  }

  onDataChannelOpen(peerId) {
    console.log('Data channel opened with:', peerId);
  }

  onEncryptedMessage(peerId, message) {
    console.log('Received encrypted message from:', peerId, message);
  }

  onPeerListReceived(peers) {
    console.log('Received peer list:', peers);
  }
}

export default WebRTCManager;