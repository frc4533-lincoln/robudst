use super::IncomingTagHandler;

pub(crate) struct UdpIncomingPacket {
    pub seqnum: u16,
    pub status: Status,
    pub trace: Trace,
    pub battery: f32,
    pub need_date: bool,
}

pub(crate) struct UdpIncomingStream<'u> {
    buf: &'u [u8],
    pos: usize,
}
impl<'u> UdpIncomingStream<'u> {
    #[inline(always)]
    pub const fn new(buf: &'u [u8]) -> Self {
        Self {
            buf,
            pos: 0usize,
        }
    }
    pub fn parse_one(buf: &'u [u8]) -> UdpIncomingPacket {
        Self::new(buf).next().unwrap()
    }
}
impl Iterator for UdpIncomingStream<'_> {
    type Item = UdpIncomingPacket;

    fn next(&mut self) -> Option<Self::Item> {
        let buf = self.buf;
        let len = buf.len();

        // Verify there's at least 8 bytes (for the static fields)
        if len-self.pos <= 8 {
            return None;
        }

        // Get a slice that starts at the cursor pos, so impl is cleaner
        let buf = &buf[self.pos..];

        // Get values for each of the fields, then advance cursor pos by 8
        let seqnum = u16::from_be_bytes([buf[0], buf[1]]);
        let _comm_version = buf[2];
        let status = Status::from_bits(buf[3]).unwrap();
        let trace = Trace::from_bits(buf[4]).unwrap();
        let battery = (buf[5] as f32 + buf[6] as f32) / 256.0;
        let need_date = buf[7] == 1;
        self.pos += 8;

        while self.pos < len {
            let tag_size = buf[self.pos];
            let tag_id = buf[self.pos+1];
            self.pos += 2;

            if (self.pos + tag_size as usize) < len {
                return None;
            }
            let buf = &buf[self.pos..self.pos+tag_size as usize];
            self.pos += tag_size as usize;

            match tag_id {
                // Joystick output
                0x01 => {
                    if tag_size == 1 {
                        continue;
                    }
                    // 1 byte for tag id + 8 bytes of data
                    assert_eq!(tag_size, 9);

                    JoystickOutput::parse(buf);
                }

                // Disk space
                0x04 => {
                    let _free_disk = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
                }

                // CPU stats
                0x05 => {
                    // 1 byte for tag id + 5*f32
                    assert_eq!(tag_size, 21);

                    CpuInfo::parse(buf);
                }

                // RAM stats
                0x06 => {
                    // 1 byte for tag id + 2*u32
                    assert_eq!(tag_size, 9);

                    RamInfo::parse(buf);
                }

                // PDP log
                0x08 => {
                    // 1 byte for tag id + 25 bytes of stuff I'd rather not deal with at the moment
                    assert_eq!(tag_size, 26);
                }

                // Unknown
                0x09 => {
                    // 1 byte for tag id + 9 bytes of who knows what
                    assert_eq!(tag_size, 10);
                }
                // CAN metrics
                0x0E => {
                    // 1 byte for tag id + f32 + 2*u32 + 2*u8
                    assert_eq!(tag_size, 15);

                    CanMetrics::parse(buf);
                }
                _ => {
                }
            }
        }

        Some(UdpIncomingPacket { seqnum, status, trace, battery, need_date })
    }
}

pub(crate) enum UdpIncomingTag {
    JoystickOutput(JoystickOutput),
    DiskSpace(usize),
    CpuInfo(CpuInfo),
    RamInfo(RamInfo),
    CanMetrics(CanMetrics),
}

pub(crate) struct JoystickOutput {
    outputs: u32,
    left_rumble: u16,
    right_rumble: u16,
}
impl JoystickOutput {
    #[inline(always)]
    pub(crate) const fn parse(buf: &[u8]) -> Self {
        let outputs = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let left_rumble = u16::from_be_bytes([buf[4], buf[5]]);
        let right_rumble = u16::from_be_bytes([buf[6], buf[7]]);

        JoystickOutput {
            outputs,
            left_rumble,
            right_rumble,
        }
    }
}
impl IncomingTagHandler<'_> for JoystickOutput {
    fn handle(&self, ds: &'_ crate::Ds) {
        //
    }
}

pub(crate) struct CpuInfo {
    num_of_cpus: f32,
    cpu_time_critical: f32,
    cpu_above_normal: f32,
    cpu_normal: f32,
    cpu_low: f32,
}
impl CpuInfo {
    #[inline(always)]
    pub(crate) const fn parse(buf: &[u8]) -> Self {
        let num_of_cpus = f32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let cpu_time_critical = f32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let cpu_above_normal = f32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let cpu_normal = f32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]);
        let cpu_low = f32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]);

        Self {
            num_of_cpus,
            cpu_time_critical,
            cpu_above_normal,
            cpu_normal,
            cpu_low,
        }
    }
}

pub(crate) struct RamInfo {
    block: u32,
    free_space: u32,
}
impl RamInfo {
    #[inline(always)]
    pub(crate) const fn parse(buf: &[u8]) -> Self {
        let block = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let free_space = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);

        Self {
            block,
            free_space,
        }
    }
}

pub(crate) struct CanMetrics {
    utilization: f32,
    bus_off: u32,
    tx_full: u32,
    rx_errors: u8,
    tx_errors: u8,
}
impl CanMetrics {
    #[inline(always)]
    pub(crate) const fn parse(buf: &[u8]) -> Self {
        let utilization = f32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let bus_off = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let tx_full = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let rx_errors = buf[12];
        let tx_errors = buf[13];

        Self {
            utilization,
            bus_off,
            tx_full,
            rx_errors,
            tx_errors,
        }
    }
}
impl IncomingTagHandler<'_> for CanMetrics {
    fn handle(&self, ds: &'_ crate::Ds) {
        ds.can_bus_util.store(self.utilization);
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Status: u8 {
        const ESTOP = 0b1000_0000;
        const BROWNOUT = 0b0001_0000;
        const CODE_START = 0b0000_1000;
        const ENABLED = 0b0000_0100;

        // Mode flags
        const TELEOP = 0b00;
        const TEST = 0b01;
        const AUTO = 0b10;
    }
}
impl Status {
    #[inline(always)]
    pub const fn is_enabled(&self) -> bool {
        self.contains(Status::ENABLED)
    }

    #[inline(always)]
    pub const fn is_browned_out(self) -> bool {
        self.contains(Status::BROWNOUT)
    }

    #[inline(always)]
    pub const fn is_estopped(self) -> bool {
        self.contains(Status::ESTOP)
    }

    //#[inline(always)]
    //pub const fn is_in_(&self) -> bool {
    //    self.contains(Self::)
    //}
    #[inline(always)]
    pub const fn is_in_teleop(&self) -> bool {
        self.contains(Self::TELEOP)
    }
    #[inline(always)]
    pub const fn is_in_auto(&self) -> bool {
        self.contains(Self::AUTO)
    }
    #[inline(always)]
    pub const fn is_in_test(&self) -> bool {
        self.contains(Self::TEST)
    }
}

bitflags! {
    pub struct Trace: u8 {
        const ROBOT_CODE = 0b0010_0000;
        const IS_ROBORIO = 0b0001_0000;
        const TEST_MODE  = 0b0000_1000;
        const AUTONOMOUS = 0b0000_0100;
        const TELEOP     = 0b0000_0010;
        const DISABLED   = 0b0000_0001;
    }
}
impl Trace {
    #[inline(always)]
    pub const fn has_robot_code(&self) -> bool {
        self.contains(Self::ROBOT_CODE)
    }
}
