# Cipher GitHub Pages

This directory contains the GitHub Pages site for the Cipher project.

## 🌐 Live Site

Visit the live site at: `https://your-username.github.io/cipher/`

## 📁 Files

- `index.html` - Main landing page
- `styles.css` - CSS styling and responsive design
- `script.js` - Interactive JavaScript functionality
- `_config.yml` - Jekyll configuration for GitHub Pages

## 🚀 Setup Instructions

### 1. Enable GitHub Pages

1. Go to your repository on GitHub
2. Navigate to **Settings** → **Pages**
3. Under **Source**, select "Deploy from a branch"
4. Choose **Branch**: `main` or `master`
5. Choose **Folder**: `/docs`
6. Click **Save**

### 2. Update Configuration

Before deploying, update the following in the files:

#### In `index.html`:
- Replace `your-username` with your actual GitHub username
- Update social media links and meta tags

#### In `_config.yml`:
- Update `url` and `baseurl` with your GitHub username and repo name
- Add your social media handles

#### In `script.js`:
- Update the `githubUsername` and `repoName` variables

### 3. Custom Domain (Optional)

To use a custom domain:

1. Create a `CNAME` file in the `/docs` directory
2. Add your domain name to the file (e.g., `cipher.yourdomain.com`)
3. Configure your DNS settings to point to GitHub Pages

## 🎨 Customization

### Colors and Branding

The site uses a modern purple gradient color scheme:
- Primary: `#667eea` to `#764ba2`
- Accent: `#ffeaa7` to `#fab1a0`

You can customize these in `styles.css` by updating the CSS variables.

### Content Updates

- **Hero Section**: Update the main title and description
- **Features**: Modify the feature cards to highlight your specific implementation
- **Tech Stack**: Update the technology items based on your actual stack
- **Setup Instructions**: Customize the installation steps

### Adding Screenshots

To add actual screenshots of your application:

1. Create an `images/` directory in `/docs`
2. Add your screenshots (recommended: PNG format, high resolution)
3. Update the demo section in `index.html` to display your images

Example:
```html
<div class="demo-screenshots">
    <img src="images/dashboard.png" alt="Cipher Dashboard" />
    <img src="images/encryption.png" alt="Client-side Encryption" />
</div>
```

## 🔧 Development

To test the site locally:

1. Install Jekyll: `gem install bundler jekyll`
2. Navigate to the `/docs` directory
3. Run: `bundle exec jekyll serve`
4. Visit `http://localhost:4000`

## 📊 Analytics (Optional)

To add Google Analytics:

1. Get your GA tracking ID
2. Update `google_analytics` in `_config.yml`
3. The site will automatically include tracking code

## 🌟 Features

The homepage includes:

- **Responsive Design**: Works on all device sizes
- **Smooth Scrolling**: Navigation with smooth scroll effects
- **Copy-to-Clipboard**: Interactive setup instructions
- **Animations**: Subtle animations and hover effects
- **SEO Optimized**: Meta tags and structured data
- **Performance**: Optimized loading and minimal dependencies

## 📝 License

This GitHub Pages site is part of the Cipher project and follows the same license as the main repository.