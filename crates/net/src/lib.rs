//! Rework p2p transport over iroh.
//!
//! Endpoints are Ed25519 keys: dial keys, not IPs. QUIC with UDP hole
//! punching plus encrypted relay fallback. One ALPN per protocol.

use iroh::endpoint::{Connection, RecvStream, SendStream};
use iroh::{Endpoint, EndpointAddr, EndpointId, SecretKey};
use thiserror::Error;

pub use iroh::endpoint::presets::N0 as DefaultPreset;

/// Analysis protocol: length-prefixed request/response frames.
pub const ALPN_ANALYSIS: &[u8] = b"rework/analysis/0";
/// Echo protocol: length-prefixed roundtrip (connectivity probe).
pub const ALPN_ECHO: &[u8] = b"rework/echo/0";

/// Max single frame: 64 MiB. Larger payloads go by reference + paging.
pub const MAX_FRAME: usize = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum Error {
    #[error("bind failed: {0}")]
    Bind(String),
    #[error("connect to {peer} failed: {message}")]
    Connect { peer: String, message: String },
    #[error("accept failed: {0}")]
    Accept(String),
    #[error("stream error: {0}")]
    Stream(String),
    #[error("frame too large: {0} bytes (max {MAX_FRAME})")]
    FrameTooLarge(usize),
    #[error("peer closed the connection")]
    Closed,
}

/// An iroh endpoint owned by this process (app or VM daemon).
#[derive(Debug, Clone)]
pub struct Node {
    endpoint: Endpoint,
}

impl Node {
    /// Bind an ephemeral endpoint (fresh random key) on the default preset
    /// (n0 relay + discovery). Good for tests and zero-config first run.
    pub async fn bind_ephemeral() -> Result<Self, Error> {
        let endpoint = Endpoint::bind(DefaultPreset)
            .await
            .map_err(|e| Error::Bind(e.to_string()))?;
        Ok(Self { endpoint })
    }

    /// Bind with a stable 32-byte secret (persist it to keep your node id).
    pub async fn bind_with_secret(secret: &[u8; 32]) -> Result<Self, Error> {
        let key = SecretKey::from_bytes(secret);
        let endpoint = Endpoint::builder(DefaultPreset)
            .secret_key(key)
            .bind()
            .await
            .map_err(|e| Error::Bind(e.to_string()))?;
        Ok(Self { endpoint })
    }

    /// This node's public id (the dialable key).
    pub fn id(&self) -> EndpointId {
        self.endpoint.id()
    }

    /// This node's current address (id + relay + direct addrs).
    pub fn addr(&self) -> EndpointAddr {
        self.endpoint.addr()
    }

    /// Short id for UI/log lines.
    pub fn short_id(&self) -> String {
        self.id().to_string().chars().take(12).collect()
    }

    /// Connect to a peer's address on one ALPN.
    pub async fn connect(&self, addr: EndpointAddr, alpn: &[u8]) -> Result<Connection, Error> {
        let peer = addr.id.to_string();
        self.endpoint
            .connect(addr, alpn)
            .await
            .map_err(|e| Error::Connect {
                peer,
                message: e.to_string(),
            })
    }

    /// Wait for the next incoming connection; `None` if the endpoint closed.
    pub async fn accept(&self) -> Option<Incoming> {
        self.endpoint.accept().await.map(Incoming)
    }

    /// Shut the endpoint down.
    pub async fn close(&self) {
        self.endpoint.close().await;
    }
}

/// An accepted-but-not-yet-handshaked incoming connection.
pub struct Incoming(iroh::endpoint::Incoming);

impl Incoming {
    pub async fn accept(self) -> Result<Connection, Error> {
        let accepting = self.0.accept().map_err(|e| Error::Accept(e.to_string()))?;
        accepting.await.map_err(|e| Error::Accept(e.to_string()))
    }

    pub fn refuse(self) {
        self.0.refuse();
    }
}

/// Write one length-prefixed frame (u32 big-endian len + bytes).
pub async fn write_frame(send: &mut SendStream, bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > MAX_FRAME {
        return Err(Error::FrameTooLarge(bytes.len()));
    }
    let len = (bytes.len() as u32).to_be_bytes();
    send.write_all(&len)
        .await
        .map_err(|e| Error::Stream(e.to_string()))?;
    send.write_all(bytes)
        .await
        .map_err(|e| Error::Stream(e.to_string()))?;
    Ok(())
}

/// Read one length-prefixed frame.
pub async fn read_frame(recv: &mut RecvStream) -> Result<Vec<u8>, Error> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf)
        .await
        .map_err(|e| Error::Stream(e.to_string()))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME {
        return Err(Error::FrameTooLarge(len));
    }
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf)
        .await
        .map_err(|e| Error::Stream(e.to_string()))?;
    Ok(buf)
}

/// Echo roundtrip against a peer: open a stream, send, read the reply.
pub async fn echo_roundtrip(node: &Node, peer: EndpointAddr, msg: &[u8]) -> Result<Vec<u8>, Error> {
    let conn = node.connect(peer, ALPN_ECHO).await?;
    let (mut send, mut recv) = conn
        .open_bi()
        .await
        .map_err(|e| Error::Stream(e.to_string()))?;
    write_frame(&mut send, msg).await?;
    send.finish().map_err(|e| Error::Stream(e.to_string()))?;
    let reply = read_frame(&mut recv).await?;
    Ok(reply)
}

/// Serve the echo protocol until `shutdown` fires: accept connections,
/// read one frame per stream, write it back.
pub async fn serve_echo(
    node: Node,
    mut shutdown: tokio::sync::oneshot::Receiver<()>,
) -> Result<(), Error> {
    loop {
        tokio::select! {
            _ = &mut shutdown => return Ok(()),
            incoming = node.accept() => {
                let Some(incoming) = incoming else { return Ok(()) };
                let conn = match incoming.accept().await {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                tokio::spawn(async move {
                    while let Ok((mut send, mut recv)) = conn.accept_bi().await {
                        match read_frame(&mut recv).await {
                            Ok(msg) => {
                                if write_frame(&mut send, &msg).await.is_err() {
                                    break;
                                }
                                let _ = send.finish();
                            }
                            Err(_) => break,
                        }
                    }
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    #[test]
    fn alpns_are_stable() {
        assert_eq!(ALPN_ECHO, b"rework/echo/0");
        assert_eq!(ALPN_ANALYSIS, b"rework/analysis/0");
    }

    /// Requires UDP socket bind (blocked in sandboxed shells; runs in CI
    /// via `cargo test -p rework-net -- --include-ignored`).
    #[tokio::test]
    #[ignore]
    async fn stable_secret_gives_stable_id() {
        let secret = [7u8; 32];
        let a = Node::bind_with_secret(&secret).await.unwrap();
        let b = Node::bind_with_secret(&secret).await.unwrap();
        assert_eq!(a.id(), b.id());
        a.close().await;
        b.close().await;
    }

    /// Full QUIC roundtrip between two local nodes (direct path, no relay needed).
    #[tokio::test]
    #[ignore]
    async fn echo_roundtrip_loopback() {
        let server = Node::bind_ephemeral().await.unwrap();
        let client = Node::bind_ephemeral().await.unwrap();
        let (tx, rx) = oneshot::channel();
        let srv = server.clone();
        let serve = tokio::spawn(async move { serve_echo(srv, rx).await });

        let peer = server.addr();
        let reply = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            echo_roundtrip(&client, peer, b"hello-rework"),
        )
        .await
        .expect("echo timed out")
        .expect("echo failed");
        assert_eq!(reply, b"hello-rework");

        let _ = tx.send(());
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), serve).await;
        server.close().await;
        client.close().await;
    }
}
