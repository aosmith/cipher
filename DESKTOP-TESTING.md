# Desktop Application Testing Strategy

This document outlines the testing approach for Cipher's Tauri-based desktop application.

## Current Architecture

- **Desktop App**: Tauri 2.0 with Rust backend + WebView frontend
- **Web Content**: Rails 8.0.2 application served locally on port 3001
- **Platform**: Cross-platform (Windows, macOS, Linux)

## Testing Analysis

### Web vs Desktop Testing

Since the Tauri app is essentially a WebView wrapper around the Rails application:
- **95% of functionality** is the Rails web application
- **5% is desktop-specific** (window management, system tray, file system access)

### Current Test Coverage

✅ **Comprehensive Rails System Tests**
- Complete friend management e2e tests (14 tests, 70 assertions)
- User registration workflow tests
- Authentication and API endpoint testing
- Database integrity and business logic validation

✅ **Unit Tests**
- Model validations and associations
- Controller API endpoints
- Service layer functionality

## Desktop Testing Options

### Platform Support Matrix

| Platform | WebDriver Support | Status |
|----------|-------------------|---------|
| Windows  | ✅ Edge Driver   | Full automated testing available |
| Linux    | ✅ WebKitWebDriver | Full automated testing available |
| macOS    | ❌ No WKWebView driver | **Limited - No WebDriver support** |

### Tauri WebDriver Testing

For platforms that support it (Windows/Linux):

```bash
# Install tauri-driver
cargo install tauri-driver --locked

# Platform-specific setup
# Linux: webkit2gtk-driver package
# Windows: msedgedriver.exe in PATH
```

**Test Structure:**
```javascript
// Selenium + Mocha + Chai
const capabilities = new Capabilities();
capabilities.set('tauri:options', { application: './target/debug/app' });
capabilities.setBrowserName('wry');

const driver = await new Builder()
  .withCapabilities(capabilities)
  .usingServer('http://127.0.0.1:4444')
  .build();
```

## Recommended Testing Strategy

### Phase 1: Current Approach (Implemented) ✅

**Web Application Testing:**
- Comprehensive Selenium tests for all user workflows
- Rails system tests cover 95% of application functionality
- Database integrity and API endpoint validation
- Authentication and authorization flows

**Benefits:**
- Tests the actual business logic users interact with
- Same codebase runs in desktop WebView
- Fast, reliable, easy to maintain
- Cross-platform by nature

### Phase 2: Desktop-Specific Testing

**Manual Testing:**
- Window management (resize, minimize, maximize, close)
- System tray functionality
- Desktop notifications
- File system integration
- Auto-updater (when implemented)

**Automated Unit Testing:**
- Rust backend components
- Tauri command handlers
- Desktop-specific business logic

### Phase 3: CI/CD Integration (Future)

**Linux-based WebDriver Testing:**
```yaml
# GitHub Actions example
name: Desktop E2E Tests
runs-on: ubuntu-latest
steps:
  - name: Install WebKit driver
    run: sudo apt-get install webkit2gtk-driver
  - name: Run Tauri WebDriver tests
    run: npm run test:desktop
```

## Current Status

### ✅ What's Covered
- All core application functionality via web tests
- User workflows and business logic
- API endpoints and data integrity
- Authentication and session management
- Error handling and edge cases

### 🔄 Manual Testing Required
- Desktop window behavior
- System integration features
- Platform-specific styling
- Installation and auto-update flows

### 📋 Future Automation Opportunities
- CI/CD testing on Linux runners
- Cross-platform build verification
- Desktop-specific integration tests

## Development Workflow

1. **Feature Development**: Build and test via Rails web application
2. **Web Testing**: Comprehensive system tests ensure functionality works
3. **Desktop Build**: `./scripts/build-desktop.sh` creates desktop app
4. **Manual Verification**: Test desktop-specific features manually
5. **Release**: Confidence in core functionality from web tests

## Conclusion

The current web testing strategy provides excellent coverage for the Cipher desktop application. Since the desktop app is a WebView wrapper, the Rails system tests effectively validate the user experience that will be delivered in the desktop environment.

This approach maximizes test coverage while minimizing complexity and maintenance overhead.