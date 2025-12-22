# Transaction Signing and Sending Flow

## Overview
The desktop app now handles BOTH signing AND sending transactions to ensure they land on-chain.

## Flow

### 1. dApp initiates transaction
- dApp calls `signTransaction()` or `signAndSendTransaction()` on the wallet

### 2. Extension receives request
- Extension receives transaction as `Uint8Array`
- Encodes it to base58
- Sends to desktop app via bridge

### 3. Desktop app processes transaction
**Location: `src/bridge/handler.rs`**

The desktop app:
1. Receives base58-encoded transaction
2. Decodes to bytes
3. **Signs the transaction message** (extracts message portion and signs it)
4. **Inserts signature into transaction** at bytes 1-65
5. **Sends to Solana RPC** using the configured RPC endpoint
6. **Returns both**:
   - On-chain transaction signature (from Solana)
   - Full signed transaction (base58 encoded)

### 4. Extension returns to dApp
- Extension receives signed transaction from desktop
- Decodes from base58 to `Uint8Array`
- Returns to dApp as `signedTransaction` / `signedTransactions`

**Note**: Transaction is ALREADY on-chain at this point!

## Key Changes

### Desktop App (`src/bridge/handler.rs`)
```rust
// Now signs AND sends
BridgeResponse::TransactionSigned {
    signature: sig_string,           // On-chain signature
    signed_transaction: signed_tx_base58,  // Full signed tx
}
```

### Extension (`extension/wallet-standard-impl.js`)
```javascript
// Desktop handles everything
const signedTx = bs58Decode(response.signed_transaction);
return {
  signedTransactions: [signedTx],
  signedTransaction: signedTx
};
```

## Benefits

1. **Guaranteed on-chain**: Desktop app confirms transaction was sent
2. **User approval**: Desktop UI shows approval dialog
3. **Reliable RPC**: Uses configured RPC from desktop settings
4. **Better error handling**: Desktop can show detailed errors
5. **No duplicate sends**: Extension doesn't try to re-send

## Logs to Watch

### Desktop Terminal:
```
✍️  Bridge: Sign and send transaction request
✅ Bridge: Transaction signed with signature: ...
✅ Bridge: Signature inserted into transaction
🌐 Bridge: Using RPC: https://api.mainnet-beta.solana.com
✅ Bridge: Transaction sent successfully!
🔗 On-chain Signature: [signature]
```

### Browser Console:
```
🔏 Wallet Standard: Signing transaction
🎯 IMPORTANT: Desktop app will SIGN and SEND this transaction!
✅ Transaction signed AND SENT by desktop app
🔗 On-chain signature: [signature]
🎉 Transaction already on-chain with signature: [signature]
```
