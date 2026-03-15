use iroh::{Endpoint, EndpointAddr, EndpointId, SecretKey};
use std::time::Duration;
use crate::Result;
use crate::network::protocol::{self, WireMessage};

pub const ALPN: &[u8] = b"imax/1";

pub struct IrohNode {
    endpoint: Endpoint,
}

impl IrohNode {
    pub async fn new(secret_key: SecretKey) -> Result<Self> {
        let endpoint = Endpoint::builder()
            .secret_key(secret_key)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .map_err(|e| crate::Error::Network(format!("endpoint bind: {e}")))?;
        Ok(Self { endpoint })
    }

    pub fn node_id(&self) -> EndpointId {
        self.endpoint.id()
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// Send a WireMessage to a peer over a new bidirectional QUIC stream.
    pub async fn send_to_peer(&self, node_id: EndpointId, msg: &WireMessage) -> Result<()> {
        // Connect to peer (EndpointId implements Into<EndpointAddr>)
        let conn = self
            .endpoint
            .connect(node_id, ALPN)
            .await
            .map_err(|e| crate::Error::Network(format!("connect error: {e}")))?;

        // Open a bidirectional stream
        let (mut send, _recv) = conn
            .open_bi()
            .await
            .map_err(|e| crate::Error::Network(format!("open_bi error: {e}")))?;

        // Encode and write the message
        let encoded = protocol::encode(msg)?;
        send.write_all(&encoded)
            .await
            .map_err(|e| crate::Error::Network(format!("write error: {e}")))?;

        // Signal end of stream — QUIC guarantees delivery of all data up to FIN
        send.finish()
            .map_err(|e| crate::Error::Network(format!("finish error: {e}")))?;

        // Wait for receiver to signal it processed the stream (via stop()),
        // with a timeout so we don't hang forever if the peer misbehaves.
        let _ = tokio::time::timeout(Duration::from_secs(5), send.stopped()).await;

        Ok(())
    }

    /// Send a WireMessage to a peer using a full EndpointAddr (includes relay + direct addrs).
    pub async fn send_to_addr(&self, addr: EndpointAddr, msg: &WireMessage) -> Result<()> {
        let conn = self
            .endpoint
            .connect(addr, ALPN)
            .await
            .map_err(|e| crate::Error::Network(format!("connect error: {e}")))?;

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

        // Wait for receiver to signal it processed the stream (via stop()),
        // with a timeout so we don't hang forever if the peer misbehaves.
        let _ = tokio::time::timeout(Duration::from_secs(5), send.stopped()).await;

        Ok(())
    }

    /// Accept one incoming connection and read a WireMessage.
    /// Returns the decoded WireMessage and the remote EndpointId.
    pub async fn accept_one(&self) -> Result<(WireMessage, EndpointId)> {
        // Wait for an incoming connection
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| crate::Error::Network("endpoint closed, no incoming connections".into()))?;

        // Complete the handshake
        let conn = incoming
            .await
            .map_err(|e| crate::Error::Network(format!("accept handshake error: {e}")))?;

        let remote_id = conn.remote_id();

        // Accept a bidirectional stream
        let (_send, mut recv) = conn
            .accept_bi()
            .await
            .map_err(|e| crate::Error::Network(format!("accept_bi error: {e}")))?;

        // Read all bytes (up to 16 MiB)
        let bytes = recv
            .read_to_end(16 * 1024 * 1024)
            .await
            .map_err(|e| crate::Error::Network(format!("read error: {e}")))?;

        // Signal sender that we're done reading — this unblocks send.stopped()
        // stop() on RecvStream sends STOP_SENDING to the peer's SendStream
        let _ = recv.stop(0u32.into());

        // Decode the framed message
        let (msg, _consumed) = protocol::decode_frame(&bytes)?
            .ok_or_else(|| crate::Error::Network("incomplete message frame".into()))?;

        Ok((msg, remote_id))
    }

    pub async fn shutdown(self) -> Result<()> {
        self.endpoint.close().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_iroh_node() {
        // Generate key from fixed bytes to avoid rand version conflicts
        let secret_key = SecretKey::from_bytes(&[42u8; 32]);
        let node = IrohNode::new(secret_key).await.expect("node creation failed");
        let node_id = node.node_id();
        let id_bytes = node_id.as_bytes();
        assert_ne!(id_bytes, &[0u8; 32], "node_id should not be all zeros");
        node.shutdown().await.expect("shutdown failed");
    }

    #[tokio::test]
    async fn test_two_nodes_send_receive() {
        // Create two IrohNodes with different secret keys
        let key_a = SecretKey::from_bytes(&[1u8; 32]);
        let key_b = SecretKey::from_bytes(&[2u8; 32]);

        let node_a = IrohNode::new(key_a).await.expect("node_a creation failed");
        let node_b = IrohNode::new(key_b).await.expect("node_b creation failed");

        let node_a_id = node_a.node_id();
        let node_b_id = node_b.node_id();

        // Wait for both endpoints to be online (connected to relay)
        node_a.endpoint().online().await;
        node_b.endpoint().online().await;

        let msg_to_send = WireMessage::Hello {
            public_key: [42u8; 32],
            nickname: "Alice".to_string(),
            protocol_version: 1,
        };

        // Get node_b's full address so node_a can connect without relay lookup
        let node_b_addr = node_b.endpoint().addr();

        // Node A sends Hello to Node B using send_to_addr (which now waits for stopped())
        let msg_clone = msg_to_send.clone();
        let sender = tokio::spawn(async move {
            node_a.send_to_addr(node_b_addr, &msg_clone).await.expect("send_to_addr failed");
            node_a
        });

        // Node B accepts one incoming connection and reads the message
        let (received_msg, from_id) = node_b.accept_one().await.expect("accept_one failed");

        assert_eq!(received_msg, msg_to_send, "received message should match sent message");
        assert_eq!(from_id, node_a_id, "sender should be node_a");

        // Clean up
        let node_a = sender.await.expect("sender task panicked");
        node_a.shutdown().await.expect("node_a shutdown failed");
        node_b.shutdown().await.expect("node_b shutdown failed");

        let _ = node_b_id;
    }
}
