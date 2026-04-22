// src/server/hmjserver.rs
//
// Handle connections with performer client (autohmjclient)
//
// Needs tokio runtime because main app is sync

use crate::{
    events::{ConvoEvent, HMJEventBus},
    models::HMJMessageWrapper,
};
use futures_util::{SinkExt, StreamExt};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc, Mutex},
};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};

// Event Interface
#[derive(Clone, Debug)]
pub enum ServerEvent {
    NeedConversation,            // need to broadcast Conversation
    NeedConversationFor(String), // need to send Conversation to Client ID
}

pub struct HMJServer {
    // Server configuration
    port: u16,

    // Async task channel - messages
    // Used to directly send amd receive messages to a specific client
    message_tx: mpsc::Sender<HMJMessageWrapper>,
    message_rx: mpsc::Receiver<HMJMessageWrapper>,

    // Client broadcast channel
    // Used to broadcast convo messages to all clients
    broadcast_tx: broadcast::Sender<String>,

    // Clients with direct channel
    clients: Arc<Mutex<HashMap<String, mpsc::Sender<Message>>>>,

    // Client Registration channel
    reg_tx: mpsc::Sender<String>,
    reg_rx: mpsc::Receiver<String>,

    // Events channel
    event_tx: broadcast::Sender<ServerEvent>,
    history_rx: broadcast::Receiver<ConvoEvent>, // subscribe to HistoryEvents

    // Tokio handle
    rthandle: tokio::runtime::Handle,
    server_started: bool,

    // Shutdown channel
    // Used to signal tasks to terminate
    shutdown_tx: broadcast::Sender<()>,
}

impl HMJServer {
    pub fn new(port: u16, events: &HMJEventBus, rthandle: tokio::runtime::Handle) -> Self {
        // Create comms channels
        let (message_tx, message_rx) = mpsc::channel(16);
        let (broadcast_tx, _) = broadcast::channel(16);
        let (shutdown_tx, _) = broadcast::channel(1);
        let (reg_tx, reg_rx) = mpsc::channel(16);

        // Set up events channels
        let event_tx = events.server.clone();
        let history_rx = events.convo.subscribe();

        Self {
            port,
            message_tx,
            message_rx,
            broadcast_tx,
            clients: Arc::new(Mutex::new(HashMap::new())),
            reg_tx,
            reg_rx,
            event_tx,
            history_rx,
            rthandle,
            server_started: false,
            shutdown_tx,
        }
    }

    // The server's main loop is simple:
    // 1. Process events and trigger appropriate actions
    // 2. Gather new client registrations
    pub fn update(&mut self) {
        self.process_events();
        self.gather_registrations();
    }

    // The server listens for events from the EventBus and triggers appropriate actions.
    fn process_events(&mut self) {
        while let Ok(event) = self.history_rx.try_recv() {
            match event {
                // Broadcast Conversation to all Clients
                // This command originates from ConversationService as a response to a request from this server
                // for the serialized conversation history.
                ConvoEvent::BroadcastConvoCommand(convo) => {
                    self.broadcast(convo);
                }
                // Send Conversation to a specific client
                // This command originates from ConversationService as a response to a request from this server
                // for the serialized conversation history, but only for a specific client.
                ConvoEvent::SendConvoCommand(client_id, convo) => {
                    self.send_to_client(&client_id, convo);
                }
                _ => {}
            }
        }
    }

    /// Start the WebSocket server
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // If server is already running, return Ok.
        if self.server_started {
            return Ok(());
        }

        // Clone channels for server task
        let message_tx = self.message_tx.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let port = self.port;
        let clients = self.clients.clone();
        let reg_tx = self.reg_tx.clone();

        // Start server task
        self.rthandle.spawn(async move {

            // Create TCP listener
            let addr = format!("0.0.0.0:{}", port);
            let listener = match TcpListener::bind(&addr).await {
                Ok(listener) => listener,
                Err(e) => {
                    eprintln!("Failed to bind to {}: {}", addr, e);
                    return;
                }
            };

            println!("HMJServer listening on {}", addr);

            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, addr)) => {
                                let clients = clients.clone();
                                let message_tx = message_tx.clone();
                                let broadcast_tx = broadcast_tx.clone();
                                let conn_shutdown_rx = shutdown_rx.resubscribe();
                                let reg_tx = reg_tx.clone();

                                tokio::spawn(async move {
                                    if let Err(e) = handle_connection(
                                        stream, addr, clients, message_tx, broadcast_tx, conn_shutdown_rx, reg_tx,
                                    ).await {
                                        eprintln!("Error connecting to client: {}", e);
                                    }
                                });
                            }
                            Err(e) => eprintln! ("Error while accepting connection: {}", e),
                        }
                    }

                    _ = shutdown_rx.recv() => {
                        println!(".....WebSocket listener shutting down");
                        break;
                    }
                }
            }
        });

        self.server_started = true;

        Ok(())
    }

    /// Try to receive a message from clients (non-blocking)
    pub fn try_recv_message(&mut self) -> Option<HMJMessageWrapper> {
        self.message_rx.try_recv().ok()
    }

    /// Broadcast a message to all connected clients
    pub fn broadcast(&self, message: String) {
        let _ = self.broadcast_tx.send(message);
    }

    // Send a message to a particular client
    pub fn send_to_client(&self, client_id: &str, message: String) {
        if let Ok(clients) = self.clients.try_lock() {
            if let Some(tx) = clients.get(client_id) {
                if let Err(e) = tx.try_send(Message::Text(message.into())) {
                    eprintln!("Error sending message to client {}: {}", client_id, e);
                }
            } else {
                eprintln!(
                    "Attempted to send message to unregistered client: {}",
                    client_id
                );
            }
        } else {
            eprintln!(
                "Could not acquire lock to send message to client: {}",
                client_id
            );
        }
    }

    // Get newly registered clients
    pub fn gather_registrations(&mut self) {
        while let Ok(client_id) = self.reg_rx.try_recv() {
            let _ = self
                .event_tx
                .send(ServerEvent::NeedConversationFor(client_id));
        }
    }

    /// Shutdown the server gracefully
    pub fn shutdown(&mut self) {
        println!("...Shutting down HMJServer...");

        // Signal all tasks to terminate
        let _ = self.shutdown_tx.send(());

        println!("...HMJServer shutdown complete");
    }
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    clients: Arc<Mutex<HashMap<String, mpsc::Sender<Message>>>>,
    message_tx: mpsc::Sender<HMJMessageWrapper>,
    broadcast_tx: broadcast::Sender<String>,
    shutdown_rx: broadcast::Receiver<()>,
    reg_tx: mpsc::Sender<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Accept WebSocket Connection
    let ws_stream: WebSocketStream<TcpStream> = accept_async(stream)
        .await
        .expect("Error receiving WebSocket stream");

    // Split the WebSocket stream with explicit types
    let (mut ws_sender, mut ws_receiver) = StreamExt::split(ws_stream);

    // Create channel for sending messages to client
    let (client_tx, mut client_rx) = mpsc::channel(16);

    // Client ID (set when registered)
    let client_id = Arc::new(Mutex::new(String::new()));

    // Create broadcast subscription
    let mut broadcast_rx = broadcast_tx.subscribe();

    // Create shutdown subscriptions for each task
    let mut sender_shutdown_rx = shutdown_rx.resubscribe();
    let mut recv_shutdown_rx = shutdown_rx.resubscribe();
    let mut main_shutdown_rx = shutdown_rx.resubscribe();

    // Task to send messages to the WebSocket (handles both direct and broadcast messages)
    let sender_handle = {
        let _client_id = client_id.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Send a direct message
                    Some(msg) = client_rx.recv() => {
                        if let Err(e) = ws_sender.send(msg).await {
                            eprintln!("Error sending direct message: {}", e);
                            break;
                        }
                    }
                    // Relay messages received via broadcast task bus, and send through WebSocket
                    result = broadcast_rx.recv() => {
                        match result {
                            Ok(msg) => {
                                if let Err(e) = ws_sender.send(Message::Text(msg.into())).await {
                                    eprintln!("Error sending broadcast: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                eprintln!("Broadcast channel error: {}", e);
                                break; // Broadcast channel error
                            }
                        }
                    }
                    _ = sender_shutdown_rx.recv() => {
                        println!(".....WebSocket sender shutting down...");
                        break;
                    }
                }
            }
        })
    };

    // Task to receive messages from the WebSocket
    let receiver_handle = {
        let client_id = client_id.clone();
        let clients = clients.clone();
        let message_tx = message_tx.clone();
        let client_tx = client_tx.clone();
        let reg_tx = reg_tx.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = ws_receiver.next() => {
                        match result {
                            Some(Ok(msg)) => {
                                if let Message::Text(text) = msg {
                                    // Handle registration message
                                    if let Some((command, payload)) = text.split_once('|') {
                                        if command == ("register") {
                                            let id = payload.to_owned();

                                            // Store client ID
                                            {
                                                let mut client_id_lock = client_id.lock().await;
                                                *client_id_lock = id.clone();
                                            }

                                            // Add client to map
                                            let mut clients_lock = clients.lock().await;
                                            clients_lock.insert(id.clone(), client_tx.clone());

                                            // Notice on new client registration
                                            let _ = reg_tx.send(id.clone()).await;

                                            println!("Client registered: {} from {}", id, addr);
                                            continue;
                                        }
                                    }

                                    // Handle regular message
                                    let id = {
                                        client_id.lock().await.clone()
                                    };

                                    if !id.is_empty() {
                                        match serde_json::from_str::<HMJMessageWrapper>(&text) {
                                            Ok(hmj_message) => {
                                                if let Err(e) = message_tx.send(hmj_message).await {
                                                    eprintln!("Failed to send message to channel: {}", e);
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!("Failed to deserialize HMJMessage from text: {}\nError: {}", text, e);
                                            }
                                        }
                                    } else {
                                        eprintln!("Message from unregistered client: {}", addr);
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                eprintln!("WebSocket receive error: {}", e);
                                break;
                            }
                            None => break, // Connection closed
                        }
                    }
                    _ = recv_shutdown_rx.recv() => {
                        println!(".....Shutting down WebSocket receiver process");
                        break;
                    }
                }
            }

            // Remove client on disconnect
            let id = { client_id.lock().await.clone() };

            if !id.is_empty() {
                let mut clients_lock = clients.lock().await;
                clients_lock.remove(&id);
                println!("Client disconnected: {} from {}", id, addr);
            }
        })
    };

    // Wait for both tasks to complete

    tokio::select! {
        result = async {
            // Waiting for either task to complete (which implies the connection is done)
            tokio::select! {
                _ = sender_handle => {}
                _ = receiver_handle => {}
            }
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        } => { result? }
        _ = main_shutdown_rx.recv() => {
            println!(".....Connection handler shutting down: {}", addr);
            // Just return OK - the tasks will exit naturally via their own shutdown receivers
        }
    }

    Ok(())
}

impl Drop for HMJServer {
    fn drop(&mut self) {
        println!("...HMJServer being dropped");
        self.shutdown();
        // Wait briefly
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
