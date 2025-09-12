import { Controller } from "@hotwired/stimulus"

// Connects to data-controller="post-creation"
export default class extends Controller {
  static targets = ["uploadArea", "fileInput", "previews"]
  
  connect() {
    this.selectedFiles = []
    this.setupDragAndDrop()
  }
  
  setupDragAndDrop() {
    // Prevent default drag behaviors
    ['dragenter', 'dragover', 'dragleave', 'drop'].forEach(eventName => {
      this.uploadAreaTarget.addEventListener(eventName, this.preventDefaults.bind(this), false)
    })

    // Highlight drop area when item is dragged over it
    ['dragenter', 'dragover'].forEach(eventName => {
      this.uploadAreaTarget.addEventListener(eventName, this.highlight.bind(this), false)
    })

    ['dragleave', 'drop'].forEach(eventName => {
      this.uploadAreaTarget.addEventListener(eventName, this.unhighlight.bind(this), false)
    })
  }
  
  preventDefaults(e) {
    e.preventDefault()
    e.stopPropagation()
  }
  
  highlight() {
    this.uploadAreaTarget.classList.add('drag-over')
  }
  
  unhighlight() {
    this.uploadAreaTarget.classList.remove('drag-over')
  }
  
  // Stimulus actions
  handleDrop(event) {
    const dt = event.dataTransfer
    const files = dt.files
    this.handleFiles(files)
  }
  
  handleFileSelect(event) {
    const files = event.target.files
    this.handleFiles(files)
  }
  
  handleFiles(files) {
    Array.from(files).forEach(file => this.addFile(file))
    this.updateFileInput()
  }
  
  addFile(file) {
    // Check if file is already added
    if (this.selectedFiles.find(f => f.name === file.name && f.size === file.size)) {
      return // File already added
    }
    
    this.selectedFiles.push(file)
    this.createFilePreview(file)
  }
  
  createFilePreview(file) {
    const preview = document.createElement('div')
    preview.className = 'file-preview'
    preview.dataset.fileName = file.name
    
    let mediaElement = ''
    
    if (file.type.startsWith('image/')) {
      const reader = new FileReader()
      reader.onload = (e) => {
        const img = preview.querySelector('img') || document.createElement('img')
        img.src = e.target.result
        img.alt = file.name
        if (!preview.querySelector('img')) {
          preview.insertBefore(img, preview.querySelector('.file-preview-info'))
        }
      }
      reader.readAsDataURL(file)
      mediaElement = '<img alt="Preview loading...">'
    } else if (file.type.startsWith('video/')) {
      const reader = new FileReader()
      reader.onload = (e) => {
        const video = preview.querySelector('video') || document.createElement('video')
        video.src = e.target.result
        video.controls = true
        video.muted = true
        if (!preview.querySelector('video')) {
          preview.insertBefore(video, preview.querySelector('.file-preview-info'))
        }
      }
      reader.readAsDataURL(file)
      mediaElement = '<video controls muted>Video preview loading...</video>'
    } else {
      mediaElement = `<div class="file-icon" style="font-size: 3em; margin: 20px 0;">📄</div>`
    }
    
    preview.innerHTML = `
      ${mediaElement}
      <button type="button" 
              class="remove-file" 
              data-action="click->post-creation#removeFile"
              data-file-name="${file.name}">&times;</button>
      <div class="file-preview-info">
        <div class="file-preview-name">${file.name}</div>
        <div class="file-preview-size">${this.formatFileSize(file.size)}</div>
      </div>
    `
    
    this.previewsTarget.appendChild(preview)
  }
  
  removeFile(event) {
    const fileName = event.target.dataset.fileName
    this.selectedFiles = this.selectedFiles.filter(file => file.name !== fileName)
    const preview = this.previewsTarget.querySelector(`[data-file-name="${fileName}"]`)
    if (preview) {
      preview.remove()
    }
    this.updateFileInput()
  }
  
  updateFileInput() {
    const dt = new DataTransfer()
    this.selectedFiles.forEach(file => dt.items.add(file))
    this.fileInputTarget.files = dt.files
  }
  
  formatFileSize(bytes) {
    if (bytes === 0) return '0 Bytes'
    const k = 1024
    const sizes = ['Bytes', 'KB', 'MB', 'GB']
    const i = Math.floor(Math.log(bytes) / Math.log(k))
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i]
  }
}