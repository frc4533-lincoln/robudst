pub enum TcpOutgoingTag<'t> {
    JoystickDescriptor {
        index: u8,
        is_xbox: bool,
        kind: JoystickKind,
        name: &'t str,
        axes: &'t [AxisKind],
        button_count: u8,
        pov_count: u8,
    },
    MatchInfo {
        competition: &'t str,
        match_kind: u8,
    },
    GameData {
        game_data: &'t str,
    },
}
impl TcpOutgoingTag<'_> {
    pub fn write(self) -> Vec<u8> {
        match self {
            Self::JoystickDescriptor {
                index,
                is_xbox,
                kind,
                name,
                axes,
                button_count,
                pov_count,
            } => {
                let mut buf = Vec::new();
                buf.clear();

                // 1 byte for tag id
                // 1 byte each for index, is_xbox, kind, and name.len (4 bytes)
                // 1 byte each for axis_count, button_count, and pov_count (3 bytes)
                buf.push(8u8 + name.len() as u8 + axes.len() as u8);
                buf.push(0x02);

                buf.extend([index, is_xbox as u8, kind as u8, name.len() as u8]);

                buf.extend_from_slice(name.as_bytes());
                buf.push(axes.len() as u8);
                buf.extend(axes.into_iter().map(|axis| *axis as u8));
                buf.extend([button_count, pov_count]);

                buf
            }

            Self::MatchInfo {
                competition,
                match_kind,
            } => Vec::new(),

            Self::GameData { game_data } => Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
#[repr(i8)]
pub enum JoystickKind {
    Unknown = -1,
    XInputUnknown = 0,
    XInputGamepad = 1,
    XInputWheel = 2,
    XInputArcade = 3,
    XInputFlightStick = 4,
    XInputDancePad = 5,
    XInputGuitar = 6,
    XInputGuitar2 = 7,
    XInputDrumKit = 8,
    XInputGuitar3 = 11,
    XInputArcadePad = 19,
    HIDJoystick = 20,
    HIDGamepad = 21,
    HIDDriving = 22,
    HIDFlight = 23,
    HIDFirstPerson = 24,
}

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum AxisKind {
    X = 0,
    Y = 1,
    Z = 2,
    Twist = 3,
    Throttle = 4,
}
