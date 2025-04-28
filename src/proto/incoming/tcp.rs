use crate::Error;
use bytes::Buf;
use std::str;
use tracing::Level;

use super::IncomingTagHandler;

/// Enum containing possible incoming TCP packets from the roboRIO
pub enum TcpIncomingTag<'t> {
    RadioEvent(&'t str),
    UsageReport,
    DisableFaults(DisableFaults),
    RailFaults(RailFaults),
    VersionInfo(VersionInfo<'t>),
    ErrorMessage(ErrorMessage<'t>),
    Stdout(Stdout<'t>),
    Dummy,
}

pub(crate) trait IncomingTcpPacket: Sized {
    fn decode(buf: &mut impl Buf) -> Result<Self, Error>;
}

pub struct TcpTagStream<'t> {
    buf: &'t [u8],
    pos: usize,
}
impl<'t> TcpTagStream<'t> {
    #[inline(always)]
    pub const fn new(buf: &'t [u8]) -> Self {
        Self { buf, pos: 0usize }
    }
}
impl<'t> Iterator for TcpTagStream<'t> {
    type Item = TcpIncomingTag<'t>;

    fn next(&mut self) -> Option<Self::Item> {
        let buf = self.buf;
        let len = buf.len();

        if len - self.pos < 2 {
            return None;
        }

        let buf = &buf[self.pos..];

        let size = u16::from_be_bytes([buf[0], buf[1]]);
        self.pos += 2;

        if size > 0 {
            let id = buf[2];
            self.pos += 1;

            let buf = &buf[self.pos..];

            match id {
                // Radio event
                0x00 => {
                    let message = core::str::from_utf8(buf).unwrap();
                    Some(TcpIncomingTag::RadioEvent(message))
                }

                // Usage report
                0x01 => Some(TcpIncomingTag::UsageReport),

                // Disable faults
                0x04 => {
                    // 1 byte for tag id + 2*u16
                    assert_eq!(size, 5);

                    Some(TcpIncomingTag::DisableFaults(DisableFaults::parse(buf)))
                }

                // Rail faults
                0x05 => {
                    // 1 byte for tag id + 3*u16
                    assert_eq!(size, 7);

                    Some(TcpIncomingTag::RailFaults(RailFaults::parse(buf)))
                }

                // Version info
                0x0A => {
                    // 1 byte for tag id + at least 6 bytes of data
                    assert!(size >= 6);

                    Some(TcpIncomingTag::VersionInfo(VersionInfo::parse(buf)))
                }

                // Error message
                0x0B => {
                    // 1 byte for tag id + at least 19 bytes of data
                    assert!(size >= 20);

                    Some(TcpIncomingTag::ErrorMessage(ErrorMessage::parse(buf)))
                }

                // Stdout
                0x0C => {
                    // 1 byte for tag id + at least 6 bytes for message
                    assert!(size >= 7);

                    Some(TcpIncomingTag::Stdout(Stdout::parse(buf)))
                }

                // Unknown
                0x0D => {
                    assert_eq!(buf, &[0x00, 0x00, 0x04, 0x04, 0x04, 0x04]);

                    Some(TcpIncomingTag::Dummy)
                }

                _ => None,
            }
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct DisableFaults {
    comms: u16,
    pwr12v: u16,
}
impl DisableFaults {
    #[inline(always)]
    pub(crate) const fn parse(buf: &[u8]) -> Self {
        let comms = u16::from_be_bytes([buf[0], buf[1]]);
        let pwr12v = u16::from_be_bytes([buf[2], buf[3]]);

        Self { comms, pwr12v }
    }
}
impl IncomingTagHandler<'_> for DisableFaults {
    fn handle(&self, _ds: &crate::Ds) {
        event!(Level::ERROR, ?self, "A disable fault occurred");
    }
}

#[derive(Debug)]
pub struct RailFaults {
    pwr6v: u16,
    pwr5v: u16,
    pwr3_3v: u16,
}
impl RailFaults {
    #[inline(always)]
    pub(crate) const fn parse(buf: &[u8]) -> Self {
        let pwr6v = u16::from_be_bytes([buf[0], buf[1]]);
        let pwr5v = u16::from_be_bytes([buf[2], buf[3]]);
        let pwr3_3v = u16::from_be_bytes([buf[4], buf[5]]);

        Self {
            pwr6v,
            pwr5v,
            pwr3_3v,
        }
    }
}
impl IncomingTagHandler<'_> for RailFaults {
    fn handle(&self, _ds: &crate::Ds) {
        event!(Level::ERROR, ?self, "A rail fault occurred");
    }
}

pub struct VersionInfo<'v> {
    ty: u8,
    id: u8,
    name: &'v str,
    version: &'v str,
}
impl<'v> VersionInfo<'v> {
    #[inline(always)]
    pub(crate) fn parse(buf: &'v [u8]) -> Self {
        let ty = buf[0];
        let id = buf[3];
        let name_len = buf[4] as usize;
        let name = core::str::from_utf8(&buf[5..=5 + name_len]);
        let version_len = buf[6 + name_len] as usize;
        let version = core::str::from_utf8(&buf[7 + name_len..=7 + name_len + version_len]);

        Self {
            ty,
            id,
            name: if let Ok(name) = name { name } else { "" },
            version: if let Ok(version) = version {
                version
            } else {
                ""
            },
        }
    }
}
impl<'v> IncomingTagHandler<'_> for VersionInfo<'v> {
    fn handle(&self, _ds: &crate::Ds) {
        // TODO: properly share this with the library consumer
        event!(
            Level::INFO,
            r#type = self.ty,
            id = self.id,
            name = self.name,
            version = self.version
        );
    }
}

pub struct ErrorMessage<'e> {
    timestamp: f32,
    seqnum: u16,
    error_code: i32,
    // TODO: bitflags
    flags: ErrorMsgFlags,
    details: &'e str,
    location: &'e str,
    call_stack: &'e str,
}
impl<'e> ErrorMessage<'e> {
    #[inline(always)]
    pub(crate) fn parse(buf: &'e [u8]) -> Self {
        let timestamp = f32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let seqnum = u16::from_be_bytes([buf[4], buf[5]]);
        let error_code = i32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let flags = if let Some(flags) = ErrorMsgFlags::from_bits(buf[12]) {
            flags
        } else {
            ErrorMsgFlags::empty()
        };
        let details_len = u16::from_be_bytes([buf[13], buf[14]]) as usize;
        let details = if let Ok(details) = core::str::from_utf8(&buf[15..15 + details_len]) {
            details
        } else {
            ""
        };
        let location_len =
            u16::from_be_bytes([buf[15 + details_len], buf[16 + details_len]]) as usize;
        let location = if let Ok(location) =
            core::str::from_utf8(&buf[17 + details_len..17 + details_len + location_len])
        {
            location
        } else {
            ""
        };
        let call_stack_len = u16::from_be_bytes([
            buf[17 + details_len + location_len],
            buf[18 + details_len + location_len],
        ]) as usize;
        let call_stack = if let Ok(call_stack) = core::str::from_utf8(
            &buf[19 + details_len + location_len..19 + details_len + location_len + call_stack_len],
        ) {
            call_stack
        } else {
            ""
        };

        Self {
            timestamp,
            seqnum,
            error_code,
            flags,
            details,
            location,
            call_stack,
        }
    }
}
impl<'e> IncomingTagHandler<'_> for ErrorMessage<'e> {
    fn handle(&self, _ds: &crate::Ds) {
        if self.flags.contains(ErrorMsgFlags::ERROR) {
            event!(
                Level::ERROR,
                timestamp = self.timestamp,
                seqnum = self.seqnum,
                error_code = self.seqnum,
                details = self.details,
                location = self.location,
                call_stack = self.call_stack
            );
        } else {
            event!(
                Level::WARN,
                timestamp = self.timestamp,
                seqnum = self.seqnum,
                error_code = self.seqnum,
                details = self.details,
                location = self.location,
                call_stack = self.call_stack
            );
        }
    }
}

bitflags! {
    pub struct ErrorMsgFlags: u8 {
        const ERROR      = 0b0000_0001;
        const IS_LV_CODE = 0b0000_0010;
    }
}

pub struct Stdout<'s> {
    timestamp: f32,
    seqnum: u16,
    message: &'s str,
}
impl<'s> Stdout<'s> {
    #[inline(always)]
    pub(crate) fn parse(buf: &'s [u8]) -> Self {
        let timestamp = f32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let seqnum = u16::from_be_bytes([buf[4], buf[5]]);
        let message = if let Ok(message) = core::str::from_utf8(&buf[6..]) {
            message
        } else {
            ""
        };

        Self {
            timestamp,
            seqnum,
            message,
        }
    }
}
impl<'s> IncomingTagHandler<'_> for Stdout<'s> {
    fn handle(&self, _ds: &crate::Ds) {
        event!(
            Level::INFO,
            self.message,
            timestamp = self.timestamp,
            seqnum = self.seqnum
        );
    }
}
