// Proper Wallet Standard Implementation for Unruggable
// Based on https://github.com/wallet-standard/wallet-standard

(function() {
  'use strict';

  // Wait for both window.unruggable and wallet-standard to be ready
  function initialize() {
    const wallet = window.unruggable;

    if (!wallet) {
      console.log('⏳ Waiting for Unruggable wallet...');
      setTimeout(initialize, 50);
      return;
    }

    console.log('📱 Initializing Wallet Standard for Unruggable');

    // Wallet Standard Feature Implementation
    class UnruggableWalletAccount {
      constructor(publicKey) {
        this.address = publicKey;
        this.publicKey = new Uint8Array(bs58Decode(publicKey));
        this.chains = ['solana:mainnet', 'solana:devnet', 'solana:testnet'];
        this.features = ['solana:signAndSendTransaction', 'solana:signTransaction', 'solana:signMessage'];
        this.label = 'Unruggable';
        this.icon = wallet.icon;
      }
    }

    // Main Wallet Standard Wallet Class
    class UnruggableStandardWallet {
      get version() {
        return '1.0.0';
      }

      get name() {
        return 'Unruggable';
      }

      get icon() {
        return wallet.icon;
      }

      get chains() {
        return ['solana:mainnet', 'solana:devnet', 'solana:testnet'];
      }

      #features = null;

      get features() {
        if (this.#features) return this.#features;

        this.#features = {
          'standard:connect': {
            version: '1.0.0',
            connect: async (input) => {
              console.log('🔗 Feature: connect called');
              const result = await this.#connect(input);
              console.log('🔗 Feature: connect result:', result);
              return result;
            }
          },
          'standard:disconnect': {
            version: '1.0.0',
            disconnect: async () => {
              console.log('🔌 Feature: disconnect called');
              await this.#disconnect();
            }
          },
          'standard:events': {
            version: '1.0.0',
            on: (event, callback) => {
              console.log('👂 Feature: events.on called');
              return this.#on(event, callback);
            }
          },
          'solana:signAndSendTransaction': {
            version: '1.0.0',
            supportedTransactionVersions: ['legacy', 0],
            signAndSendTransaction: async (input) => {
              console.log('📤 Feature: signAndSendTransaction called');
              console.log('📤 Input chain:', input.chain);
              const result = await this.#signAndSendTransaction(input);
              console.log('📤 Feature: signAndSendTransaction result:', result);
              console.log('📤 Signature bytes:', result?.signature?.length, 'bytes');
              return result;
            }
          },
          'solana:signTransaction': {
            version: '1.0.0',
            supportedTransactionVersions: ['legacy', 0],
            signTransaction: async (input) => {
              console.log('✍️ Feature: signTransaction called');
              const result = await this.#signTransaction(input);
              console.log('✍️ Feature: signTransaction returning:', result);
              console.log('✍️ Feature: result.signedTransactions exists?', !!result?.signedTransactions);
              console.log('✍️ Feature: result.signedTransactions[0] exists?', !!result?.signedTransactions?.[0]);
              console.log('✍️ Feature: result.signedTransaction exists?', !!result?.signedTransaction);
              return result;
            }
          },
          'solana:signMessage': {
            version: '1.0.0',
            signMessage: async (input) => {
              console.log('💬 Feature: signMessage called');
              const result = await this.#signMessage(input);
              console.log('💬 Feature: signMessage result:', result);
              return result;
            }
          }
        };

        return this.#features;
      }

      get accounts() {
        return this.#accounts;
      }

      #accounts = [];
      #listeners = new Map();

      async #connect(input) {
        try {
          const result = await wallet.connect();

          const account = new UnruggableWalletAccount(result.publicKey.toString());
          this.#accounts = [account];

          this.#emit('change', { accounts: this.#accounts });

          return { accounts: this.#accounts };
        } catch (error) {
          throw new Error(`Connection failed: ${error.message}`);
        }
      }

      async #disconnect() {
        try {
          await wallet.disconnect();
          this.#accounts = [];
          this.#emit('change', { accounts: [] });
        } catch (error) {
          throw new Error(`Disconnect failed: ${error.message}`);
        }
      }

      #on(event, callback) {
        if (!this.#listeners.has(event)) {
          this.#listeners.set(event, []);
        }
        this.#listeners.get(event).push(callback);
        return () => {
          const listeners = this.#listeners.get(event);
          const index = listeners?.indexOf(callback);
          if (index !== undefined && index >= 0) {
            listeners.splice(index, 1);
          }
        };
      }

      #emit(event, data) {
        const listeners = this.#listeners.get(event);
        if (listeners) {
          listeners.forEach(callback => {
            try {
              callback(data);
            } catch (error) {
              console.error('Error in wallet event listener:', error);
            }
          });
        }
      }

      async #signTransaction(input) {
        try {
          console.log('🔏 Wallet Standard: Signing transaction');
          console.log('🎯 IMPORTANT: Desktop app will SIGN and SEND this transaction!');
          console.log('📥 Input:', {
            transaction: input.transaction,
            txLength: input.transaction?.length,
            firstByte: input.transaction?.[0],
            account: input.account,
            chain: input.chain
          });

          // input.transaction is a Uint8Array of the serialized transaction
          // Convert to base58 to send to desktop app
          const txBase58 = bs58Encode(input.transaction);
          console.log('📝 Base58 transaction:', txBase58.substring(0, 40) + '...');

          // Send to desktop app for signing
          console.log('📤 Sending to desktop app...');
          const response = await this._sendToDesktop({
            method: 'SignTransaction',
            transaction: txBase58,
            origin: window.location.origin
          });

          console.log('📨 Response from desktop:', response);

          if (response.type === 'TransactionSigned') {
            console.log('✅ Transaction signed AND SENT by desktop app');
            console.log('🔗 On-chain signature:', response.signature);
            console.log('📦 Signed transaction received');

            // Desktop app has already signed AND sent the transaction
            // It returns the full signed transaction as base58
            const signedTx = bs58Decode(response.signed_transaction);
            console.log('📦 Decoded signed transaction length:', signedTx.length);
            console.log('📦 First 10 bytes:', Array.from(signedTx.slice(0, 10)));

            // Wallet Standard spec requires signedTransactions (plural) as an array
            // But some adapters (like Jupiter) incorrectly expect signedTransaction (singular)
            // Return both for compatibility
            const result = {
              signedTransactions: [signedTx],
              signedTransaction: signedTx  // For buggy adapters
            };
            console.log('✅ Returning signed transaction to dApp');
            console.log('🎉 Transaction already on-chain with signature:', response.signature);

            return result;
          } else if (response.type === 'Rejected') {
            console.log('❌ Transaction rejected by user');
            throw new Error('Transaction rejected: ' + response.reason);
          } else {
            console.log('❌ Unknown response type:', response.type);
            throw new Error(response.message || 'Signing failed');
          }
        } catch (error) {
          console.error('❌ Transaction signing error:', error);
          console.error('❌ Error stack:', error.stack);
          throw new Error(`Transaction signing failed: ${error.message}`);
        }
      }

      async _sendToDesktop(request) {
        // Use the base wallet's message sending
        return new Promise((resolve, reject) => {
          const messageId = Math.random().toString(36).substring(7);
          console.log('📮 _sendToDesktop called with messageId:', messageId);
          console.log('📮 Request:', request);

          const handleResponse = (event) => {
            if (event.source !== window) return;
            if (event.data.type !== 'UNRUGGABLE_RESPONSE') return;
            if (event.data.messageId !== messageId) return;

            console.log('📬 Received response for messageId:', messageId);
            console.log('📬 Event data:', event.data);

            window.removeEventListener('message', handleResponse);

            if (event.data.error) {
              console.error('📬 Error in response:', event.data.error);
              reject(new Error(event.data.error));
            } else {
              console.log('📬 Resolving with response:', event.data.response);
              resolve(event.data.response);
            }
          };

          window.addEventListener('message', handleResponse);

          window.postMessage({
            type: 'UNRUGGABLE_REQUEST',
            messageId,
            request
          }, '*');

          setTimeout(() => {
            console.error('⏰ Request timeout for messageId:', messageId);
            window.removeEventListener('message', handleResponse);
            reject(new Error('Request timeout'));
          }, 60000);
        });
      }

      async #signAndSendTransaction(input) {
        try {
          console.log('📤 Wallet Standard: Sign and send transaction');
          console.log('📤 Note: Desktop app handles both signing AND sending');

          // The #signTransaction method already signs AND sends via desktop app
          // So we just need to call it and extract the signature
          const signResult = await this.#signTransaction(input);

          console.log('✅ Transaction was signed and sent by desktop app');

          // Extract the signature from the signed transaction
          // Signature is at bytes 1-65 (after the signature count byte)
          const signedTx = signResult.signedTransaction;
          const signatureBytes = signedTx.slice(1, 65);

          console.log('📤 Returning signature:', bs58Encode(signatureBytes));

          return {
            signature: signatureBytes
          };
        } catch (error) {
          console.error('❌ signAndSendTransaction error:', error);
          throw new Error(`Sign and send failed: ${error.message}`);
        }
      }

      async #signMessage(input) {
        try {
          const result = await wallet.signMessage(input.message);

          return {
            signedMessage: input.message,
            signature: result.signature
          };
        } catch (error) {
          throw new Error(`Message signing failed: ${error.message}`);
        }
      }
    }

    // Helper: Base58 decode
    function bs58Decode(str) {
      const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
      let num = 0n;
      for (let i = 0; i < str.length; i++) {
        const digit = alphabet.indexOf(str[i]);
        if (digit < 0) throw new Error('Invalid base58 character');
        num = num * 58n + BigInt(digit);
      }

      const bytes = [];
      while (num > 0n) {
        bytes.unshift(Number(num % 256n));
        num = num / 256n;
      }

      for (let i = 0; i < str.length && str[i] === alphabet[0]; i++) {
        bytes.unshift(0);
      }

      return new Uint8Array(bytes);
    }

    // Helper: Base58 encode
    function bs58Encode(buffer) {
      const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
      const bytes = new Uint8Array(buffer);

      let num = 0n;
      for (let i = 0; i < bytes.length; i++) {
        num = num * 256n + BigInt(bytes[i]);
      }

      let encoded = '';
      while (num > 0n) {
        const remainder = num % 58n;
        num = num / 58n;
        encoded = alphabet[Number(remainder)] + encoded;
      }

      for (let i = 0; i < bytes.length && bytes[i] === 0; i++) {
        encoded = alphabet[0] + encoded;
      }

      return encoded;
    }

    // Create the standard wallet instance
    const standardWallet = new UnruggableStandardWallet();

    // Register with Wallet Standard
    // The proper way is to use the global register callback
    function registerWallet() {
      try {
        // Method 1: Use the window.__registerWallet__ callback if available
        if (typeof window.__registerWallet__ === 'function') {
          window.__registerWallet__(standardWallet);
          console.log('✅ Registered via __registerWallet__ callback');
          return true;
        }

        // Method 2: Dispatch wallet-standard:register-wallet event
        const event = new CustomEvent('wallet-standard:register-wallet', {
          detail: standardWallet
        });
        window.dispatchEvent(event);
        console.log('✅ Dispatched wallet-standard:register-wallet event');

        return true;
      } catch (error) {
        console.error('Failed to register wallet:', error);
        return false;
      }
    }

    // Register immediately and also listen for app-ready events from dApps
    registerWallet();

    // Listen for dApps requesting wallet registration
    window.addEventListener('wallet-standard:app-ready', (event) => {
      console.log('📱 dApp requested wallet registration');
      if (event.detail && typeof event.detail.register === 'function') {
        try {
          event.detail.register(standardWallet);
          console.log('✅ Registered wallet with dApp');
        } catch (error) {
          console.error('Failed to register with dApp:', error);
        }
      }
    });

    console.log('✅ Unruggable Wallet Standard implementation complete');
  }

  // Start initialization
  initialize();
})();
