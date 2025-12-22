// Wallet Standard implementation for Unruggable
// This makes the wallet detectable by all Solana dApps

(function() {
  'use strict';

  let attempts = 0;
  const maxAttempts = 50; // Try for 5 seconds

  // Function to register wallet
  function registerWallet() {
    attempts++;

    // Check if Wallet Standard registry is available
    const wallets = window.navigator?.wallets;

    if (!wallets || typeof wallets.get !== 'function') {
      if (attempts < maxAttempts) {
        console.log(`⏳ Waiting for Wallet Standard... (attempt ${attempts}/${maxAttempts})`);
        setTimeout(registerWallet, 100);
      } else {
        console.log('⚠️ Wallet Standard not found, wallet may not appear in some dApps');
      }
      return;
    }

  class UnruggableWalletStandard {
    constructor(adapter) {
      this.adapter = adapter;
      this.version = '1.0.0';
      this.name = 'Unruggable';
      this.icon = 'data:image/png;base64,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';
      this.chains = ['solana:mainnet', 'solana:devnet', 'solana:testnet'];
      this.features = {
        'standard:connect': {
          version: '1.0.0',
          connect: this.connect.bind(this)
        },
        'standard:disconnect': {
          version: '1.0.0',
          disconnect: this.disconnect.bind(this)
        },
        'standard:events': {
          version: '1.0.0',
          on: this.on.bind(this)
        },
        'solana:signAndSendTransaction': {
          version: '1.0.0',
          supportedTransactionVersions: ['legacy', 0],
          signAndSendTransaction: this.signAndSendTransaction.bind(this)
        },
        'solana:signTransaction': {
          version: '1.0.0',
          supportedTransactionVersions: ['legacy', 0],
          signTransaction: this.signTransaction.bind(this)
        },
        'solana:signMessage': {
          version: '1.0.0',
          signMessage: this.signMessage.bind(this)
        }
      };

      this.accounts = [];
    }

    async connect() {
      const result = await this.adapter.connect();

      this.accounts = [{
        address: result.publicKey.toString(),
        publicKey: new Uint8Array(result.publicKey.toBuffer()),
        chains: this.chains,
        features: ['solana:signTransaction', 'solana:signMessage']
      }];

      return { accounts: this.accounts };
    }

    async disconnect() {
      await this.adapter.disconnect();
      this.accounts = [];
    }

    async signTransaction(input) {
      const signedTx = await this.adapter.signTransaction(input.transaction);
      const serialized = signedTx.serialize();
      return {
        signedTransactions: [serialized],
        signedTransaction: serialized  // For compatibility with buggy adapters
      };
    }

    async signAndSendTransaction(input) {
      // For now, just sign - sending is done by the dApp
      const result = await this.signTransaction(input);
      return {
        signature: result.signedTransactions[0]
      };
    }

    async signMessage(input) {
      const result = await this.adapter.signMessage(input.message);
      return {
        signedMessage: result.signature,
        signature: result.signature
      };
    }

    on(event, callback) {
      this.adapter.on(event, callback);
    }
  }

    // Get the base wallet adapter
    const unruggableAdapter = window.unruggable;

    if (!unruggableAdapter) {
      console.error('Unruggable adapter not found');
      return;
    }

    // Create Wallet Standard wallet
    const wallet = new UnruggableWalletStandard(unruggableAdapter);

    // Register with Wallet Standard
    // The standard way is to emit a registration event that dApps listen for
    try {
      // Try multiple registration methods for compatibility

      // Method 1: Dispatch standard wallet registration event
      window.dispatchEvent(
        new CustomEvent('wallet-standard:app-ready', {
          detail: { register: (callback) => callback(wallet) }
        })
      );

      // Method 2: Also announce via window event (some dApps listen for this)
      window.dispatchEvent(
        new CustomEvent('wallet-standard:register-wallet', {
          detail: wallet
        })
      );

      // Method 3: Store in global for dApps that look for it
      if (!window.__unruggableWallet__) {
        window.__unruggableWallet__ = wallet;
      }

      console.log('✅ Unruggable wallet announced to dApps');
    } catch (error) {
      console.error('Failed to announce Unruggable wallet:', error);
    }
  }

  // Start trying to register
  registerWallet();
})();
