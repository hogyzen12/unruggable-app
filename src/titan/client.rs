// Titan WebSocket client implementation

use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream, MaybeTlsStream};
use tokio::net::TcpStream;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::Mutex;
use futures_util::stream::Stream;

use super::types::*;
use super::codec::*;

/// Titan WebSocket client for swap quote streaming
pub struct TitanClient {
    /// WebSocket connection (wrapped in Arc<Mutex> for thread-safe access)
    ws: Arc<Mutex<Option<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    /// JWT authentication token
    jwt_token: String,
    /// Server endpoint
    endpoint: String,
    /// Request ID counter
    request_id: Arc<Mutex<u32>>,
}

impl TitanClient {
    /// Create a new Titan client
    /// 
    /// # Arguments
    /// * `endpoint` - Server endpoint (e.g., "de1.api.demo.titan.exchange")
    /// * `jwt_token` - JWT authentication token
    pub fn new(endpoint: String, jwt_token: String) -> Self {
        TitanClient {
            ws: Arc::new(Mutex::new(None)),
            jwt_token,
            endpoint,
            request_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Connect to Titan WebSocket server with protocol negotiation and JWT authentication
    pub async fn connect(&self) -> Result<(), String> {
        let url = format!("wss://{}/api/v1/ws", self.endpoint);
        println!("Connecting to Titan: {}", url);

        // Build WebSocket request with protocol negotiation and JWT auth
        let request = tokio_tungstenite::tungstenite::http::Request::builder()
            .uri(&url)
            .header("Host", &self.endpoint)
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", tokio_tungstenite::tungstenite::handshake::client::generate_key())
            .header("Sec-WebSocket-Protocol", "v1.api.titan.ag") // Protocol negotiation
            .header("Authorization", format!("Bearer {}", self.jwt_token)) // JWT auth
            .body(())
            .map_err(|e| format!("Failed to build request: {}", e))?;

        // Connect to WebSocket
        let (ws_stream, response) = connect_async(request)
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;

        println!("Connected to Titan! Response: {:?}", response.status());

        // Store the connection
        let mut ws_lock = self.ws.lock().await;
        *ws_lock = Some(ws_stream);

        Ok(())
    }

    /// Send a MessagePack-encoded request to the server
    async fn send_request(&self, request: ClientRequest) -> Result<(), String> {
        let mut ws_lock = self.ws.lock().await;
        let ws = ws_lock.as_mut().ok_or("Not connected")?;

        // Encode request as MessagePack using named (map-based) encoding
        // Titan requires structs to be encoded as maps, not arrays
        let encoded = rmp_serde::encode::to_vec_named(&request)
            .map_err(|e| format!("Failed to encode request: {}", e))?;

        println!("Sending request {} ({} bytes)", request.id, encoded.len());
        
        // Debug: Print hex dump of encoded message
        let preview_len = std::cmp::min(150, encoded.len());
        let hex_parts: Vec<String> = encoded[..preview_len]
            .chunks(16)
            .enumerate()
            .map(|(i, chunk)| {
                let hex: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
                format!("{:04x}: {}", i * 16, hex.join(" "))
            })
            .collect();
        println!("MessagePack hex dump:");
        for line in hex_parts {
            println!("  {}", line);
        }

        // Send as binary message
        ws.send(Message::Binary(encoded))
            .await
            .map_err(|e| format!("Failed to send: {}", e))?;

        Ok(())
    }

    /// Receive and decode a MessagePack message from the server
    async fn receive_message(&self) -> Result<ServerMessage, String> {
        let mut ws_lock = self.ws.lock().await;
        let ws = ws_lock.as_mut().ok_or("Not connected")?;

        // Receive message
        let msg = ws.next().await
            .ok_or("Connection closed")?
            .map_err(|e| format!("Failed to receive: {}", e))?;

        // Decode MessagePack
        match msg {
            Message::Binary(data) => {
                println!("Received message ({} bytes)", data.len());
                rmp_serde::from_slice(&data)
                    .map_err(|e| format!("Failed to decode: {}", e))
            }
            Message::Close(_) => Err("Connection closed".to_string()),
            _ => Err("Unexpected message type".to_string()),
        }
    }

    /// Get next request ID
    async fn next_request_id(&self) -> u32 {
        let mut id_lock = self.request_id.lock().await;
        let id = *id_lock;
        *id_lock += 1;
        id
    }

    /// Request server information
    pub async fn get_info(&self) -> Result<ServerInfo, String> {
        let request_id = self.next_request_id().await;
        let request = ClientRequest {
            id: request_id,
            data: RequestData::GetInfo(GetInfoRequest {}),
        };

        self.send_request(request).await?;

        // Wait for response
        loop {
            let msg = self.receive_message().await?;
            match msg {
                ServerMessage::Response(resp) if resp.request_id == request_id => {
                    match resp.data {
                        ResponseData::GetInfo(info) => return Ok(info),
                        _ => return Err("Unexpected response type".to_string()),
                    }
                }
                ServerMessage::Error(err) if err.request_id == request_id => {
                    return Err(format!("Server error: {} (code {})", err.message, err.code));
                }
                _ => {
                    // Ignore other messages (stream data, etc.)
                    continue;
                }
            }
        }
    }

    /// Request swap quotes with streaming updates
    /// Returns the best route from all providers
    pub async fn request_swap_quotes(
        &self,
        input_mint: &str,
        output_mint: &str,
        amount: u64,
        user_pubkey: &str,
        slippage_bps: Option<u16>,
    ) -> Result<(String, SwapRoute), String> {
        let request_id = self.next_request_id().await;
        
        // Convert pubkeys to bytes
        let input_mint_bytes = base58_to_bytes(input_mint)?;
        let output_mint_bytes = base58_to_bytes(output_mint)?;
        let user_pubkey_bytes = base58_to_bytes(user_pubkey)?;

        // Build swap quote request
        let request = ClientRequest {
            id: request_id,
            data: RequestData::NewSwapQuoteStream(SwapQuoteRequest {
                swap: SwapParams {
                    input_mint: input_mint_bytes,
                    output_mint: output_mint_bytes,
                    amount,
                    swap_mode: Some(SwapMode::ExactIn),
                    slippage_bps,
                    dexes: None,
                    exclude_dexes: None,
                    only_direct_routes: None,
                    add_size_constraint: None,
                    size_constraint: None,
                    providers: None,
                    accounts_limit_total: None,
                    accounts_limit_writable: None,
                },
                transaction: TransactionParams {
                    user_public_key: user_pubkey_bytes,
                    close_input_token_account: None,
                    create_output_token_account: Some(true),
                    fee_account: None,
                    fee_bps: None,
                    fee_from_input_mint: None,
                    output_account: None,
                },
                update: Some(QuoteUpdateParams {
                    interval_ms: Some(1000), // Update every second
                    num_quotes: Some(5),     // Get top 5 quotes
                }),
            }),
        };

        println!("Requesting swap quotes: {} -> {} (amount: {})", input_mint, output_mint, amount);
        self.send_request(request).await?;

        // Wait for initial response with stream ID
        let stream_id = loop {
            let msg = self.receive_message().await?;
            match msg {
                ServerMessage::Response(resp) if resp.request_id == request_id => {
                    if let Some(stream) = resp.stream {
                        println!("Quote stream started with ID: {}", stream.id);
                        break stream.id;
                    } else {
                        return Err("No stream started".to_string());
                    }
                }
                ServerMessage::Error(err) if err.request_id == request_id => {
                    return Err(format!("Server error: {} (code {})", err.message, err.code));
                }
                _ => continue,
            }
        };

        // Wait for first quote data
        let quotes = loop {
            let msg = self.receive_message().await?;
            match msg {
                ServerMessage::StreamData(data) if data.id == stream_id => {
                    match data.payload {
                        StreamDataPayload::SwapQuotes(quotes) => {
                            println!("Received quotes from {} providers", quotes.quotes.len());
                            break quotes;
                        }
                    }
                }
                ServerMessage::StreamEnd(end) if end.id == stream_id => {
                    if let Some(err_msg) = end.error_message {
                        return Err(format!("Stream ended with error: {}", err_msg));
                    }
                    return Err("Stream ended without quotes".to_string());
                }
                _ => continue,
            }
        };

        // Stop the stream (we only need one quote)
        self.stop_stream(stream_id).await?;

        // Find best route (highest out_amount for ExactIn)
        let (best_provider, best_route) = quotes.quotes.iter()
            .max_by_key(|(_, route)| route.out_amount)
            .ok_or("No quotes available")?;

        println!("Best quote from provider '{}': {} output tokens", best_provider, best_route.out_amount);

        Ok((best_provider.clone(), best_route.clone()))
    }

    /// Stop a streaming quote
    async fn stop_stream(&self, stream_id: u32) -> Result<(), String> {
        let request_id = self.next_request_id().await;
        let request = ClientRequest {
            id: request_id,
            data: RequestData::StopStream(StopStreamRequest { id: stream_id }),
        };

        self.send_request(request).await?;

        // Wait for confirmation
        loop {
            let msg = self.receive_message().await?;
            match msg {
                ServerMessage::Response(resp) if resp.request_id == request_id => {
                    println!("Stream {} stopped", stream_id);
                    return Ok(());
                }
                ServerMessage::Error(err) if err.request_id == request_id => {
                    return Err(format!("Failed to stop stream: {}", err.message));
                }
                ServerMessage::StreamEnd(end) if end.id == stream_id => {
                    // Stream ended naturally
                    return Ok(());
                }
                _ => continue,
            }
        }
    }

    /// Close the WebSocket connection
    pub async fn close(&self) -> Result<(), String> {
        let mut ws_lock = self.ws.lock().await;
        if let Some(mut ws) = ws_lock.take() {
            ws.close(None)
                .await
                .map_err(|e| format!("Failed to close: {}", e))?;
            println!("Titan connection closed");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = TitanClient::new(
            "de1.api.demo.titan.exchange".to_string(),
            "test_token".to_string(),
        );
        // Just verify it compiles and creates
        assert!(client.ws.lock().await.is_none());
    }
}