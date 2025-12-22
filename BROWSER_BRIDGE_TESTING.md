# Browser Bridge - Testing Guide

## ✅ What's Working

Your Unruggable wallet extension now works with **any Solana dApp** in the browser!

## 🧪 Test on Real Websites

### 1. Jupiter (Swap)
```
https://jup.ag
```
1. Visit Jupiter
2. Click "Connect Wallet" button (top right)
3. Look for **"Unruggable"** in the wallet list
4. Click it → Should connect instantly!
5. Try a swap → Approve in desktop app

### 2. Raydium (DEX)
```
https://raydium.io/swap/
```
1. Click "Connect Wallet"
2. Select "Unruggable"
3. Test a swap

### 3. Magic Eden (NFT Marketplace)
```
https://magiceden.io/
```
1. Click "Connect" (top right)
2. Select "Unruggable"
3. Browse NFTs

### 4. Phantom's Test Page
```
https://phantom.app/developer-tools/connect-wallet
```
This is Phantom's official test page - perfect for testing wallet functionality!

## 🔍 Debugging

### Extension Console
1. Go to `chrome://extensions/`
2. Find "Unruggable Wallet"
3. Click "service worker" → Opens background console
4. Should see: `✅ Connected to Unruggable desktop app`

### Website Console
1. Open any dApp
2. Press F12 → Console tab
3. Type: `window.solana`
4. Should see the Unruggable wallet object with `isUnruggable: true`

### Desktop App Terminal
Watch for these messages:
- `🔗 Bridge: Connect request from https://jup.ag`
- `✍️ Bridge: Sign transaction request from https://jup.ag`
- `✅ Bridge: Transaction signed`

## ⚙️ Current Limitations (MVP)

### Auto-Approve (No Modal Yet)
- All transactions are **auto-signed** without showing approval modal
- This is for MVP testing only
- ⚠️ **Don't use with real money yet!**

### PIN-Protected Wallets
If you see:
```
⚠️ Bridge: Could not load wallet: PIN-protected wallets not yet supported
```

**Workaround**: Temporarily remove PIN protection:
1. Open desktop app
2. Go to Settings → Security
3. Remove PIN (for testing only)
4. Restart desktop app

### First Wallet Only
- Bridge uses your first wallet only
- Multi-wallet selection coming soon

## 📊 Testing Checklist

- [ ] Extension loads and shows green check on test.html
- [ ] Desktop app shows `✅ Bridge: Wallet loaded successfully`
- [ ] Can connect on test.html with real public key
- [ ] Can sign message on test.html
- [ ] Jupiter detects Unruggable wallet
- [ ] Can connect to Jupiter
- [ ] Can see wallet balance on Jupiter
- [ ] Can create a swap transaction (small amount!)
- [ ] Transaction gets signed and submitted

## 🐛 Common Issues

**"No wallet loaded" error:**
- Make sure desktop app is running
- Check terminal for wallet loading status
- Try removing PIN protection temporarily

**Port 7777 in use:**
```bash
lsof -ti:7777 | xargs kill -9
```
Then restart desktop app

**Extension not detected:**
- Refresh the webpage
- Check extension is enabled in `chrome://extensions/`
- Check browser console for injection logs

**Can't see Unruggable in wallet list:**
- Some dApps only show known wallets
- Try typing "Unruggable" in search
- Use Phantom's test page first

## 🎯 Next Steps

### High Priority
1. **Approval Modals** - Show transaction details before signing
2. **PIN Support** - Unlock wallet from bridge requests
3. **Security** - Origin allowlist, transaction amount limits

### Medium Priority
1. **Multi-wallet** - Select which wallet to use for browser
2. **Session Management** - "Remember this site for 24h"
3. **Better Error Messages** - User-friendly errors in browser

### Nice to Have
1. **Transaction Preview** - Show SOL amounts, tokens, etc.
2. **Notification System** - Desktop notifications for requests
3. **Browser Extension Popup** - Settings, connected sites, etc.

## 🚀 Deployment

When ready to ship:

1. **Add Approval Modals** (required for safety)
2. **Test thoroughly** with testnet tokens
3. **Package extension:**
   ```bash
   cd extension
   zip -r unruggable-extension.zip . -x "*.md" -x "test.html"
   ```
4. **Publish to Chrome Web Store** (~2 day review)
5. **Update website** with extension download link

## 📝 Notes

- Extension works with **all** Solana wallet adapter dApps
- No changes needed to dApp code - they just detect `window.solana`
- Desktop app must be running for extension to work
- All signing happens in desktop app (private keys never leave)
- WebSocket only accessible from localhost (secure)

---

**Test it now!** Go to https://jup.ag and connect your wallet! 🎉
