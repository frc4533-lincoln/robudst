use crate::{AlliancePos, Ds, RobotCodeMode, RobotStatus};

pub struct UdpOutgoingPacket<'u> {
    seqnum: u16,
    comm_version: u8,
    control: Control,
    req: Request,
    alliance: AlliancePos,
    tags: &'u [UdpOutgoingTag<'u>],
}
impl UdpOutgoingPacket<'_> {
    pub fn build(ds: &Ds) -> Self {
        let mut control = Control::empty();

        match ds.status.load() {
            RobotStatus::EStopped => {
                control |= Control::ESTOP;
            }
            RobotStatus::Enabled => {
                control |= Control::ENABLED;
            }
            _ => {}
        }
        match ds.mode.load() {
            RobotCodeMode::Teleop => {
                control |= Control::TELEOP;
            }
            RobotCodeMode::Autonomous => {
                control |= Control::AUTO;
            }
            RobotCodeMode::Test => {
                control |= Control::TEST;
            }
        }

        let alliance = ds.alliance_pos.load();

        Self {
            seqnum: 0,
            comm_version: 0x01,
            control,
            req: Request::empty(),
            alliance,
            tags: &[],
        }
    }

    pub(crate) const fn reboot_rio(&mut self) {
        self.req = Request::REBOOT_RIO;
    }

    pub(crate) const fn restart_code(&mut self) {
        self.req = Request::RESTART_CODE;
    }

    pub(crate) fn write(self) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::new();
        buf.clear();

        buf.extend(self.seqnum.to_be_bytes().to_vec());
        buf.push(self.comm_version);
        buf.push(self.control.bits());
        buf.push(self.req.bits());
        buf.push(self.alliance.to_pos());
        
        for tag in self.tags {
            match tag {
                UdpOutgoingTag::Countdown { countdown: _ } => {
                    let tag = tag.write();
                    buf.extend_from_slice(&[tag.len() as u8, 0x07]);
                    buf.extend(tag);
                }
                UdpOutgoingTag::Joystick { axes: _, buttons: _, povs: _ } => {
                    let tag = tag.write();
                    buf.extend_from_slice(&[tag.len() as u8, 0x0C]);
                    buf.extend(tag);
                }
                UdpOutgoingTag::Date { microseconds: _, second: _, minute: _, hour: _, day: _, month: _, year: _ } => {
                    let tag = tag.write();
                    buf.extend_from_slice(&[tag.len() as u8, 0x0F]);
                    buf.extend(tag);
                }
                UdpOutgoingTag::Timezone { timezone: _ } => {
                    let tag = tag.write();
                    buf.extend_from_slice(&[tag.len() as u8, 0x10]);
                    buf.extend(tag);
                }
                _ => {}
            }
        }

        buf
    }
}

bitflags! {
    pub struct Control: u8 {
        const ESTOP         = 0b1000_0000;
        const FMS_CONNECTED = 0b0000_1000;
        const ENABLED       = 0b0000_0100;

        const TELEOP = 0b00;
        const AUTO   = 0b10;
        const TEST   = 0b01;
    }

    pub struct Request: u8 {
        const REBOOT_RIO   = 0b0000_1000;
        const RESTART_CODE = 0b0000_0100;
    }
}

pub enum UdpOutgoingTag<'u> {
    Countdown {
        countdown: f32,
    },
    Joystick {
        axes: &'u [i8],
        buttons: &'u [bool],
        povs: &'u [i16],
    },
    Date {
        microseconds: u32,
        second: u8,
        minute: u8,
        hour: u8,
        day: u8,
        month: u8,
        year: u8,
    },
    Timezone {
        timezone: &'u str,
    },
}
impl<'u> UdpOutgoingTag<'u> {
    pub fn write(&self) -> Vec<u8> {
        match self {
            UdpOutgoingTag::Countdown { countdown } => {
                countdown.to_be_bytes().to_vec()
            }
            UdpOutgoingTag::Joystick { axes, buttons, povs } => {
                let mut buf = Vec::new();
                buf.clear();

                buf.push(axes.len() as u8);
                buf.extend(axes.iter().map(|axis| *axis as u8));

                // Each button's state is a binary value, packed in little endian byte order
                buf.push((buttons.len() / 8) as u8 + if buttons.len() == 0 { 0 } else { 1 });
                for btn_chunk in buttons.array_chunks::<8>() {
                    let mut byte = 0u8;
                    for button in btn_chunk {
                        byte |= if *button { 1 } else { 0 };
                        byte <<= 1;
                    }
                    buf.push(byte);
                }

                buf.push(povs.len() as u8);
                buf.extend(povs.iter().map(|pov| pov.to_be_bytes()).flatten());

                buf
            }
            UdpOutgoingTag::Date { microseconds, second, minute, hour, day, month, year } => {
                Vec::new()
            }
            UdpOutgoingTag::Timezone { timezone } => {
                timezone.as_bytes().to_vec()
            }
        }
    }
}
