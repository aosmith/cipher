// Post Creation with File Upload and Drag & Drop
class PostCreation {
  constructor() {
    this.fileUploadArea = document.getElementById('file-upload-area');
    this.fileInput = document.getElementById('file-input');
    this.filePreviewsContainer = document.getElementById('file-previews');
    this.form = document.getElementById('post-form');
    
    this.selectedFiles = [];
    
    this.init();
  }

  init() {
    if (!this.form) return; // Exit if not on post creation page
    
    this.setupEventListeners();
    this.setupDragAndDrop();
  }

  setupEventListeners() {
    this.fileUploadArea?.addEventListener('drop', this.handleDrop.bind(this), false);
    this.fileInput?.addEventListener('change', this.handleFileSelect.bind(this), false);
    
    // Make removeFile globally accessible for onclick handlers
    window.removeFile = this.removeFile.bind(this);
  }

  setupDragAndDrop() {
    if (!this.fileUploadArea) return;
    
    // Prevent default drag behaviors
    ['dragenter', 'dragover', 'dragleave', 'drop'].forEach(eventName => {
      this.fileUploadArea.addEventListener(eventName, this.preventDefaults, false);
    });

    // Highlight drop area when item is dragged over it
    ['dragenter', 'dragover'].forEach(eventName => {
      this.fileUploadArea.addEventListener(eventName, () => {
        this.fileUploadArea.classList.add('drag-over');
      }, false);
    });

    ['dragleave', 'drop'].forEach(eventName => {
      this.fileUploadArea.addEventListener(eventName, () => {
        this.fileUploadArea.classList.remove('drag-over');
      }, false);
    });
  }

  preventDefaults(e) {
    e.preventDefault();
    e.stopPropagation();
  }

  handleDrop(e) {
    const dt = e.dataTransfer;
    const files = dt.files;
    this.handleFiles(files);
  }

  handleFileSelect(e) {
    const files = e.target.files;
    this.handleFiles(files);
  }

  handleFiles(files) {
    Array.from(files).forEach(file => this.addFile(file));
    this.updateFileInput();
  }

  addFile(file) {
    // Check if file is already added
    if (this.selectedFiles.find(f => f.name === file.name && f.size === file.size)) {
      return; // File already added
    }
    
    this.selectedFiles.push(file);
    this.createFilePreview(file);
  }

  createFilePreview(file) {
    const preview = document.createElement('div');
    preview.className = 'file-preview';
    preview.dataset.fileName = file.name;
    
    let mediaElement = '';
    
    if (file.type.startsWith('image/')) {
      const reader = new FileReader();
      reader.onload = (e) => {
        const img = preview.querySelector('img') || document.createElement('img');
        img.src = e.target.result;
        img.alt = file.name;
        if (!preview.querySelector('img')) {
          preview.insertBefore(img, preview.querySelector('.file-preview-info'));
        }
      };
      reader.readAsDataURL(file);
      mediaElement = '<img alt="Preview loading...">';
    } else if (file.type.startsWith('video/')) {
      const reader = new FileReader();
      reader.onload = (e) => {
        const video = preview.querySelector('video') || document.createElement('video');
        video.src = e.target.result;
        video.controls = true;
        video.muted = true;
        if (!preview.querySelector('video')) {
          preview.insertBefore(video, preview.querySelector('.file-preview-info'));
        }
      };
      reader.readAsDataURL(file);
      mediaElement = '<video controls muted>Video preview loading...</video>';
    } else {
      mediaElement = `<div class="file-icon" style="font-size: 3em; margin: 20px 0;">📄</div>`;
    }
    
    preview.innerHTML = `
      ${mediaElement}
      <button type="button" class="remove-file" onclick="removeFile('${file.name}')">&times;</button>
      <div class="file-preview-info">
        <div class="file-preview-name">${file.name}</div>
        <div class="file-preview-size">${this.formatFileSize(file.size)}</div>
      </div>
    `;
    
    if (this.filePreviewsContainer) {
      this.filePreviewsContainer.appendChild(preview);
    }
  }

  removeFile(fileName) {
    this.selectedFiles = this.selectedFiles.filter(file => file.name !== fileName);
    const preview = document.querySelector(`[data-file-name="${fileName}"]`);
    if (preview) {
      preview.remove();
    }
    this.updateFileInput();
  }

  updateFileInput() {
    if (!this.fileInput) return;
    
    const dt = new DataTransfer();
    this.selectedFiles.forEach(file => dt.items.add(file));
    this.fileInput.files = dt.files;
  }

  formatFileSize(bytes) {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  }
}

// Initialize when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
  new PostCreation();
});

export default PostCreation;