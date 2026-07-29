/**
 * Source-level hardening regressions.
 *
 * These guard invariants that are cheap to reintroduce by accident: a remote
 * <script> in the privileged WebView, a console.log of the recovery phrase, or
 * a new inline onclick that interpolates peer-controlled data into JS source.
 */

const fs = require('fs');
const path = require('path');

const SRC = path.join(__dirname, '..', '..');
const MAIN_JS = fs.readFileSync(path.join(SRC, 'js', 'main.js'), 'utf8');
const NAVBAR_JS = fs.readFileSync(path.join(SRC, 'js', 'navbar.js'), 'utf8');
const INDEX_HTML = fs.readFileSync(path.join(SRC, 'index.html'), 'utf8');

// Strip line comments and single-quoted string literals so these source scans
// assert on code, not on prose that happens to mention the thing being banned.
function code(source) {
    return source
        .replace(/^\s*\/\/.*$/gm, '')
        .replace(/'(?:[^'\\\n]|\\.)*'/g, "''");
}

const MAIN_CODE = code(MAIN_JS);
const NAVBAR_CODE = code(NAVBAR_JS);

describe('privileged WebView loads no remote code', () => {
    test('index.html has no external script or stylesheet', () => {
        const remote = INDEX_HTML.match(/(?:src|href)\s*=\s*["']https?:\/\/[^"']+["']/gi) || [];
        expect(remote).toEqual([]);
    });

    test('Google Analytics is gone', () => {
        expect(INDEX_HTML).not.toMatch(/googletagmanager|gtag\(|dataLayer/i);
    });

    test('jsQR is vendored locally and still exports the same global', () => {
        expect(INDEX_HTML).toMatch(/<script src="js\/vendor\/jsQR\.js"><\/script>/);

        const vendored = path.join(SRC, 'js', 'vendor', 'jsQR.js');
        expect(fs.existsSync(vendored)).toBe(true);

        // Evaluate it the way a <script> tag would (no CommonJS/AMD in scope)
        // and confirm it publishes window.jsQR, which main.js calls.
        const vm = require('vm');
        const ctx = { window: {} };
        ctx.self = ctx;
        vm.createContext(ctx);
        vm.runInContext(fs.readFileSync(vendored, 'utf8'), ctx);
        expect(typeof ctx.jsQR).toBe('function');
        expect(MAIN_JS).toMatch(/window\.jsQR/);
    });
});

describe('recovery phrase is not leaked', () => {
    test('the BIP39 mnemonic is never written to the console', () => {
        // console output goes to logcat on Android.
        // Any console argument mentioning the phrase must be a boolean probe.
        const logged = (MAIN_CODE.match(/console\.\w+\([^;]*?\)/gs) || [])
            .filter(call => /\brecoveryPhrase\b/.test(call))
            .filter(call => !/!!\s*recoveryPhrase/.test(call));
        expect(logged).toEqual([]);
        expect(MAIN_JS).not.toMatch(/Save your recovery phrase/);
    });

    test('the phrase is not parked on the window object', () => {
        expect(MAIN_CODE).not.toMatch(/window\.currentRecoveryPhrase/);
    });

    test('confirmRecoveryPhraseSaved clears both the variable and the DOM node', () => {
        const fn = MAIN_JS.slice(MAIN_JS.indexOf('async function confirmRecoveryPhraseSaved'));
        expect(fn).toMatch(/pendingRecoveryPhrase = null/);
        expect(fn).toMatch(/textContent = ''/);
    });

    test('the phrase is still shown to the user during onboarding', () => {
        // Removing the log must not break the only place the user can read it.
        expect(MAIN_JS).toMatch(/getElementById\('recoveryPhraseText'\)\.textContent = recoveryPhrase/);
        expect(INDEX_HTML).toMatch(/id="recoveryPhraseText"/);
    });

    test('whole user objects and raw friend-request payloads are not dumped', () => {
        expect(MAIN_JS).not.toMatch(/Full user object received/);
        expect(MAIN_JS).not.toMatch(/JSON\.stringify\((?:pendingRequests|outgoingRequests)/);
    });
});

describe('no inline handlers carry interpolated data', () => {
    const INLINE_HANDLER_WITH_INTERPOLATION = /\bon(?:click|keypress|keydown|change|input|error|load|mouseover)\s*=\s*"[^"]*\$\{/gi;

    test('main.js', () => {
        expect(MAIN_CODE.match(INLINE_HANDLER_WITH_INTERPOLATION) || []).toEqual([]);
    });

    test('navbar.js', () => {
        expect(NAVBAR_CODE.match(INLINE_HANDLER_WITH_INTERPOLATION) || []).toEqual([]);
    });
});

describe('inline post edit does not round-trip markup through an attribute', () => {
    test('cancelEdit reads from an in-memory map, not encodeURIComponent', () => {
        expect(MAIN_JS).not.toMatch(/decodeURIComponent\(encodedOriginal\)/);
        expect(MAIN_JS).toMatch(/editOriginals: new Map\(\)/);
    });
});
