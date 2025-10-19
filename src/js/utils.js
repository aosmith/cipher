// Consolidated JavaScript utilities to eliminate redundancy

// API call wrapper - replaces repeated fetch patterns
async function apiCall(endpoint, options = {}) {
    try {
        const response = await fetch(`http://localhost:3000${endpoint}`, {
            ...options,
            headers: {
                'Content-Type': 'application/json',
                ...options.headers
            }
        });

        if (!response.ok) {
            const error = await response.json().catch(() => ({ message: 'Network error' }));
            throw new Error(error.message || `HTTP ${response.status}`);
        }

        return await response.json();
    } catch (error) {
        console.error(`API call failed: ${endpoint}`, error);
        throw error;
    }
}

// Show/hide elements with optional animation
function toggleElement(elementId, show, animate = true) {
    const element = document.getElementById(elementId);
    if (!element) return;

    if (show) {
        element.classList.remove('hidden');
        if (animate) element.classList.add('fade-in');
    } else {
        element.classList.add('hidden');
        if (animate) element.classList.remove('fade-in');
    }
}

// Display error messages consistently
function showError(message, elementId = 'error') {
    const errorElement = document.getElementById(elementId);
    if (errorElement) {
        errorElement.textContent = message;
        errorElement.classList.remove('hidden');
        setTimeout(() => errorElement.classList.add('hidden'), 5000);
    }
}

// Display success messages consistently
function showSuccess(message, elementId = 'success') {
    const successElement = document.getElementById(elementId);
    if (successElement) {
        successElement.textContent = message;
        successElement.classList.remove('hidden');
        setTimeout(() => successElement.classList.add('hidden'), 3000);
    }
}

// Format timestamps consistently
function formatTimestamp(timestamp) {
    const date = new Date(timestamp);
    const now = new Date();
    const diff = now - date;
    const seconds = Math.floor(diff / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);

    if (days > 7) {
        return date.toLocaleDateString();
    } else if (days > 0) {
        return `${days} day${days > 1 ? 's' : ''} ago`;
    } else if (hours > 0) {
        return `${hours} hour${hours > 1 ? 's' : ''} ago`;
    } else if (minutes > 0) {
        return `${minutes} minute${minutes > 1 ? 's' : ''} ago`;
    } else {
        return 'just now';
    }
}

// Create HTML elements with classes and content
function createElement(tag, className, content = '') {
    const element = document.createElement(tag);
    if (className) element.className = className;
    if (content) element.innerHTML = content;
    return element;
}

// Debounce function for search inputs
function debounce(func, wait) {
    let timeout;
    return function executedFunction(...args) {
        const later = () => {
            clearTimeout(timeout);
            func(...args);
        };
        clearTimeout(timeout);
        timeout = setTimeout(later, wait);
    };
}

// File to base64 conversion (used in multiple upload scenarios)
async function fileToBase64(file) {
    return new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.readAsDataURL(file);
        reader.onload = () => resolve(reader.result.split(',')[1]);
        reader.onerror = error => reject(error);
    });
}

// Validate form inputs
function validateInput(input, type = 'text') {
    const value = input.value.trim();

    switch(type) {
        case 'email':
            const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
            return emailRegex.test(value);
        case 'username':
            return value.length >= 3 && /^[a-zA-Z0-9_]+$/.test(value);
        case 'required':
            return value.length > 0;
        default:
            return true;
    }
}

// Copy text to clipboard with feedback
async function copyToClipboard(text, feedbackElement = null) {
    try {
        await navigator.clipboard.writeText(text);
        if (feedbackElement) {
            const original = feedbackElement.textContent;
            feedbackElement.textContent = 'Copied!';
            setTimeout(() => feedbackElement.textContent = original, 2000);
        }
        return true;
    } catch (error) {
        console.error('Copy failed:', error);
        return false;
    }
}

// Export utilities for use in main.js
window.utils = {
    apiCall,
    toggleElement,
    showError,
    showSuccess,
    formatTimestamp,
    createElement,
    debounce,
    fileToBase64,
    validateInput,
    copyToClipboard
};