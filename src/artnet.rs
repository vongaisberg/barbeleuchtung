#![allow(dead_code)]

use std::convert::TryInto;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

use artnet_protocol::{ArtCommand, Output};

use crate::fixture::UniverseBuffer;

/// Art-Net UDP broadcaster. Wraps a bound socket and the broadcast destination.
pub struct ArtNetSender {
    socket: UdpSocket,
    dest: SocketAddr,
    /// Per-universe sequence counters (wraps 1..=255, 0 = disabled).
    sequences: Vec<u8>,
}

impl ArtNetSender {
    /// Bind to `0.0.0.0:0` (ephemeral port) and set up broadcast.
    pub fn new() -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_broadcast(true)?;
        let dest = ("10.255.255.255", 6454u16)
            .to_socket_addrs()?
            .next()
            .expect("failed to resolve broadcast address");
        Ok(Self {
            socket,
            dest,
            sequences: Vec::new(),
        })
    }

    /// Send a single universe buffer via Art-Net.
    ///
    /// `universe` is the 0-based Art-Net universe number.
    pub fn send_universe(&mut self, universe: u16, buf: &UniverseBuffer) -> std::io::Result<()> {
        // Grow sequence table as needed.
        if self.sequences.len() <= universe as usize {
            self.sequences.resize(universe as usize + 1, 0);
        }

        // Advance sequence counter (1..=255, then wraps to 1).
        let seq = &mut self.sequences[universe as usize];
        *seq = if *seq >= 255 { 1 } else { *seq + 1 };

        // Art-Net port-address is a 15-bit value; for simple single-net use,
        // universe 0..=15 maps directly to port-address 0..=15.
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

        self.socket.send_to(&bytes, self.dest)?;
        Ok(())
    }

    /// Send all universes in the map. `universe_buffers` is indexed by
    /// universe number; only entries where `Some` are transmitted.
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
