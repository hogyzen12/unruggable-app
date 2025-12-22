// This script is injected into the page context to provide window.solana
(function() {
  'use strict';

  // Don't inject if already present
  if (window.solana?.isUnruggable) {
    console.log('Unruggable wallet already injected');
    return;
  }

  class UnruggableWallet {
    constructor() {
      this.name = 'Unruggable';
      this.icon = 'data:image/png;base64,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';
      this.isUnruggable = true;
      this.isPhantom = false; // Not Phantom
      this.isConnected = false;
      this.publicKey = null;
      this._listeners = new Map();

      // For compatibility with Wallet Adapter
      this._readyState = 'Installed';
    }

    // Connect to the wallet
    async connect() {
      try {
        const response = await this._sendMessage({
          method: 'Connect',
          origin: window.location.origin
        });

        if (response.type === 'Connected') {
          this.isConnected = true;
          this.publicKey = {
            toString: () => response.public_key,
            toBase58: () => response.public_key,
            toBuffer: () => {
              // Convert base58 to buffer
              const bytes = window.solanaWeb3?.PublicKey.decode(response.public_key);
              return bytes || new Uint8Array();
            }
          };
          this._emit('connect', this.publicKey);
          return { publicKey: this.publicKey };
        } else if (response.type === 'Rejected') {
          throw new Error('Connection rejected: ' + response.reason);
        } else {
          throw new Error(response.message || 'Connection failed');
        }
      } catch (error) {
        console.error('Unruggable connect error:', error);
        throw error;
      }
    }

    // Disconnect from the wallet
    async disconnect() {
      try {
        await this._sendMessage({
          method: 'Disconnect',
          origin: window.location.origin
        });

        this.isConnected = false;
        this.publicKey = null;
        this._emit('disconnect');
      } catch (error) {
        console.error('Unruggable disconnect error:', error);
      }
    }

    // Sign a transaction
    async signTransaction(transaction) {
      if (!this.isConnected) {
        throw new Error('Wallet not connected');
      }

      try {
        // Serialize transaction to base58
        const serialized = transaction.serialize({
          requireAllSignatures: false,
          verifySignatures: false
        });
        const base58Tx = this._encodeBase58(serialized);

        const response = await this._sendMessage({
          method: 'SignTransaction',
          transaction: base58Tx,
          origin: window.location.origin
        });

        if (response.type === 'TransactionSigned') {
          // Decode signature and add to transaction
          const signatureBytes = this._decodeBase58(response.signature);
          transaction.addSignature(this.publicKey, signatureBytes);
          return transaction;
        } else if (response.type === 'Rejected') {
          throw new Error('Transaction rejected: ' + response.reason);
        } else {
          throw new Error(response.message || 'Signing failed');
        }
      } catch (error) {
        console.error('Unruggable sign transaction error:', error);
        throw error;
      }
    }

    // Sign all transactions
    async signAllTransactions(transactions) {
      const signed = [];
      for (const tx of transactions) {
        signed.push(await this.signTransaction(tx));
      }
      return signed;
    }

    // Sign a message
    async signMessage(message) {
      if (!this.isConnected) {
        throw new Error('Wallet not connected');
      }

      try {
        const base58Message = this._encodeBase58(message);

        const response = await this._sendMessage({
          method: 'SignMessage',
          message: base58Message,
          origin: window.location.origin
        });

        if (response.type === 'MessageSigned') {
          return {
            signature: this._decodeBase58(response.signature),
            publicKey: this.publicKey
          };
        } else if (response.type === 'Rejected') {
          throw new Error('Message signing rejected: ' + response.reason);
        } else {
          throw new Error(response.message || 'Signing failed');
        }
      } catch (error) {
        console.error('Unruggable sign message error:', error);
        throw error;
      }
    }

    // Event listeners
    on(event, handler) {
      if (!this._listeners.has(event)) {
        this._listeners.set(event, []);
      }
      this._listeners.get(event).push(handler);
    }

    off(event, handler) {
      if (!this._listeners.has(event)) return;
      const handlers = this._listeners.get(event);
      const index = handlers.indexOf(handler);
      if (index !== -1) {
        handlers.splice(index, 1);
      }
    }

    _emit(event, data) {
      if (!this._listeners.has(event)) return;
      const handlers = this._listeners.get(event);
      handlers.forEach(handler => {
        try {
          handler(data);
        } catch (err) {
          console.error('Error in event handler:', err);
        }
      });
    }

    // Send message to content script
    async _sendMessage(request) {
      return new Promise((resolve, reject) => {
        const messageId = Math.random().toString(36).substring(7);

        const handleResponse = (event) => {
          if (event.source !== window) return;
          if (event.data.type !== 'UNRUGGABLE_RESPONSE') return;
          if (event.data.messageId !== messageId) return;

          window.removeEventListener('message', handleResponse);

          if (event.data.error) {
            reject(new Error(event.data.error));
          } else {
            resolve(event.data.response);
          }
        };

        window.addEventListener('message', handleResponse);

        // Send to content script
        window.postMessage({
          type: 'UNRUGGABLE_REQUEST',
          messageId,
          request
        }, '*');

        // Timeout after 60 seconds
        setTimeout(() => {
          window.removeEventListener('message', handleResponse);
          reject(new Error('Request timeout'));
        }, 60000);
      });
    }

    // Base58 encoding/decoding helpers
    _encodeBase58(buffer) {
      const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
      const bytes = new Uint8Array(buffer);

      let num = BigInt(0);
      for (let i = 0; i < bytes.length; i++) {
        num = num * BigInt(256) + BigInt(bytes[i]);
      }

      let encoded = '';
      while (num > 0) {
        const remainder = num % BigInt(58);
        num = num / BigInt(58);
        encoded = alphabet[Number(remainder)] + encoded;
      }

      // Add leading zeros
      for (let i = 0; i < bytes.length && bytes[i] === 0; i++) {
        encoded = alphabet[0] + encoded;
      }

      return encoded;
    }

    _decodeBase58(str) {
      const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
      let num = BigInt(0);

      for (let i = 0; i < str.length; i++) {
        const digit = alphabet.indexOf(str[i]);
        if (digit < 0) throw new Error('Invalid base58 character');
        num = num * BigInt(58) + BigInt(digit);
      }

      const bytes = [];
      while (num > 0) {
        bytes.unshift(Number(num % BigInt(256)));
        num = num / BigInt(256);
      }

      // Add leading zeros
      for (let i = 0; i < str.length && str[i] === alphabet[0]; i++) {
        bytes.unshift(0);
      }

      return new Uint8Array(bytes);
    }
  }

  // Create and expose the wallet
  const unruggableWallet = new UnruggableWallet();

  // Try to set as window.solana if not already taken
  // (for backwards compatibility with older dApps)
  if (!window.solana) {
    try {
      Object.defineProperty(window, 'solana', {
        value: unruggableWallet,
        writable: false,
        configurable: false
      });
      console.log('🛡️ Unruggable Wallet set as window.solana');
    } catch (error) {
      console.log('ℹ️ window.solana already claimed by another wallet');
    }
  } else {
    console.log('ℹ️ window.solana already exists:', window.solana?.name || 'unknown wallet');
  }

  // Always expose under unruggable name (this is our primary namespace)
  try {
    Object.defineProperty(window, 'unruggable', {
      value: unruggableWallet,
      writable: false,
      configurable: false
    });
    console.log('🛡️ Unruggable Wallet available at window.unruggable');
  } catch (error) {
    console.error('Failed to set window.unruggable:', error);
  }

  // Emit ready event
  window.dispatchEvent(new Event('unruggable#initialized'));

  // Listen for status checks from popup
  window.addEventListener('message', (event) => {
    if (event.source !== window) return;

    if (event.data.type === 'UNRUGGABLE_STATUS_CHECK') {
      // Return current status
      window.postMessage({
        type: 'UNRUGGABLE_STATUS_RESPONSE',
        status: {
          connected: unruggableWallet.isConnected,
          locked: !unruggableWallet.isConnected,
          publicKey: unruggableWallet.publicKey?.toString() || null,
          balance: 0, // TODO: Fetch real balance
          connectedDapps: [] // TODO: Track connected dApps
        }
      }, '*');
    }
  });
})();
