use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use futures_util::{StreamExt, SinkExt, future::BoxFuture};
use serde_json;

use crate::bridge::protocol::{BridgeRequest, BridgeResponse};

pub type RequestCallback = Arc<dyn Fn(BridgeRequest) -> BoxFuture<'static, BridgeResponse> + Send + Sync>;

pub struct BridgeServer {
    port: u16,
    callback: Arc<Mutex<Option<RequestCallback>>>,
}

impl BridgeServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            callback: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_callback(&self, callback: RequestCallback) {
        let cb = Arc::clone(&self.callback);
        tokio::spawn(async move {
            let mut cb_lock = cb.lock().await;
            *cb_lock = Some(callback);
        });
    }

    pub async fn start(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error>> {
        let addr: SocketAddr = format!("127.0.0.1:{}", self.port).parse()?;
        let listener = TcpListener::bind(&addr).await?;

        println!("🌉 Browser bridge running on ws://localhost:{}", self.port);

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let server = Arc::clone(&self);
                    tokio::spawn(async move {
                        if let Err(e) = server.handle_connection(stream).await {
                            eprintln!("Error handling connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Error accepting connection: {}", e);
                }
            }
        }
    }

    async fn handle_connection(&self, stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
        let ws_stream = accept_async(stream).await?;
        let (mut write, mut read) = ws_stream.split();

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Parse request
                    let request: BridgeRequest = match serde_json::from_str(&text) {
                        Ok(req) => req,
                        Err(e) => {
                            let error_response = BridgeResponse::Error {
                                message: format!("Invalid request format: {}", e),
                            };
                            let response_text = serde_json::to_string(&error_response)?;
                            write.send(Message::Text(response_text)).await?;
                            continue;
                        }
                    };

                    // Get callback and handle request
                    let callback = {
                        let cb_lock = self.callback.lock().await;
                        cb_lock.clone()
                    };

                    let response = if let Some(cb) = callback {
                        cb(request).await
                    } else {
                        BridgeResponse::Error {
                            message: "Desktop wallet not ready".to_string(),
                        }
                    };

                    // Send response
                    let response_text = serde_json::to_string(&response)?;
                    write.send(Message::Text(response_text)).await?;
                }
                Ok(Message::Close(_)) => {
                    break;
                }
                Ok(Message::Ping(data)) => {
                    write.send(Message::Pong(data)).await?;
                }
                Err(e) => {
                    eprintln!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }
}
