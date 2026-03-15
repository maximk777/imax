use iroh::{Endpoint, EndpointAddr, EndpointId, SecretKey};
use iroh::endpoint::Connection;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use crate::Result;
use crate::network::protocol::{self, WireMessage};

pub const ALPN: &[u8] = b"imax/1";

pub struct IrohNode {
    endpoint: Endpoint,
    connections: Mutex<HashMap<EndpointId, Connection>>,
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
        })
    }

    pub fn node_id(&self) -> EndpointId {
        self.endpoint.id()
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// Get a cached connection or create a new one to the given peer.
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
        self.clear_connections();
        self.endpoint.close().await;
        Ok(())
    }
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
