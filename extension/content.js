// Content script: bridges between injected script and background worker

// Inject the wallet provider script into the page
const injectScript = document.createElement('script');
injectScript.src = chrome.runtime.getURL('inject.js');
injectScript.onload = function() {
  this.remove();

  // After base wallet is injected, inject Wallet Standard implementation
  const standardScript = document.createElement('script');
  standardScript.src = chrome.runtime.getURL('wallet-standard-impl.js');
  standardScript.onload = function() {
    this.remove();
  };
  (document.head || document.documentElement).appendChild(standardScript);
};
(document.head || document.documentElement).appendChild(injectScript);

// Listen for messages from the injected script
window.addEventListener('message', async (event) => {
  // Only accept messages from the page
  if (event.source !== window) return;
  if (event.data.type !== 'UNRUGGABLE_REQUEST') return;

  const { messageId, request } = event.data;

  try {
    // Forward to background worker which communicates with desktop app
    const response = await chrome.runtime.sendMessage({
      type: 'BRIDGE_REQUEST',
      request
    });

    // Send response back to injected script
    window.postMessage({
      type: 'UNRUGGABLE_RESPONSE',
      messageId,
      response
    }, '*');
  } catch (error) {
    // Send error back to injected script
    window.postMessage({
      type: 'UNRUGGABLE_RESPONSE',
      messageId,
      error: error.message || 'Request failed'
    }, '*');
  }
});

// Listen for messages from popup
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === 'CHECK_DESKTOP_STATUS') {
    // Check if wallet is available and get status
    window.postMessage({
      type: 'UNRUGGABLE_STATUS_CHECK'
    }, '*');

    // Listen for response from injected script
    const handleStatusResponse = (event) => {
      if (event.source !== window) return;
      if (event.data.type !== 'UNRUGGABLE_STATUS_RESPONSE') return;

      window.removeEventListener('message', handleStatusResponse);
      sendResponse(event.data.status);
    };

    window.addEventListener('message', handleStatusResponse);

    // Timeout after 2 seconds
    setTimeout(() => {
      window.removeEventListener('message', handleStatusResponse);
      sendResponse({ connected: false, locked: true });
    }, 2000);

    return true; // Keep channel open for async response
  }

  if (message.type === 'LOCK_WALLET') {
    // Send lock request to desktop app
    window.postMessage({
      type: 'UNRUGGABLE_LOCK_REQUEST'
    }, '*');
    sendResponse({ success: true });
    return true;
  }

  if (message.type === 'DISCONNECT_SITE') {
    // Send disconnect request
    window.postMessage({
      type: 'UNRUGGABLE_DISCONNECT_SITE',
      url: message.url
    }, '*');
    sendResponse({ success: true });
    return true;
  }
});

console.log('Unruggable content script loaded');
