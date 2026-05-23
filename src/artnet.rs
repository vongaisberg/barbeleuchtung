#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use artnet_protocol::{ArtCommand, Output, Poll, PollReply};

use crate::fixture::UniverseBuffer;

// ─── Subscriber table (Art-Net 4 unicast subscription) ──────────────────────

/// Per-node entry built from ArtPollReply.
#[derive(Debug)]
struct NodeEntry {
    addr: SocketAddr,
    universes: HashSet<u16>,
    last_seen: Instant,
}

/// Tracks which nodes subscribe to which universes. Updated by the discovery
/// thread from ArtPollReply packets; read by the engine when unicasting ArtDmx.
#[derive(Default)]
pub struct SubscriberTable {
    nodes: HashMap<IpAddr, NodeEntry>,
    /// Derived cache: universe -> list of socket addrs to unicast to.
    by_universe: HashMap<u16, Vec<SocketAddr>>,
}

impl SubscriberTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a node from an ArtPollReply. Extracts subscribed universes from
    /// port_address, port_types, and swout per Art-Net 4 spec.
    pub fn add_from_poll_reply(&mut self, reply: &PollReply) {
        use std::net::IpAddr::V4;
        let addr = SocketAddr::new(V4(reply.address), reply.port);
        let ip = IpAddr::V4(reply.address);

        let net = (reply.port_address[0] & 0x7F) as u16;  // bits 14-8
        let sub = (reply.port_address[1] & 0x0F) as u16;  // bits 7-4

        let num_ports = reply.num_ports[1].min(4);
        let mut universes = HashSet::new();

        for i in 0..num_ports {
            let idx = i as usize;
            // port_types[i] bit 7: port can output from Art-Net (subscribes)
            if (reply.port_types.get(idx).copied().unwrap_or(0) & 0x80) != 0 {
                let uni = (reply.swout.get(idx).copied().unwrap_or(0) & 0x0F) as u16;
                let full = (net << 8) | (sub << 4) | uni;
                universes.insert(full);
            }
        }

        let now = Instant::now();
        let is_new = !self.nodes.contains_key(&ip);
        let old_universes = self.nodes.get(&ip).map(|e| e.universes.clone());
        self.nodes.insert(ip, NodeEntry {
            addr,
            universes: universes.clone(),
            last_seen: now,
        });
        let uni_list: Vec<_> = universes.iter().cloned().collect();
        if is_new {
            log::info!(
                "Art-Net: receiver added {} (universes {:?})",
                addr,
                uni_list
            );
        } else if let Some(ref old) = old_universes {
            if old != &universes {
                log::info!(
                    "Art-Net: receiver {} universes changed {:?} -> {:?}",
                    addr,
                    old,
                    uni_list
                );
            }
        }
        self.rebuild_by_universe();
    }

    /// Remove nodes not seen for longer than `stale_threshold`.
    pub fn purge_stale(&mut self, stale_threshold: Duration) {
        let now = Instant::now();
        let to_remove: Vec<(IpAddr, SocketAddr)> = self
            .nodes
            .iter()
            .filter(|(_, e)| now.duration_since(e.last_seen) >= stale_threshold)
            .map(|(ip, e)| (*ip, e.addr))
            .collect();
        for (_, addr) in &to_remove {
            log::info!("Art-Net: receiver removed {} (timed out)", addr);
        }
        let remove_ips: HashSet<_> = to_remove.into_iter().map(|(ip, _)| ip).collect();
        self.nodes.retain(|ip, _| !remove_ips.contains(ip));
        self.rebuild_by_universe();
    }

    fn rebuild_by_universe(&mut self) {
        self.by_universe.clear();
        for entry in self.nodes.values() {
            for &u in &entry.universes {
                self.by_universe
                    .entry(u)
                    .or_default()
                    .push(entry.addr);
            }
        }
    }

    /// Get subscribers for a given universe (port-address). Returns empty if none.
    pub fn subscribers(&self, universe: u16) -> &[SocketAddr] {
        static EMPTY: &[SocketAddr] = &[];
        self.by_universe
            .get(&universe)
            .map(|v| v.as_slice())
            .unwrap_or(EMPTY)
    }
}

// ─── Discovery loop ─────────────────────────────────────────────────────────

const ARTNET_PORT: u16 = 6454;
const POLL_INTERVAL: Duration = Duration::from_millis(2500);
const POLL_RECV_TIMEOUT: Duration = Duration::from_secs(3);
const STALE_NODE_THRESHOLD: Duration = Duration::from_secs(60);

/// Run the Art-Net discovery loop in the current thread. Sends ArtPoll
/// periodically, parses ArtPollReply, and updates the shared subscriber table.
pub fn run_discovery(
    socket: UdpSocket,
    subscribers: std::sync::Arc<RwLock<SubscriberTable>>,
    broadcast_addr: SocketAddr,
) {
    let poll_bytes = ArtCommand::Poll(Poll::default())
        .write_to_buffer()
        .expect("Poll serialization");

    loop {
        let loop_start = Instant::now();

        // Broadcast ArtPoll
        if let Err(e) = socket.send_to(&poll_bytes, broadcast_addr) {
            log::warn!("Art-Net discovery: failed to send ArtPoll: {e}");
        }

        // Receive ArtPollReply packets until timeout
        socket
            .set_read_timeout(Some(POLL_RECV_TIMEOUT))
            .expect("set read timeout");

        let mut buf = [0u8; 1024];
        loop {
            match socket.recv_from(&mut buf) {
                Ok((len, _src)) => {
                    match ArtCommand::from_buffer(&buf[..len]) {
                        Ok(ArtCommand::PollReply(reply)) => {
                            let mut table = subscribers.write().expect("subscriber table lock");
                            table.add_from_poll_reply(&reply);
                        }
                        Ok(ArtCommand::Poll(_)) => {
                            // Our own broadcast or another controller; ignore
                        }
                        _ => {}
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) => {
                    log::warn!("Art-Net discovery: recv error: {e}");
                    break;
                }
            }
        }

        // Purge stale nodes
        {
            let mut table = subscribers.write().expect("subscriber table lock");
            table.purge_stale(STALE_NODE_THRESHOLD);
        }

        // Sleep remainder of poll interval
        let elapsed = loop_start.elapsed();
        if elapsed < POLL_INTERVAL {
            std::thread::sleep(POLL_INTERVAL - elapsed);
        }
    }
}

// ─── Art-Net sender (unicast with broadcast fallback) ────────────────────────

/// Art-Net controller. Unicasts ArtDmx to subscribers discovered via ArtPoll.
/// Falls back to broadcast if no subscribers exist for a universe.
pub struct ArtNetSender {
    socket: UdpSocket,
    broadcast_addr: SocketAddr,
    subscribers: std::sync::Arc<RwLock<SubscriberTable>>,
    /// Per-universe sequence counters (wraps 1..=255, 0 = disabled).
    sequences: Vec<u8>,
}

impl ArtNetSender {
    /// Create sender with shared socket and subscriber table.
    pub fn new(
        socket: UdpSocket,
        subscribers: std::sync::Arc<RwLock<SubscriberTable>>,
    ) -> std::io::Result<Self> {
        socket.set_broadcast(true)?;
        let broadcast_addr = ("10.255.255.255", ARTNET_PORT)
            .to_socket_addrs()?
            .next()
            .expect("failed to resolve broadcast address");

        Ok(Self {
            socket,
            broadcast_addr,
            subscribers,
            sequences: Vec::new(),
        })
    }

    /// Send a single universe buffer via Art-Net.
    /// Unicasts to subscribers; broadcasts if no subscribers.
    pub fn send_universe(&mut self, universe: u16, buf: &UniverseBuffer) -> std::io::Result<()> {
        if self.sequences.len() <= universe as usize {
            self.sequences.resize(universe as usize + 1, 0);
        }

        let seq = &mut self.sequences[universe as usize];
        *seq = if *seq >= 255 { 1 } else { *seq + 1 };

        let port_address = universe
            .try_into()
            .unwrap_or_else(|_| (universe & 0x7FFF).try_into().unwrap());

        let command = ArtCommand::Output(Output {
            sequence: *seq,
            port_address,
            data: buf.to_vec().into(),
            ..Output::default()
        });

        let bytes = command
            .write_to_buffer()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e:?}")))?;

        // Copy addresses and release lock before I/O to avoid blocking discovery.
        let addrs: Vec<SocketAddr> = {
            let subs = self.subscribers.read().expect("subscriber table lock");
            subs.subscribers(universe).to_vec()
        };

        if addrs.is_empty() {
            self.socket.send_to(&bytes, self.broadcast_addr)?;
        } else {
            for addr in addrs {
                if let Err(e) = self.socket.send_to(&bytes, addr) {
                    log::warn!("Art-Net unicast to {addr}: {e}");
                }
            }
        }
        Ok(())
    }

    /// Send all universes in the map.
    pub fn send_all(
        &mut self,
        universe_buffers: &[Option<Box<UniverseBuffer>>],
    ) -> std::io::Result<()> {
        for (idx, buf_opt) in universe_buffers.iter().enumerate() {
            if let Some(buf) = buf_opt {
                self.send_universe(idx as u16, buf.as_ref())?;
            }
        }
        Ok(())
    }
}
