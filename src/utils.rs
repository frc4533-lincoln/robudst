use std::net::Ipv4Addr;

use crate::{RobotCodeMode, RobotStatus};

/// Generate the team IP
///
/// Let's say you're on team number 12345 (just like all of my passwords).
/// Here's how you'd do that:
///
/// ```text
/// 1   2   3   4   5
/// |___|___|   |___|
///        \     /
///     10.123.45.2
/// ```
///
/// Reference:
/// <https://docs.wpilib.org/en/stable/docs/networking/networking-introduction/ip-configurations.html#te-am-ip-notation>
pub fn gen_team_ip(team_number: u16) -> Option<Ipv4Addr> {
    if team_number > 25_599 {
        None
    } else {
        Some(Ipv4Addr::new(
            10,
            (team_number / 100) as u8,
            (team_number % 100) as u8,
            2,
        ))
    }
}

#[inline(always)]
pub const fn find_status(
    status: crate::proto::incoming::udp::Status,
    trace: crate::proto::incoming::udp::Trace,
) -> (RobotStatus, RobotCodeMode) {
    assert!(status.is_in_teleop() ^ status.is_in_auto() ^ status.is_in_test());

    let mode = if status.is_in_teleop() {
        RobotCodeMode::Teleop
    } else if status.is_in_auto() {
        RobotCodeMode::Autonomous
    } else if status.is_in_test() {
        RobotCodeMode::Test
    } else {
        panic!();
    };

    if !trace.has_robot_code() {
        return (RobotStatus::NoRobotCode, mode);
    }

    if status.is_estopped() {
        return (RobotStatus::EStopped, mode);
    }

    if status.is_browned_out() {
        return (RobotStatus::BrownedOut, mode);
    }

    if status.is_enabled() {
        (RobotStatus::Enabled, mode)
    } else {
        (RobotStatus::Disabled, mode)
    }
}
