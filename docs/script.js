// Smooth scrolling for navigation links
document.querySelectorAll('a[href^="#"]').forEach(anchor => {
    anchor.addEventListener('click', function (e) {
        e.preventDefault();
        const target = document.querySelector(this.getAttribute('href'));
        if (target) {
            target.scrollIntoView({
                behavior: 'smooth',
                block: 'start'
            });
        }
    });
});

// Copy to clipboard functionality
function copyToClipboard(text) {
    navigator.clipboard.writeText(text).then(() => {
        // Visual feedback for successful copy
        const event = document.activeElement;
        if (event && event.classList.contains('copy-btn')) {
            const original = event.textContent;
            event.textContent = '✅';
            setTimeout(() => {
                event.textContent = original;
            }, 1500);
        }
    }).catch(err => {
        console.error('Failed to copy text: ', err);
        // Fallback for older browsers
        fallbackCopyTextToClipboard(text);
    });
}

// Fallback copy function for older browsers
function fallbackCopyTextToClipboard(text) {
    const textArea = document.createElement("textarea");
    textArea.value = text;
    textArea.style.top = "0";
    textArea.style.left = "0";
    textArea.style.position = "fixed";

    document.body.appendChild(textArea);
    textArea.focus();
    textArea.select();

    try {
        const successful = document.execCommand('copy');
        if (successful) {
            const event = document.activeElement;
            if (event && event.classList.contains('copy-btn')) {
                const original = event.textContent;
                event.textContent = '✅';
                setTimeout(() => {
                    event.textContent = original;
                }, 1500);
            }
        }
    } catch (err) {
        console.error('Fallback: Oops, unable to copy', err);
    }

    document.body.removeChild(textArea);
}

// Navbar scroll effect
window.addEventListener('scroll', function() {
    const navbar = document.querySelector('.navbar');
    if (window.scrollY > 50) {
        navbar.style.background = 'rgba(255, 255, 255, 0.98)';
        navbar.style.boxShadow = '0 2px 20px rgba(0, 0, 0, 0.1)';
    } else {
        navbar.style.background = 'rgba(255, 255, 255, 0.95)';
        navbar.style.boxShadow = 'none';
    }
});

// Intersection Observer for animations
const observerOptions = {
    threshold: 0.1,
    rootMargin: '0px 0px -50px 0px'
};

const observer = new IntersectionObserver((entries) => {
    entries.forEach(entry => {
        if (entry.isIntersecting) {
            entry.target.style.opacity = '1';
            entry.target.style.transform = 'translateY(0)';
        }
    });
}, observerOptions);

// Observe elements for animation
document.addEventListener('DOMContentLoaded', function() {
    // Add animation classes to elements
    const animatedElements = document.querySelectorAll('.feature-card, .tech-category, .setup-card');
    animatedElements.forEach(el => {
        el.style.opacity = '0';
        el.style.transform = 'translateY(20px)';
        el.style.transition = 'opacity 0.6s ease, transform 0.6s ease';
        observer.observe(el);
    });
    
    // Update GitHub links if they exist
    updateGitHubLinks();
    
    // Add typing effect to hero title
    typewriterEffect();
});

// Update GitHub links with actual repository URL
function updateGitHubLinks() {
    // This would be updated with your actual GitHub username/repo
    const githubUsername = 'your-username';
    const repoName = 'cipher';
    const githubUrl = `https://github.com/${githubUsername}/${repoName}`;
    
    // Update all GitHub links
    document.querySelectorAll('a[href*="your-username"]').forEach(link => {
        link.href = link.href.replace('your-username', githubUsername);
    });
    
    // Update og:url meta tag
    const ogUrl = document.querySelector('meta[property="og:url"]');
    if (ogUrl) {
        ogUrl.content = `https://${githubUsername}.github.io/${repoName}/`;
    }
    
    // Update twitter:url meta tag
    const twitterUrl = document.querySelector('meta[property="twitter:url"]');
    if (twitterUrl) {
        twitterUrl.content = `https://${githubUsername}.github.io/${repoName}/`;
    }
}

// Typewriter effect for hero title
function typewriterEffect() {
    const heroTitle = document.querySelector('.hero-title');
    if (!heroTitle) return;
    
    const originalText = heroTitle.innerHTML;
    heroTitle.innerHTML = '';
    
    let i = 0;
    const speed = 50; // typing speed in milliseconds
    
    function typeWriter() {
        if (i < originalText.length) {
            heroTitle.innerHTML += originalText.charAt(i);
            i++;
            setTimeout(typeWriter, speed);
        }
    }
    
    // Start typing effect after a short delay
    setTimeout(typeWriter, 500);
}

// Enhanced node animation for hero visual
document.addEventListener('DOMContentLoaded', function() {
    const nodes = document.querySelectorAll('.node');
    
    // Add staggered animation delays
    nodes.forEach((node, index) => {
        node.style.animationDelay = `${index * 0.5}s`;
    });
    
    // Add hover effects to feature cards
    const featureCards = document.querySelectorAll('.feature-card');
    featureCards.forEach(card => {
        card.addEventListener('mouseenter', function() {
            this.style.transform = 'translateY(-10px) scale(1.02)';
        });
        
        card.addEventListener('mouseleave', function() {
            this.style.transform = 'translateY(-5px) scale(1)';
        });
    });
});

// Add loading animation
window.addEventListener('load', function() {
    document.body.style.opacity = '0';
    document.body.style.transition = 'opacity 0.5s ease';
    
    setTimeout(() => {
        document.body.style.opacity = '1';
    }, 100);
});

// Easter egg: Konami code
let konamiCode = [];
const konamiSequence = [38, 38, 40, 40, 37, 39, 37, 39, 66, 65]; // Up Up Down Down Left Right Left Right B A

document.addEventListener('keydown', function(e) {
    konamiCode.push(e.keyCode);
    
    if (konamiCode.length > konamiSequence.length) {
        konamiCode.shift();
    }
    
    if (konamiCode.length === konamiSequence.length) {
        let match = true;
        for (let i = 0; i < konamiSequence.length; i++) {
            if (konamiCode[i] !== konamiSequence[i]) {
                match = false;
                break;
            }
        }
        
        if (match) {
            // Easter egg activated!
            document.body.style.filter = 'hue-rotate(180deg)';
            setTimeout(() => {
                document.body.style.filter = 'none';
            }, 5000);
            
            // Show a fun message
            const message = document.createElement('div');
            message.innerHTML = '🔐 Encryption Matrix Mode Activated! 🔐';
            message.style.position = 'fixed';
            message.style.top = '50%';
            message.style.left = '50%';
            message.style.transform = 'translate(-50%, -50%)';
            message.style.background = 'rgba(0, 0, 0, 0.8)';
            message.style.color = '#00ff00';
            message.style.padding = '2rem';
            message.style.borderRadius = '12px';
            message.style.fontSize = '1.5rem';
            message.style.fontWeight = 'bold';
            message.style.zIndex = '9999';
            message.style.fontFamily = 'Monaco, Consolas, monospace';
            
            document.body.appendChild(message);
            
            setTimeout(() => {
                document.body.removeChild(message);
            }, 3000);
            
            konamiCode = []; // Reset
        }
    }
});

console.log('%c🔐 Cipher - End-to-End Encrypted P2P Social Network', 'color: #667eea; font-size: 20px; font-weight: bold;');
console.log('%cBuilt with privacy and security in mind 🛡️', 'color: #764ba2; font-size: 14px;');
console.log('%cInterested in the code? Check out: https://github.com/your-username/cipher', 'color: #333; font-size: 12px;');