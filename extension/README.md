# Unruggable Browser Extension

Browser extension that connects Solana dApps to your Unruggable desktop wallet.

## How It Works

```
Website (Jupiter, Raydium, etc.)
    ↕ window.solana API
Browser Extension (content.js + inject.js)
    ↕ WebSocket (ws://localhost:7777)
Desktop App (Rust)
    ↕ Wallet signing
```

## Installation

### For Development

1. **Ensure Desktop App is Running**
   ```bash
   # The desktop app should be running with bridge server on port 7777
   cargo run --features desktop
   ```

2. **Load Extension in Chrome**
   - Open Chrome and go to `chrome://extensions/`
   - Enable "Developer mode" (toggle in top right)
   - Click "Load unpacked"
   - Select the `extension` folder
   - The extension should now appear with a puzzle piece icon

3. **Test the Extension**
   - Open `extension/test.html` in your browser
   - Click "Connect Wallet"
   - Check desktop app for approval prompt

### For Production

1. **Add Icons**
   - Create `icons/icon16.png` (16x16)
   - Create `icons/icon48.png` (48x48)
   - Create `icons/icon128.png` (128x128)

2. **Build Desktop App**
   ```bash
   cargo build --release --features desktop
   ```

3. **Package Extension**
   ```bash
   cd extension
   zip -r unruggable-extension.zip . -x "*.md" -x "test.html"
   ```

4. **Publish to Chrome Web Store**
   - Go to [Chrome Developer Dashboard](https://chrome.google.com/webstore/devconsole)
   - Upload `unruggable-extension.zip`
   - Fill in store listing details
   - Submit for review

## Files

- **manifest.json** - Extension configuration
- **inject.js** - Injected into web pages, provides `window.solana` API
- **content.js** - Bridges injected script and background worker
- **background.js** - Communicates with desktop app via WebSocket
- **test.html** - Test page for development

## Usage

Once installed:

1. Start Unruggable desktop app (bridge auto-starts on port 7777)
2. Visit any Solana dApp (Jupiter, Raydium, Orca, etc.)
3. Click "Connect Wallet"
4. Select "Unruggable" from wallet options
5. Approve connection in desktop app
6. Use dApp normally - all transactions require desktop approval

## Supported Wallet Adapter Methods

- `connect()` - Connect wallet
- `disconnect()` - Disconnect wallet
- `signTransaction(tx)` - Sign a single transaction
- `signAllTransactions(txs)` - Sign multiple transactions
- `signMessage(msg)` - Sign arbitrary message
- Event listeners: `on('connect')`, `on('disconnect')`

## Development

### Testing Locally

```bash
# Terminal 1: Run desktop app
cargo run --features desktop

# Terminal 2: Serve test page
python3 -m http.server 8000

# Browser: Load extension and open http://localhost:8000/extension/test.html
```

### Debug Logs

- Extension background logs: `chrome://extensions/` → Extension → "service worker" → Console
- Content script logs: Open webpage → F12 → Console
- Desktop app logs: Terminal where `cargo run` is running

## Troubleshooting

**Extension not detected:**
- Refresh the page after installing extension
- Check extension is enabled in `chrome://extensions/`

**"Desktop app not connected" error:**
- Ensure desktop app is running
- Check port 7777 is not blocked by firewall
- Look for "🌉 Browser bridge running on ws://localhost:7777" in desktop app logs

**Approval modal not showing:**
- Desktop app needs UI implementation for approval modals
- Check desktop app terminal for incoming requests

## Next Steps (Implementation TODO)

In the desktop app, you need to implement:

1. **Request Handler** - Handle Connect/SignTransaction/SignMessage requests
2. **Approval Modals** - Show UI when browser requests approval
3. **State Management** - Track connected origins and sessions
4. **Security** - Origin allowlist, request rate limiting

See `src/bridge/` for the WebSocket server foundation.

## Security Notes

- Bridge only listens on `127.0.0.1` (localhost) - not accessible from network
- Every transaction requires explicit user approval in desktop app
- Origin tracking prevents unauthorized requests
- WebSocket connection requires desktop app running (no remote server)

## Browser Support

- ✅ Chrome/Chromium
- ✅ Edge
- ✅ Brave
- 🚧 Firefox (requires Manifest V2 version)
- ❌ Safari (requires different extension format)
