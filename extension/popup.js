// Popup script for Unruggable Wallet extension
(function() {
  'use strict';

  // DOM elements
  const statusIndicator = document.getElementById('status-indicator');
  const statusText = document.getElementById('status-text');
  const notConnectedView = document.getElementById('not-connected-view');
  const lockedView = document.getElementById('locked-view');
  const connectedView = document.getElementById('connected-view');
  const walletAddress = document.getElementById('wallet-address');
  const solBalance = document.getElementById('sol-balance');
  const usdBalance = document.getElementById('usd-balance');
  const connectedSites = document.getElementById('connected-sites');
  const connectedCount = document.getElementById('connected-count');

  // State
  let currentState = {
    connected: false,
    locked: true,
    address: null,
    balance: 0,
    connectedDapps: []
  };

  // Initialize
  async function init() {
    console.log('🎨 Popup: Initializing');

    // Check desktop app status
    await checkDesktopStatus();

    // Set up event listeners
    setupEventListeners();

    // Update UI periodically
    setInterval(checkDesktopStatus, 5000);
  }

  // Check if desktop app is running and unlocked
  async function checkDesktopStatus() {
    try {
      // Send message directly to background script (not through content script)
      const response = await chrome.runtime.sendMessage({
        type: 'CHECK_DESKTOP_STATUS'
      }).catch(error => {
        console.error('❌ Failed to communicate with background script:', error);
        return null;
      });

      if (!response) {
        showNotConnected();
        return;
      }

      if (response.connected && !response.locked) {
        // Desktop is running and unlocked
        currentState = {
          connected: true,
          locked: false,
          address: response.publicKey,
          balance: response.balance || 0,
          connectedDapps: response.connectedDapps || []
        };
        showConnected();
      } else if (response.connected && response.locked) {
        // Desktop is running but locked
        showLocked();
      } else {
        showNotConnected();
      }
    } catch (error) {
      console.error('❌ Error checking desktop status:', error);
      showNotConnected();
    }
  }

  // Show views
  function showNotConnected() {
    statusIndicator.className = 'status-indicator disconnected';
    statusText.textContent = 'Disconnected';
    notConnectedView.classList.remove('hidden');
    lockedView.classList.add('hidden');
    connectedView.classList.add('hidden');
  }

  function showLocked() {
    statusIndicator.className = 'status-indicator disconnected';
    statusText.textContent = 'Locked';
    notConnectedView.classList.add('hidden');
    lockedView.classList.remove('hidden');
    connectedView.classList.add('hidden');
  }

  function showConnected() {
    statusIndicator.className = 'status-indicator connected';
    statusText.textContent = 'Connected';
    notConnectedView.classList.add('hidden');
    lockedView.classList.add('hidden');
    connectedView.classList.remove('hidden');

    // Update wallet info
    updateWalletInfo();
    updateConnectedSites();
  }

  function updateWalletInfo() {
    if (currentState.address) {
      // Show shortened address
      const addr = currentState.address;
      const shortened = `${addr.slice(0, 4)}...${addr.slice(-4)}`;
      walletAddress.textContent = shortened;
      walletAddress.title = addr;
    }

    // Update balance
    solBalance.textContent = currentState.balance.toFixed(4);

    // Calculate USD value (assuming SOL price - in real app, fetch from API)
    const solPrice = 100; // TODO: Fetch real price
    const usdValue = currentState.balance * solPrice;
    usdBalance.textContent = `$${usdValue.toFixed(2)}`;
  }

  function updateConnectedSites() {
    const sites = currentState.connectedDapps;
    connectedCount.textContent = sites.length.toString();

    if (sites.length === 0) {
      connectedSites.innerHTML = '<p class="empty-text">No connected sites</p>';
      return;
    }

    connectedSites.innerHTML = sites.map(site => `
      <div class="site-item">
        <img class="site-favicon" src="${site.favicon || 'icons/icon16.png'}" alt="${site.name}">
        <div class="site-info">
          <div class="site-name">${site.name}</div>
          <div class="site-url">${site.url}</div>
        </div>
        <button class="icon-btn" onclick="disconnectSite('${site.url}')" title="Disconnect">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>
    `).join('');
  }

  // Event listeners
  function setupEventListeners() {
    // Refresh button
    document.getElementById('refresh-btn')?.addEventListener('click', () => {
      checkDesktopStatus();
    });

    // Copy address
    document.getElementById('copy-address-btn')?.addEventListener('click', () => {
      if (currentState.address) {
        copyToClipboard(currentState.address);
      }
    });

    // Quick actions
    document.getElementById('send-btn')?.addEventListener('click', () => {
      openDesktopApp('send');
    });

    document.getElementById('receive-btn')?.addEventListener('click', () => {
      openDesktopApp('receive');
    });

    document.getElementById('activity-btn')?.addEventListener('click', () => {
      openDesktopApp('activity');
    });

    // Footer actions
    document.getElementById('settings-btn')?.addEventListener('click', () => {
      openDesktopApp('settings');
    });

    document.getElementById('lock-btn')?.addEventListener('click', () => {
      lockWallet();
    });

    document.getElementById('open-desktop-btn')?.addEventListener('click', () => {
      openDesktopApp();
    });
  }

  // Copy to clipboard with toast
  function copyToClipboard(text) {
    navigator.clipboard.writeText(text).then(() => {
      showToast('Address copied!');
    });
  }

  function showToast(message) {
    const toast = document.createElement('div');
    toast.className = 'copied-toast';
    toast.textContent = message;
    document.body.appendChild(toast);

    setTimeout(() => toast.classList.add('show'), 10);
    setTimeout(() => {
      toast.classList.remove('show');
      setTimeout(() => toast.remove(), 200);
    }, 2000);
  }

  // Open desktop app (this is a placeholder - actual implementation depends on your setup)
  function openDesktopApp(view = null) {
    console.log('🖥️  Opening desktop app', view);

    // Option 1: If you have a custom URL scheme (e.g., unruggable://)
    // window.open(`unruggable://${view || 'home'}`);

    // Option 2: Show a message to user
    showToast('Please open your Unruggable desktop app');

    // Option 3: Send message to desktop via localhost
    // fetch('http://localhost:7777/open', { method: 'POST', body: JSON.stringify({ view }) });
  }

  // Lock wallet
  async function lockWallet() {
    try {
      await chrome.runtime.sendMessage({
        type: 'LOCK_WALLET'
      });
      checkDesktopStatus();
    } catch (error) {
      console.error('❌ Error locking wallet:', error);
    }
  }

  // Disconnect site
  async function disconnectSite(url) {
    try {
      await chrome.runtime.sendMessage({
        type: 'DISCONNECT_SITE',
        url: url
      });
      // Refresh connected sites
      setTimeout(checkDesktopStatus, 500);
    } catch (error) {
      console.error('❌ Error disconnecting site:', error);
    }
  }

  // Make disconnectSite available globally for onclick handlers
  window.disconnectSite = disconnectSite;

  // Start
  init();
})();
