// Background service worker: communicates with desktop app via WebSocket

const DESKTOP_WS_URL = 'ws://localhost:7777';
let ws = null;
let isConnecting = false;
let reconnectTimeout = null;
const pendingRequests = new Map();

// Initialize WebSocket connection to desktop app
function connectToDesktop() {
  if (ws?.readyState === WebSocket.OPEN || isConnecting) {
    return;
  }

  isConnecting = true;
  console.log('Connecting to Unruggable desktop app...');

  try {
    ws = new WebSocket(DESKTOP_WS_URL);

    ws.onopen = () => {
      console.log('✅ Connected to Unruggable desktop app');
      isConnecting = false;

      // Send ping to verify connection
      sendToDesktop({ method: 'Ping' });
    };

    ws.onmessage = (event) => {
      try {
        const response = JSON.parse(event.data);
        handleDesktopResponse(response);
      } catch (error) {
        console.error('Failed to parse desktop response:', error);
      }
    };

    ws.onerror = (error) => {
      console.error('WebSocket error:', error);
      isConnecting = false;
    };

    ws.onclose = () => {
      console.log('Disconnected from desktop app');
      ws = null;
      isConnecting = false;

      // Reject all pending requests
      for (const [messageId, { reject }] of pendingRequests.entries()) {
        reject(new Error('Connection to desktop app closed'));
      }
      pendingRequests.clear();

      // Attempt to reconnect after 3 seconds
      if (reconnectTimeout) clearTimeout(reconnectTimeout);
      reconnectTimeout = setTimeout(() => {
        connectToDesktop();
      }, 3000);
    };
  } catch (error) {
    console.error('Failed to create WebSocket:', error);
    isConnecting = false;
  }
}

// Send request to desktop app
function sendToDesktop(request) {
  return new Promise((resolve, reject) => {
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      connectToDesktop();
      reject(new Error('Desktop app not connected. Please ensure Unruggable desktop app is running.'));
      return;
    }

    const messageId = Math.random().toString(36).substring(7);

    // Store promise callbacks
    pendingRequests.set(messageId, { resolve, reject });

    // Add messageId to request
    const requestWithId = { ...request, messageId };

    // Send to desktop
    ws.send(JSON.stringify(requestWithId));

    // Timeout after 60 seconds
    setTimeout(() => {
      if (pendingRequests.has(messageId)) {
        pendingRequests.delete(messageId);
        reject(new Error('Request timeout'));
      }
    }, 60000);
  });
}

// Handle response from desktop app
function handleDesktopResponse(response) {
  // For now, we don't have messageId in the response
  // So we'll resolve the first pending request
  // TODO: Improve this by adding messageId to protocol
  if (pendingRequests.size > 0) {
    const [messageId, { resolve }] = pendingRequests.entries().next().value;
    pendingRequests.delete(messageId);
    resolve(response);
  }
}

// Listen for messages from content scripts and popup
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === 'BRIDGE_REQUEST') {
    // Forward to desktop app
    sendToDesktop(message.request)
      .then(response => {
        sendResponse(response);
      })
      .catch(error => {
        sendResponse({
          type: 'Error',
          message: error.message || 'Request failed'
        });
      });

    // Return true to indicate async response
    return true;
  }

  if (message.type === 'CHECK_DESKTOP_STATUS') {
    // Check connection and get wallet status
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      sendResponse({
        connected: false,
        locked: true
      });
      return true;
    }

    // Send GetPublicKey request to check status
    sendToDesktop({ method: 'GetPublicKey' })
      .then(response => {
        if (response.type === 'PublicKey') {
          sendResponse({
            connected: true,
            locked: false,
            publicKey: response.public_key,
            balance: 0, // TODO: Fetch actual balance
            connectedDapps: [] // TODO: Track connected dApps
          });
        } else {
          sendResponse({
            connected: true,
            locked: true
          });
        }
      })
      .catch(error => {
        sendResponse({
          connected: false,
          locked: true
        });
      });

    return true; // Keep channel open for async response
  }

  if (message.type === 'LOCK_WALLET') {
    // TODO: Implement lock functionality
    sendResponse({ success: true });
    return true;
  }

  if (message.type === 'DISCONNECT_SITE') {
    // TODO: Implement disconnect functionality
    sendResponse({ success: true });
    return true;
  }
});

// Check desktop app connection on extension load
chrome.runtime.onInstalled.addListener(() => {
  console.log('Unruggable extension installed');
  connectToDesktop();
});

chrome.runtime.onStartup.addListener(() => {
  console.log('Unruggable extension started');
  connectToDesktop();
});

// Initial connection attempt
connectToDesktop();
