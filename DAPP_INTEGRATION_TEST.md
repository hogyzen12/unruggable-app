# dApp Integration Testing Guide

## Current Implementation Status

Your wallet now supports:

### ✅ Wallet Standard (Modern dApps)
- `standard:connect` - Wallet connection
- `standard:disconnect` - Disconnect wallet
- `standard:events` - Event listening
- `solana:signTransaction` - Sign (and send) transactions
- `solana:signAndSendTransaction` - Sign and send
- `solana:signMessage` - Sign messages (for authentication)

### ✅ Legacy Support
- `window.solana` - For older dApps
- `window.unruggable` - Primary namespace

## Test Scenarios

### 1. Test Wallet Connection

**Sites to test:**
- https://jup.ag (Jupiter)
- https://titan.exchange (Titan)
- https://dexscreener.com (wallet selector)
- https://phantom.app/learn/crypto-101/sign-in-with-solana (SIWS demo)

**What to test:**
1. Click "Connect Wallet"
2. Look for "Unruggable" in the wallet list
3. Desktop app should show approval dialog
4. dApp should show your public key after approval

**Expected logs (desktop):**
```
🔗 Bridge: Connect request from https://...
✅ Bridge: Connected with pubkey [your-pubkey]
```

**Expected logs (browser console):**
```
🔗 Feature: connect called
✅ Connected with pubkey: [your-pubkey]
```

---

### 2. Test Sign-In with Solana (SIWS)

**What this does:** Many dApps use message signing for authentication instead of just connecting.

**Test sites:**
- Any site with "Sign in with Solana" button
- Solana wallet adapters often use this

**What to test:**
1. Click "Sign In" or "Verify Ownership"
2. Desktop app should show a message signing dialog
3. Message usually says something like "Sign this message to authenticate"

**Expected logs (desktop):**
```
✍️  Bridge: Sign message request
✅ Bridge: Message signed
```

**Expected logs (browser console):**
```
💬 Feature: signMessage called
✅ Message signed
```

---

### 3. Test Transactions

**Already working!** ✅

**Test sites:**
- https://jup.ag (swaps)
- https://titan.exchange (trading)

**Expected behavior:**
- Desktop app signs AND sends transaction
- Transaction appears on-chain
- dApp shows success

---

## Common Issues and Fixes

### Issue: Wallet not appearing in dApp selector

**Check:**
1. Extension is loaded and enabled
2. Open console and look for: `✅ Unruggable Wallet available`
3. Run in console: `window.unruggable` - should show wallet object

**Fix:**
- Reload the page
- Reload the extension
- Check that `manifest.json` has correct permissions

---

### Issue: "Please unlock your desktop app first"

**Check:**
1. Desktop app is running
2. You've entered your PIN and unlocked the wallet

**Fix:**
- Unlock the desktop app with your PIN
- Try connecting again

---

### Issue: Connection works but signing fails

**Check:**
1. Desktop app is still running
2. Check desktop terminal for errors
3. Check browser console for bridge errors

**Fix:**
- Make sure desktop app is in foreground
- Check that content script is injected (look for bridge logs)

---

## Debugging Commands

### In Browser Console:

```javascript
// Check if wallet is available
window.unruggable

// Check if connected
window.unruggable.isConnected

// Check public key
window.unruggable.publicKey?.toString()

// Test sign message manually
window.unruggable.signMessage(new TextEncoder().encode("Hello Solana"))

// Check Wallet Standard registration
window.addEventListener('wallet-standard:register-wallet', (e) => {
  console.log('Wallet Standard registered:', e.detail);
});
```

---

## What Makes Integration "Seamless"

Your wallet now provides:

1. **Auto-discovery** - Wallet Standard makes it appear in all modern dApps
2. **Legacy support** - `window.solana` for older dApps
3. **Desktop approval** - All actions go through your desktop app (secure!)
4. **Message signing** - Full "Sign in with Solana" support
5. **Transaction sending** - Desktop app handles everything including RPC
6. **Error handling** - Clear error messages in both desktop and browser

---

## Next Steps

Try these popular dApps to test:

1. **Jupiter** (https://jup.ag)
   - Test: Connect → Swap tokens
   - Expected: Desktop shows approval, tx lands on-chain

2. **Magic Eden** (https://magiceden.io)
   - Test: Connect → View your NFTs
   - Expected: Connection works, shows your assets

3. **Phantom Learn** (https://phantom.app/learn/crypto-101/sign-in-with-solana)
   - Test: "Sign In with Solana" button
   - Expected: Desktop shows message to sign

4. **Solana Wallet Adapter Demo** (https://solana-labs.github.io/wallet-adapter/example/)
   - Test: Select "Unruggable" from dropdown
   - Expected: Appears in list, connects properly

---

## Report Issues

If something doesn't work:
1. Check browser console for errors
2. Check desktop terminal for errors
3. Note which dApp and what action failed
4. Share the error messages
