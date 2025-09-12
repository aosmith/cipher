/**
 * Friend Selector Component for Group File Sharing
 * Provides UI for selecting friends to share files with
 */

class FriendSelector {
  constructor(containerId, options = {}) {
    this.containerId = containerId;
    this.options = {
      multiple: true,
      maxSelections: 10,
      showSearch: true,
      onSelectionChange: null,
      ...options
    };
    
    this.friends = [];
    this.selectedFriends = [];
    this.isVisible = false;
    
    this.init();
  }

  async init() {
    await this.loadFriends();
    this.render();
    this.bindEvents();
  }

  async loadFriends() {
    try {
      const response = await fetch('/api/v1/friends', {
        headers: {
          'X-Requested-With': 'XMLHttpRequest'
        }
      });
      
      if (response.ok) {
        this.friends = await response.json();
      } else {
        console.warn('Failed to load friends');
        this.friends = [];
      }
    } catch (error) {
      console.error('Error loading friends:', error);
      this.friends = [];
    }
  }

  render() {
    const container = document.getElementById(this.containerId);
    if (!container) return;

    container.innerHTML = `
      <div class="friend-selector" style="display: ${this.isVisible ? 'block' : 'none'}">
        <div class="friend-selector-header">
          <h3>Share with Friends</h3>
          <button type="button" class="close-btn" aria-label="Close">×</button>
        </div>
        
        ${this.options.showSearch ? `
          <div class="friend-search">
            <input type="text" class="search-input" placeholder="Search friends..." />
          </div>
        ` : ''}
        
        <div class="friends-list">
          ${this.renderFriendsList()}
        </div>
        
        <div class="selected-friends">
          <div class="selected-header">
            <span>Selected (${this.selectedFriends.length})</span>
            ${this.selectedFriends.length > 0 ? '<button type="button" class="clear-all">Clear All</button>' : ''}
          </div>
          <div class="selected-list">
            ${this.renderSelectedFriends()}
          </div>
        </div>
        
        <div class="friend-selector-actions">
          <button type="button" class="btn btn-secondary cancel-btn">Cancel</button>
          <button type="button" class="btn btn-primary confirm-btn" ${this.selectedFriends.length === 0 ? 'disabled' : ''}>
            Share with ${this.selectedFriends.length} friend${this.selectedFriends.length !== 1 ? 's' : ''}
          </button>
        </div>
      </div>
      
      <div class="friend-selector-backdrop" style="display: ${this.isVisible ? 'block' : 'none'}"></div>
    `;

    this.addStyles();
  }

  renderFriendsList() {
    if (this.friends.length === 0) {
      return `
        <div class="no-friends">
          <div class="no-friends-icon">👥</div>
          <p>No friends yet</p>
          <small>Add friends to share files with them</small>
        </div>
      `;
    }

    return this.friends.map(friend => {
      const isSelected = this.selectedFriends.some(f => f.id === friend.id);
      const isDisabled = !isSelected && this.selectedFriends.length >= this.options.maxSelections;
      
      return `
        <div class="friend-item ${isSelected ? 'selected' : ''} ${isDisabled ? 'disabled' : ''}" 
             data-friend-id="${friend.id}"
             data-public-key="${friend.public_key}">
          <div class="friend-avatar">
            ${this.getAvatarInitials(friend.username)}
          </div>
          <div class="friend-info">
            <div class="friend-name">${this.escapeHtml(friend.username)}</div>
            <div class="friend-status">
              ${friend.display_name ? this.escapeHtml(friend.display_name) : ''}
            </div>
          </div>
          <div class="friend-checkbox">
            <input type="checkbox" ${isSelected ? 'checked' : ''} ${isDisabled ? 'disabled' : ''} />
          </div>
        </div>
      `;
    }).join('');
  }

  renderSelectedFriends() {
    if (this.selectedFriends.length === 0) {
      return '<div class="no-selection">No friends selected</div>';
    }

    return this.selectedFriends.map(friend => `
      <div class="selected-friend" data-friend-id="${friend.id}">
        <span class="selected-friend-name">${this.escapeHtml(friend.username)}</span>
        <button type="button" class="remove-friend" data-friend-id="${friend.id}">×</button>
      </div>
    `).join('');
  }

  bindEvents() {
    const container = document.getElementById(this.containerId);
    if (!container) return;

    // Close button
    container.addEventListener('click', (e) => {
      if (e.target.classList.contains('close-btn') || e.target.classList.contains('cancel-btn')) {
        this.hide();
      }
    });

    // Backdrop click to close
    container.addEventListener('click', (e) => {
      if (e.target.classList.contains('friend-selector-backdrop')) {
        this.hide();
      }
    });

    // Friend selection
    container.addEventListener('click', (e) => {
      const friendItem = e.target.closest('.friend-item');
      if (friendItem && !friendItem.classList.contains('disabled')) {
        this.toggleFriendSelection(friendItem);
      }
    });

    // Remove selected friend
    container.addEventListener('click', (e) => {
      if (e.target.classList.contains('remove-friend')) {
        const friendId = parseInt(e.target.dataset.friendId);
        this.removeFriendSelection(friendId);
      }
    });

    // Clear all selections
    container.addEventListener('click', (e) => {
      if (e.target.classList.contains('clear-all')) {
        this.clearAllSelections();
      }
    });

    // Confirm selection
    container.addEventListener('click', (e) => {
      if (e.target.classList.contains('confirm-btn') && !e.target.disabled) {
        this.confirmSelection();
      }
    });

    // Search functionality
    const searchInput = container.querySelector('.search-input');
    if (searchInput) {
      searchInput.addEventListener('input', (e) => {
        this.filterFriends(e.target.value);
      });
    }
  }

  toggleFriendSelection(friendItem) {
    const friendId = parseInt(friendItem.dataset.friendId);
    const publicKey = friendItem.dataset.publicKey;
    const friend = this.friends.find(f => f.id === friendId);
    
    if (!friend) return;

    const isCurrentlySelected = this.selectedFriends.some(f => f.id === friendId);
    
    if (isCurrentlySelected) {
      this.selectedFriends = this.selectedFriends.filter(f => f.id !== friendId);
    } else {
      if (this.selectedFriends.length < this.options.maxSelections) {
        this.selectedFriends.push({ ...friend, public_key: publicKey });
      }
    }

    this.render();
    this.notifySelectionChange();
  }

  removeFriendSelection(friendId) {
    this.selectedFriends = this.selectedFriends.filter(f => f.id !== friendId);
    this.render();
    this.notifySelectionChange();
  }

  clearAllSelections() {
    this.selectedFriends = [];
    this.render();
    this.notifySelectionChange();
  }

  filterFriends(searchTerm) {
    const friendItems = document.querySelectorAll('.friend-item');
    const term = searchTerm.toLowerCase();

    friendItems.forEach(item => {
      const friendName = item.querySelector('.friend-name').textContent.toLowerCase();
      const friendStatus = item.querySelector('.friend-status').textContent.toLowerCase();
      
      if (friendName.includes(term) || friendStatus.includes(term)) {
        item.style.display = 'flex';
      } else {
        item.style.display = 'none';
      }
    });
  }

  confirmSelection() {
    if (this.options.onSelectionChange) {
      this.options.onSelectionChange(this.selectedFriends);
    }
    this.hide();
  }

  notifySelectionChange() {
    if (this.options.onSelectionChange) {
      this.options.onSelectionChange(this.selectedFriends);
    }
  }

  // Public API methods
  show() {
    this.isVisible = true;
    this.render();
  }

  hide() {
    this.isVisible = false;
    this.render();
  }

  getSelectedFriends() {
    return [...this.selectedFriends];
  }

  setSelectedFriends(friends) {
    this.selectedFriends = [...friends];
    this.render();
  }

  // Utility methods
  getAvatarInitials(username) {
    return username.substring(0, 2).toUpperCase();
  }

  escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  addStyles() {
    if (document.getElementById('friend-selector-styles')) return;

    const style = document.createElement('style');
    style.id = 'friend-selector-styles';
    style.textContent = `
      .friend-selector {
        position: fixed;
        top: 50%;
        left: 50%;
        transform: translate(-50%, -50%);
        width: 90%;
        max-width: 500px;
        max-height: 80vh;
        background: white;
        border-radius: 12px;
        box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
        z-index: 10000;
        display: flex;
        flex-direction: column;
      }

      .friend-selector-backdrop {
        position: fixed;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(0, 0, 0, 0.5);
        z-index: 9999;
      }

      .friend-selector-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 20px;
        border-bottom: 1px solid #e5e7eb;
      }

      .friend-selector-header h3 {
        margin: 0;
        font-size: 1.2rem;
        font-weight: 600;
      }

      .close-btn {
        background: none;
        border: none;
        font-size: 1.5rem;
        cursor: pointer;
        color: #6b7280;
        padding: 0;
        width: 30px;
        height: 30px;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 4px;
      }

      .close-btn:hover {
        background: #f3f4f6;
      }

      .friend-search {
        padding: 15px 20px;
        border-bottom: 1px solid #e5e7eb;
      }

      .search-input {
        width: 100%;
        padding: 8px 12px;
        border: 1px solid #d1d5db;
        border-radius: 6px;
        font-size: 14px;
      }

      .friends-list {
        flex: 1;
        overflow-y: auto;
        max-height: 300px;
        padding: 10px;
      }

      .friend-item {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 12px;
        border-radius: 8px;
        cursor: pointer;
        transition: background-color 0.2s;
      }

      .friend-item:hover:not(.disabled) {
        background: #f9fafb;
      }

      .friend-item.selected {
        background: #eff6ff;
        border: 1px solid #3b82f6;
      }

      .friend-item.disabled {
        opacity: 0.5;
        cursor: not-allowed;
      }

      .friend-avatar {
        width: 40px;
        height: 40px;
        border-radius: 50%;
        background: #3b82f6;
        color: white;
        display: flex;
        align-items: center;
        justify-content: center;
        font-weight: 600;
        font-size: 14px;
      }

      .friend-info {
        flex: 1;
      }

      .friend-name {
        font-weight: 500;
        margin-bottom: 2px;
      }

      .friend-status {
        font-size: 12px;
        color: #6b7280;
      }

      .friend-checkbox input {
        width: 18px;
        height: 18px;
      }

      .selected-friends {
        border-top: 1px solid #e5e7eb;
        padding: 15px 20px;
        max-height: 120px;
        overflow-y: auto;
      }

      .selected-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 10px;
        font-weight: 500;
        font-size: 14px;
      }

      .clear-all {
        background: none;
        border: none;
        color: #ef4444;
        font-size: 12px;
        cursor: pointer;
        padding: 2px 6px;
        border-radius: 4px;
      }

      .clear-all:hover {
        background: #fef2f2;
      }

      .selected-list {
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
      }

      .selected-friend {
        display: flex;
        align-items: center;
        gap: 6px;
        background: #eff6ff;
        padding: 4px 8px;
        border-radius: 16px;
        font-size: 13px;
        border: 1px solid #3b82f6;
      }

      .remove-friend {
        background: none;
        border: none;
        color: #6b7280;
        cursor: pointer;
        width: 16px;
        height: 16px;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 50%;
        font-size: 14px;
      }

      .remove-friend:hover {
        background: rgba(0, 0, 0, 0.1);
      }

      .no-selection {
        color: #6b7280;
        font-size: 13px;
        font-style: italic;
      }

      .no-friends {
        text-align: center;
        padding: 40px 20px;
        color: #6b7280;
      }

      .no-friends-icon {
        font-size: 2rem;
        margin-bottom: 10px;
      }

      .friend-selector-actions {
        display: flex;
        justify-content: flex-end;
        gap: 12px;
        padding: 20px;
        border-top: 1px solid #e5e7eb;
      }

      .btn {
        padding: 8px 16px;
        border-radius: 6px;
        border: none;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.2s;
      }

      .btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
      }

      .btn-secondary {
        background: #f3f4f6;
        color: #374151;
      }

      .btn-secondary:hover:not(:disabled) {
        background: #e5e7eb;
      }

      .btn-primary {
        background: #3b82f6;
        color: white;
      }

      .btn-primary:hover:not(:disabled) {
        background: #2563eb;
      }

      @media (max-width: 640px) {
        .friend-selector {
          width: 95%;
          max-height: 85vh;
        }

        .friend-item {
          padding: 10px;
        }

        .friend-avatar {
          width: 35px;
          height: 35px;
          font-size: 12px;
        }
      }
    `;
    
    document.head.appendChild(style);
  }
}

// Export for ES6 modules
export default FriendSelector;

// Also make available globally
window.FriendSelector = FriendSelector;