#![feature(array_chunks)]

use std::{net::Ipv4Addr, sync::Arc};

use crossbeam_utils::atomic::AtomicCell;
use futures_lite::{Stream, StreamExt};
use proto::{incoming::{tcp::{TcpIncomingTag, TcpTagStream}, udp::{Status, UdpIncomingPacket, UdpIncomingStream}, IncomingTagHandler}, outgoing::{tcp::TcpOutgoingTag, udp::UdpOutgoingPacket}};
use tokio::{net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, unix::SocketAddr, TcpStream, UdpSocket}, sync::Mutex};
use utils::{find_status, gen_team_ip};

#[macro_use]
extern crate tracing;
#[macro_use]
extern crate bitflags;
extern crate tokio;
extern crate futures_lite;
extern crate crossbeam_utils;

pub mod proto;
mod utils;

pub enum Error {
}

#[derive(Clone, Copy)]
pub enum RobotStatus {
    NoCommunication,
    NoRobotCode,
    EStopped,
    BrownedOut,
    Disabled,
    Enabled,
}

#[derive(Clone, Copy)]
pub enum RobotCodeMode {
    Autonomous,
    Teleop,
    Test,
}

/// The position and alliance of the driver station
///
/// Position can be `1`, `2`, or `3`
#[derive(Clone, Copy)]
pub enum AlliancePos {
    Red(u8),
    Blue(u8),
}
impl AlliancePos {
    const fn to_pos(self) -> u8 {
        match self {
            Self::Red(pos) => {
                assert!(pos != 0 && pos <= 3);
                pos.saturating_sub(1)
            },
            Self::Blue(pos) => {
                assert!(pos != 0 && pos <= 3);
                pos.saturating_sub(4)
            },
        }
    }
}

/// A driver station instance
pub struct Ds {
    status: AtomicCell<RobotStatus>,
    mode: AtomicCell<RobotCodeMode>,
    can_bus_util: AtomicCell<f32>,
    battery: AtomicCell<f32>,
    alliance_pos: AtomicCell<AlliancePos>,
    //

    rio_tcp_rx: Arc<Mutex<OwnedReadHalf>>,
    rio_tcp_tx: Arc<Mutex<OwnedWriteHalf>>,
    rio_incoming_udp: Arc<Mutex<UdpSocket>>,
    rio_outgoing_udp: Arc<Mutex<UdpSocket>>,

}
impl Ds {
    pub async fn init(team_number: u16) -> Self {
        let rio_ip = gen_team_ip(team_number).unwrap();

        let (rio_tcp_rx, rio_tcp_tx) = TcpStream::connect(format!("{rio_ip}:1150")).await.unwrap().into_split();
        let rio_incoming_udp = UdpSocket::bind("0.0.0.0:1150").await.unwrap();
        let rio_outgoing_udp = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        rio_outgoing_udp.connect(format!("{rio_ip}:1110")).await.unwrap();

        Ds {
            status: AtomicCell::new(RobotStatus::NoCommunication),
            mode: AtomicCell::new(RobotCodeMode::Teleop),
            can_bus_util: AtomicCell::new(0.0),
            battery: AtomicCell::new(0.0),
            alliance_pos: AtomicCell::new(AlliancePos::Red(1)),

            rio_tcp_rx: Arc::new(Mutex::new(rio_tcp_rx)),
            rio_tcp_tx: Arc::new(Mutex::new(rio_tcp_tx)),
            rio_incoming_udp: Arc::new(Mutex::new(rio_incoming_udp)),
            rio_outgoing_udp: Arc::new(Mutex::new(rio_outgoing_udp)),
        }
    }

    /// Get robot status
    #[inline(always)]
    pub fn status(&self) -> RobotStatus {
        self.status.load()
    }

    /// Get robot code mode
    #[inline(always)]
    pub fn mode(&self) -> RobotCodeMode {
        self.mode.load()
    }

    /// Get CAN bus utilization (as percentage)
    #[inline(always)]
    pub fn can_bus_util(&self) -> f32 {
        self.can_bus_util.load()
    }

    /// Enable the robot code
    pub async fn enable(&self) {
        self.status.store(RobotStatus::Enabled);
        self.send_udp().await;
    }

    /// Disable the robot code
    pub async fn disable(&self) {
        self.status.store(RobotStatus::Disabled);
        self.send_udp().await;
    }

    /// Trigger an emergency stop
    pub async fn estop(&self) {
        self.status.store(RobotStatus::EStopped);
        self.send_udp().await;
    }

    /// Issue a command to restart the roboRIO
    pub async fn reboot_rio(&self) {
        let mut pkt = UdpOutgoingPacket::build(self);
        pkt.reboot_rio();
        self.rio_outgoing_udp.lock().await.send(&pkt.write()).await.unwrap();
    }

    /// Issue a command to restart the robot code
    pub async fn restart_code(&self) {
        let mut pkt = UdpOutgoingPacket::build(self);
        pkt.restart_code();
        self.rio_outgoing_udp.lock().await.send(&pkt.write()).await.unwrap();
    }

    async fn send_udp(&self) {
        self.rio_outgoing_udp.lock().await.send(&UdpOutgoingPacket::build(self).write()).await.unwrap();
    }

    async fn send_tcp(&self, tag: TcpOutgoingTag<'_>) {
        let tcp_tx = self.rio_tcp_tx.lock().await;
        tcp_tx.writable().await.unwrap();
        tcp_tx.try_write(&tag.write()).unwrap();
    }

    pub async fn run(&self) {
        let udp_rx = self.rio_incoming_udp.lock().await;
        let tcp_rx = self.rio_tcp_rx.lock().await;

        loop {
            tokio::select! {
                res = udp_rx.readable() => {
                    res.unwrap();

                    let mut buf = Vec::new();
                    buf.clear();

                    if let Err(err) = udp_rx.try_recv(&mut buf) {
                        panic!("{err:?}");
                    }

                    for pkt in UdpIncomingStream::new(&buf) {
                        let UdpIncomingPacket { status, trace, battery, .. } = pkt;

                        let (status, mode) = find_status(status, trace);

                        self.status.store(status);
                        self.mode.store(mode);
                        self.battery.store(battery);
                    }
                }
                res = tcp_rx.readable() => {
                    res.unwrap();

                    let mut buf = Vec::new();
                    buf.clear();

                    if let Err(err) = tcp_rx.try_read_buf(&mut buf) {
                        panic!("{err:?}");
                    }

                    for tag in TcpTagStream::new(&buf) {
                        match tag {
                            TcpIncomingTag::RadioEvent(tag) => {},
                            TcpIncomingTag::UsageReport => {},
                            TcpIncomingTag::DisableFaults(tag) => tag.handle(self),
                            TcpIncomingTag::RailFaults(tag) => tag.handle(self),
                            TcpIncomingTag::VersionInfo(tag) => tag.handle(self),
                            TcpIncomingTag::ErrorMessage(tag) => tag.handle(self),
                            TcpIncomingTag::Stdout(tag) => tag.handle(self),
                            TcpIncomingTag::Dummy => {},
                        }
                    }
                }
            }
        }
    }
}
