# Extension Popup UI

The extension now includes a popup UI that provides quick access to your wallet status and controls!

## Features

### 🎨 Modern, Minimal Design
- Dark theme matching your desktop app aesthetic
- Signature green (#00ff88) accent color
- Clean, readable interface at 360px width

### 📊 Three States

#### 1. Desktop App Not Running
Shows when the desktop app isn't launched:
- Clear icon and message
- "Refresh" button to check again
- Minimal, uncluttered view

#### 2. Wallet Locked
Shows when desktop app is running but wallet is locked:
- Lock icon indicating security
- Message to unlock desktop app
- Button to open desktop app (optional)

#### 3. Connected (Unlocked)
Full wallet interface showing:
- ✅ Connection status indicator (pulsing green dot)
- 💳 Wallet address (shortened, click to copy)
- 💰 SOL balance and USD value
- 🎯 Quick action buttons (Send, Receive, Activity)
- 🌐 Connected dApps list
- ⚙️ Settings and Lock buttons

## Architecture

### Communication Flow
```
Popup (popup.js)
    ↓ chrome.tabs.sendMessage
Content Script (content.js)
    ↓ window.postMessage
Injected Script (inject.js)
    ↓ checks window.unruggable
Desktop App (via bridge)
```

### How It Works

1. **Popup opens** → Queries current tab
2. **Sends status check** → To content script
3. **Content script** → Posts message to page
4. **Injected script** → Checks wallet state
5. **Returns status** → Back through the chain
6. **Popup updates UI** → Shows appropriate view

## Current Implementation

### ✅ Implemented
- Popup HTML structure
- CSS styling (dark theme, responsive)
- Status checking system
- Three-state view logic (disconnected/locked/connected)
- Copy address functionality
- Periodic status updates (every 5s)

### 🚧 TODOs (Future Enhancements)

#### Balance Fetching
Currently shows `0` - need to:
```javascript
// In inject.js - fetch real balance
async function fetchBalance() {
  const response = await fetch('https://api.mainnet-beta.solana.com', {
    method: 'POST',
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method: 'getBalance',
      params: [publicKey]
    })
  });
  // Update balance...
}
```

#### Connected dApps Tracking
```javascript
// Track when sites connect
const connectedSites = new Map();
// Store: origin, favicon, timestamp
```

#### Desktop App Communication
Options for "Open Desktop App" button:
1. Custom URL scheme: `unruggable://open?view=send`
2. Localhost API: `http://localhost:7777/api/focus`
3. Native messaging (more complex)

#### Lock Wallet
Send lock command through bridge to desktop app

#### Real-time Updates
Use websocket or long-polling for instant balance updates

## Testing

### Test the Popup

1. **Load extension** in Chrome
2. **Click extension icon** in toolbar
3. **Should see** "Desktop App Not Running"
4. **Launch desktop app**
5. **Click refresh** → Should show "Wallet Locked"
6. **Unlock desktop app** with PIN
7. **Refresh popup** → Should show connected view

### Expected Behavior

| Desktop State | Popup Shows |
|--------------|-------------|
| Not running | "Desktop App Not Running" |
| Running, locked | "Wallet Locked" |
| Running, unlocked | Full wallet interface |

## Customization

### Colors
Edit `popup.css`:
```css
/* Primary brand color */
#00ff88 - Green accent

/* Background shades */
#0a0a0a - Body background
#0f0f0f - Header background
#1a1a1a - Card/section background
#2a2a2a - Border color
```

### Layout
Current: 360px x 500px
To change width:
```css
body {
  width: 400px; /* Adjust as needed */
}
```

### Adding Features

To add a new quick action button:
```html
<button class="action-btn" id="swap-btn">
  <svg>...</svg>
  Swap
</button>
```

Then handle in popup.js:
```javascript
document.getElementById('swap-btn')?.addEventListener('click', () => {
  openDesktopApp('swap');
});
```

## Files

- `popup.html` - Structure
- `popup.css` - Styling
- `popup.js` - Logic and communication
- `content.js` - Message relay (updated)
- `inject.js` - Status reporting (updated)
- `manifest.json` - Adds action configuration

## Future Ideas

- Transaction history in popup
- Token list (top 5 holdings)
- Price charts
- Network selector (mainnet/devnet)
- Multiple account support
- QR code generator for receiving
- dApp permissions manager
- Theme customization
- Notifications badge

## Notes

- Popup closes when clicking outside (browser behavior)
- State persists in background but UI reloads each open
- Keep popup logic lightweight for fast loading
- Desktop app remains source of truth for all sensitive operations
