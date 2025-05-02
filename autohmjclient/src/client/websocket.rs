// src/client/websocket.rs
//
// Client module for HMJServer WebSocket communication

use autohmjcommon::HMJMessage;
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

// Commands that the WS client understands
enum Command {
    // Send a message through the WebSocket
    SendMessage(HMJMessage),
    // Request to reconnect with server
    Reconnect,
    // Request to shut down the client
    Shutdown(oneshot::Sender<()>),
}

pub struct HMJClient {
    runtime: Option<tokio::runtime::Runtime>,

    // channel for communicating with background task
    command_tx: mpsc::Sender<Command>,

    // Channel for receiving messages from server
    message_rx: mpsc::Receiver<String>,

    client_id: String,
    pub is_connected: bool,
    task_handle: Option<JoinHandle<()>>,
}

impl HMJClient {
    pub fn new(server_addr: &str, server_port: u32, id: &str) -> Self {
        // Create channels
        let (command_tx, command_rx) = mpsc::channel(16);
        let (message_tx, message_rx) = mpsc::channel(16);

        // Create tokio runtime
        let runtime =
            tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime for HMJClient");

        // Start client task
        let server_url = format!("ws://{}:{}", server_addr, server_port);
        let client_id = id.to_owned();
        let client_id_clone = client_id.clone();

        let task_handle = runtime.spawn(async move {
            run_client(server_url, client_id_clone, command_rx, message_tx).await;
        });

        Self {
            runtime: Some(runtime),
            command_tx,
            message_rx,
            client_id,
            is_connected: false,
            task_handle: Some(task_handle),
        }
    }

    // Send a message to the server via WS
    pub fn send(&self, message: HMJMessage) {
        // Use blocking_send here because egui-based main app is synchronous
        let _ = self.command_tx.blocking_send(Command::SendMessage(message));
    }

    // Try to receive message from server (non-blocking)
    pub fn try_recv(&mut self) -> Option<String> {
        // If there's an error, just convert it to None
        self.message_rx.try_recv().ok()
    }

    // Force a connection to the server
    pub fn reconnect(&self) {
        let _ = self.command_tx.blocking_send(Command::Reconnect);
    }
}

impl Drop for HMJClient {
    fn drop(&mut self) {
        // Graceful shutdown
        if let Some(runtime) = &self.runtime {
            let (tx, rx) = oneshot::channel();
            let _ = self.command_tx.blocking_send(Command::Shutdown(tx));

            // wait for confirmation or timeout
            runtime.block_on(async {
                tokio::select! {
                    _ = rx => {}
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                }
            });

            if let Some(handle) = self.task_handle.take() {
                runtime.block_on(async {
                    handle.abort();
                });
            }
        }
    }
}

// The WS client task
async fn run_client(
    server_url: String,
    client_id_clone: String,
    mut command_rx: mpsc::Receiver<Command>,
    message_tx: mpsc::Sender<String>,
) {
    let mut ws_stream: Option<WsStream> = None;
    let mut reconnect_delay = Duration::from_millis(100);
    let max_reconnect_delay = Duration::from_secs(10);

    loop {
        // Try to connect if not connected
        if ws_stream.is_none() {
            match connect_async(&server_url).await {
                Ok((mut stream, _)) => {
                    // Send registration msg
                    let register_msg = format!("register|{}", client_id_clone);
                    if let Err(e) = stream.send(Message::Text(register_msg.into())).await {
                        eprintln!("Failed to register with server: {}", e);
                        ws_stream = None;
                        tokio::time::sleep(reconnect_delay).await;
                        reconnect_delay = std::cmp::min(reconnect_delay * 2, max_reconnect_delay);
                        continue;
                    }
                    ws_stream = Some(stream);
                    reconnect_delay = Duration::from_millis(100);
                }
                Err(e) => {
                    eprintln!("Failed to connect: {}", e);
                    tokio::time::sleep(reconnect_delay).await;
                    reconnect_delay = std::cmp::min(reconnect_delay * 2, max_reconnect_delay);
                    continue;
                }
            }
        }

        // Process commands & messages
        if let Some(ref mut stream) = ws_stream {
            tokio::select! {
                // Handle commands from HMJClient
                cmd = command_rx.recv() => {
                    match cmd {
                        Some(Command::SendMessage(msg)) => {
                            if let Err(e) = stream.send(Message::Text(msg.serialize().into())).await {
                                eprintln!("Error sending message: {}", e);
                                // Trigger reconnect
                                ws_stream = None;
                            }
                        }
                        Some(Command::Reconnect) => {
                            ws_stream = None;
                        }
                        Some(Command::Shutdown(tx)) => {
                            let _ = stream.close(None).await;
                            let _ = tx.send(());
                            return;
                        }
                        None => return,
                    }
                }
                // Handle messages from server
                msg = stream.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            let _ = message_tx.send(text.to_string()).await;
                        }
                        Some(Err(e)) => {
                            eprintln!("Websocket error: {}", e);
                            ws_stream = None;
                        }
                        None => {
                            ws_stream = None;
                        }
                        _ => {}
                    }
                }

                // Ping to keep connection alive
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    if stream.send(Message::Ping(tokio_tungstenite::tungstenite::Bytes::new())).await.is_err() {
                        ws_stream = None;
                    }
                }
            }
        } else {
            // if disconnected, check for commands while waiting
            tokio::select! {
                cmd = command_rx.recv() => {
                    match cmd {
                        Some(Command::Shutdown(tx)) => {
                            let _ = tx.send(());
                            return;
                        }
                        Some(Command::Reconnect) => {
                            // already planning to reconnect
                        }
                        Some(_) => {
                            // can't process other commands while disconnected
                        }
                        None => return, // channel closed: exit
                    }
                }
                _ = tokio::time::sleep(reconnect_delay) => {
                    // Time to try reconnecting
                }
            }
        }
    }
}
