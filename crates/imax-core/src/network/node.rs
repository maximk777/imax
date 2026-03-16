use iroh::{Endpoint, EndpointAddr, EndpointId, SecretKey};
use iroh::endpoint::Connection;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use crate::Result;
use crate::network::protocol::{self, WireMessage};

pub const ALPN: &[u8] = b"imax/1";

pub struct IrohNode {
    endpoint: Endpoint,
    connections: Mutex<HashMap<EndpointId, Connection>>,
    /// Channel to register new outgoing connections for stream listening.
    /// Set by `run_accept_loop`, used by `send_to_peer`/`send_to_addr`.
    conn_register_tx: Mutex<Option<mpsc::UnboundedSender<(Connection, EndpointId)>>>,
}

impl IrohNode {
    pub async fn new(secret_key: SecretKey) -> Result<Self> {
        let endpoint = Endpoint::builder()
            .secret_key(secret_key)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .map_err(|e| crate::Error::Network(format!("endpoint bind: {e}")))?;
        Ok(Self {
            endpoint,
            connections: Mutex::new(HashMap::new()),
            conn_register_tx: Mutex::new(None),
        })
    }

    pub fn node_id(&self) -> EndpointId {
        self.endpoint.id()
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// Get a cached connection or create a new one to the given peer.
    /// If a new connection is created and an accept loop is running,
    /// registers it for stream listening.
    async fn get_or_connect(&self, node_id: EndpointId) -> Result<Connection> {
        // 1. Check cache
        {
            let conns = self.connections.lock().unwrap();
            if let Some(conn) = conns.get(&node_id) {
                if conn.close_reason().is_none() {
                    return Ok(conn.clone());
                }
            }
        }
        // 2. New connection
        let conn = self.endpoint.connect(node_id, ALPN).await
            .map_err(|e| crate::Error::Network(format!("connect error: {e}")))?;
        // 3. Cache it
        self.connections.lock().unwrap().insert(node_id, conn.clone());
        // 4. Register for stream listening
        if let Some(tx) = self.conn_register_tx.lock().unwrap().as_ref() {
            let _ = tx.send((conn.clone(), node_id));
        }
        Ok(conn)
    }

    /// Send a WireMessage to a peer, reusing cached connections.
    pub async fn send_to_peer(&self, node_id: EndpointId, msg: &WireMessage) -> Result<()> {
        let conn = self.get_or_connect(node_id).await?;

        let (mut send, _recv) = conn
            .open_bi()
            .await
            .map_err(|e| crate::Error::Network(format!("open_bi error: {e}")))?;

        let encoded = protocol::encode(msg)?;
        send.write_all(&encoded)
            .await
            .map_err(|e| crate::Error::Network(format!("write error: {e}")))?;

        send.finish()
            .map_err(|e| crate::Error::Network(format!("finish error: {e}")))?;

        let _ = tokio::time::timeout(Duration::from_secs(5), send.stopped()).await;

        Ok(())
    }

    /// Send a WireMessage to a peer using a full EndpointAddr, reusing cached connections.
    pub async fn send_to_addr(&self, addr: EndpointAddr, msg: &WireMessage) -> Result<()> {
        let peer_id = addr.id;
        let conn = {
            let conns = self.connections.lock().unwrap();
            conns.get(&peer_id).filter(|c| c.close_reason().is_none()).cloned()
        };
        let conn = match conn {
            Some(c) => c,
            None => {
                let c = self.endpoint.connect(addr, ALPN).await
                    .map_err(|e| crate::Error::Network(format!("connect error: {e}")))?;
                self.connections.lock().unwrap().insert(peer_id, c.clone());
                // Register for stream listening
                if let Some(tx) = self.conn_register_tx.lock().unwrap().as_ref() {
                    let _ = tx.send((c.clone(), peer_id));
                }
                c
            }
        };

        let (mut send, _recv) = conn
            .open_bi()
            .await
            .map_err(|e| crate::Error::Network(format!("open_bi error: {e}")))?;

        let encoded = protocol::encode(msg)?;
        send.write_all(&encoded)
            .await
            .map_err(|e| crate::Error::Network(format!("write error: {e}")))?;

        send.finish()
            .map_err(|e| crate::Error::Network(format!("finish error: {e}")))?;

        let _ = tokio::time::timeout(Duration::from_secs(5), send.stopped()).await;

        Ok(())
    }

    /// Accept one incoming connection and read a WireMessage.
    /// Caches the incoming connection for potential reply reuse.
    pub async fn accept_one(&self) -> Result<(WireMessage, EndpointId)> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| crate::Error::Network("endpoint closed, no incoming connections".into()))?;

        let conn = incoming
            .await
            .map_err(|e| crate::Error::Network(format!("accept handshake error: {e}")))?;

        let remote_id = conn.remote_id();

        // Cache the incoming connection for reply
        self.connections.lock().unwrap().insert(remote_id, conn.clone());

        let (_send, mut recv) = conn
            .accept_bi()
            .await
            .map_err(|e| crate::Error::Network(format!("accept_bi error: {e}")))?;

        let bytes = recv
            .read_to_end(16 * 1024 * 1024)
            .await
            .map_err(|e| crate::Error::Network(format!("read error: {e}")))?;

        let _ = recv.stop(0u32.into());

        let (msg, _consumed) = protocol::decode_frame(&bytes)?
            .ok_or_else(|| crate::Error::Network("incomplete message frame".into()))?;

        Ok((msg, remote_id))
    }

    /// Start an accept loop that handles both new connections and new streams
    /// on existing connections. Sends received messages to the returned channel.
    /// Also listens for streams on outgoing connections registered via send_to_peer/send_to_addr.
    /// Runs until the CancellationToken is cancelled.
    pub fn run_accept_loop(
        self: &std::sync::Arc<Self>,
        cancel: CancellationToken,
    ) -> mpsc::UnboundedReceiver<(WireMessage, EndpointId)> {
        let (tx, rx) = mpsc::unbounded_channel();
        let (conn_reg_tx, mut conn_reg_rx) = mpsc::unbounded_channel::<(Connection, EndpointId)>();

        // Store the registration channel so send_to_peer/send_to_addr can use it
        *self.conn_register_tx.lock().unwrap() = Some(conn_reg_tx);

        let node = std::sync::Arc::clone(self);

        tokio::spawn(async move {
            // Track which connections already have a stream listener
            let mut listening: HashSet<EndpointId> = HashSet::new();

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,

                    // New incoming connection
                    incoming = node.endpoint.accept() => {
                        let Some(incoming) = incoming else { break };
                        let conn = match incoming.await {
                            Ok(c) => c,
                            Err(e) => {
                                println!("[imax] accept handshake error: {e}");
                                continue;
                            }
                        };
                        let remote_id = conn.remote_id();
                        node.connections.lock().unwrap().insert(remote_id, conn.clone());

                        if listening.insert(remote_id) {
                            spawn_stream_listener(conn, remote_id, tx.clone(), cancel.clone());
                        }
                    }

                    // Outgoing connection registered for stream listening
                    Some((conn, remote_id)) = conn_reg_rx.recv() => {
                        if listening.insert(remote_id) {
                            spawn_stream_listener(conn, remote_id, tx.clone(), cancel.clone());
                        }
                    }
                }
            }
        });

        rx
    }

    /// Clear all cached connections.
    pub fn clear_connections(&self) {
        self.connections.lock().unwrap().clear();
    }

    /// Number of cached connections (useful for tests).
    pub fn cached_connection_count(&self) -> usize {
        self.connections.lock().unwrap().len()
    }

    /// Shut down the node, closing all QUIC connections and the endpoint.
    pub async fn shutdown(&self) -> Result<()> {
        // Drop the registration channel
        *self.conn_register_tx.lock().unwrap() = None;
        self.clear_connections();
        self.endpoint.close().await;
        Ok(())
    }
}

/// Spawn a task that listens for incoming bi-streams on a connection
/// and forwards decoded messages to the tx channel.
fn spawn_stream_listener(
    conn: Connection,
    remote_id: EndpointId,
    tx: mpsc::UnboundedSender<(WireMessage, EndpointId)>,
    cancel: CancellationToken,
) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                stream = conn.accept_bi() => {
                    match stream {
                        Ok((_send, mut recv)) => {
                            let bytes = match recv.read_to_end(16 * 1024 * 1024).await {
                                Ok(b) => b,
                                Err(e) => {
                                    println!("[imax] stream read error from {remote_id:?}: {e}");
                                    continue;
                                }
                            };
                            let _ = recv.stop(0u32.into());
                            match protocol::decode_frame(&bytes) {
                                Ok(Some((msg, _))) => {
                                    let _ = tx.send((msg, remote_id));
                                }
                                Ok(None) => println!("[imax] incomplete frame from {remote_id:?}"),
                                Err(e) => println!("[imax] decode error from {remote_id:?}: {e}"),
                            }
                        }
                        Err(_) => break, // connection closed
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_iroh_node() {
        let secret_key = SecretKey::from_bytes(&[42u8; 32]);
        let node = IrohNode::new(secret_key).await.expect("node creation failed");
        let node_id = node.node_id();
        let id_bytes = node_id.as_bytes();
        assert_ne!(id_bytes, &[0u8; 32], "node_id should not be all zeros");
        node.shutdown().await.expect("shutdown failed");
    }

    #[tokio::test]
    async fn test_two_nodes_send_receive() {
        let key_a = SecretKey::from_bytes(&[1u8; 32]);
        let key_b = SecretKey::from_bytes(&[2u8; 32]);

        let node_a = IrohNode::new(key_a).await.expect("node_a creation failed");
        let node_b = IrohNode::new(key_b).await.expect("node_b creation failed");

        let node_a_id = node_a.node_id();
        let node_b_id = node_b.node_id();

        node_a.endpoint().online().await;
        node_b.endpoint().online().await;

        let msg_to_send = WireMessage::Hello {
            public_key: [42u8; 32],
            nickname: "Alice".to_string(),
            protocol_version: 1,
        };

        let node_b_addr = node_b.endpoint().addr();

        let msg_clone = msg_to_send.clone();
        let sender = tokio::spawn(async move {
            node_a.send_to_addr(node_b_addr, &msg_clone).await.expect("send_to_addr failed");
            node_a
        });

        let (received_msg, from_id) = node_b.accept_one().await.expect("accept_one failed");

        assert_eq!(received_msg, msg_to_send, "received message should match sent message");
        assert_eq!(from_id, node_a_id, "sender should be node_a");

        let node_a = sender.await.expect("sender task panicked");
        node_a.shutdown().await.expect("node_a shutdown failed");
        node_b.shutdown().await.expect("node_b shutdown failed");

        let _ = node_b_id;
    }
}
