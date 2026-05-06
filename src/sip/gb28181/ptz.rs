use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzCommand {
    pub device_id: String,
    pub channel_id: String,
    pub command: PtzCommandType,
    pub speed: u8,
    pub extra: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum PtzCommandType {
    Left,
    Right,
    Up,
    Down,
    ZoomIn,
    ZoomOut,
    LeftUp,
    LeftDown,
    RightUp,
    RightDown,
    Stop,
    IrisIn,
    IrisOut,
    FocusNear,
    FocusFar,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetCommand {
    pub device_id: String,
    pub channel_id: String,
    pub command: PresetCommandType,
    pub preset_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PresetCommandType {
    Goto,
    Set,
    Clear,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardCommand {
    pub device_id: String,
    pub guard_cmd: GuardCmd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum GuardCmd {
    SetGuard,
    ResetGuard,
}

impl PtzCommand {
    pub fn to_xml(&self) -> String {
        let cmd_str = match self.command {
            PtzCommandType::Left => "LEFT",
            PtzCommandType::Right => "RIGHT",
            PtzCommandType::Up => "UP",
            PtzCommandType::Down => "DOWN",
            PtzCommandType::ZoomIn => "ZOOM_IN",
            PtzCommandType::ZoomOut => "ZOOM_OUT",
            PtzCommandType::LeftUp => "LEFT_UP",
            PtzCommandType::LeftDown => "LEFT_DOWN",
            PtzCommandType::RightUp => "RIGHT_UP",
            PtzCommandType::RightDown => "RIGHT_DOWN",
            PtzCommandType::Stop => "STOP",
            PtzCommandType::IrisIn => "IRIS_IN",
            PtzCommandType::IrisOut => "IRIS_OUT",
            PtzCommandType::FocusNear => "FOCUS_NEAR",
            PtzCommandType::FocusFar => "FOCUS_FAR",
        };

        if let Some(extra) = &self.extra {
            format!(
                r#"<Control><CmdType>DeviceControl</CmdType><SN>1</SN><DeviceID>{}</DeviceID><PTZCmd>{} {} {}</PTZCmd></Control>"#,
                self.channel_id, cmd_str, self.speed, extra
            )
        } else {
            format!(
                r#"<Control><CmdType>DeviceControl</CmdType><SN>1</SN><DeviceID>{}</DeviceID><PTZCmd>{} {}</PTZCmd></Control>"#,
                self.channel_id, cmd_str, self.speed
            )
        }
    }
}

impl PresetCommand {
    pub fn to_xml(&self) -> String {
        match self.command {
            PresetCommandType::Goto => {
                format!(
                    r#"<Control><CmdType>DeviceControl</CmdType><SN>1</SN><DeviceID>{}</DeviceID><PTZCmd>GOTO_PRESET {}</PTZCmd></Control>"#,
                    self.channel_id, self.preset_index
                )
            }
            PresetCommandType::Set => {
                format!(
                    r#"<Control><CmdType>DeviceControl</CmdType><SN>1</SN><DeviceID>{}</DeviceID><PTZCmd>SET_PRESET {}</PTZCmd></Control>"#,
                    self.channel_id, self.preset_index
                )
            }
            PresetCommandType::Clear => {
                format!(
                    r#"<Control><CmdType>DeviceControl</CmdType><SN>1</SN><DeviceID>{}</DeviceID><PTZCmd>CLE_PRESET {}</PTZCmd></Control>"#,
                    self.channel_id, self.preset_index
                )
            }
        }
    }
}

impl GuardCommand {
    pub fn to_xml(&self) -> String {
        let cmd = match self.guard_cmd {
            GuardCmd::SetGuard => "SetGuard",
            GuardCmd::ResetGuard => "ResetGuard",
        };
        format!(
            r#"<Control><CmdType>DeviceControl</CmdType><SN>1</SN><DeviceID>{}</DeviceID><GuardCmd>{}</GuardCmd></Control>"#,
            self.device_id, cmd
        )
    }
}

pub fn parse_ptz_command(xml: &str) -> Option<(String, String, u8)> {
    let parsed = crate::sip::gb28181::XmlParser::parse(xml);
    let device_id = parsed.get("DeviceID")?.clone();
    let ptz_cmd = parsed.get("PTZCmd")?.clone();

    let parts: Vec<&str> = ptz_cmd.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let speed: u8 = parts[1].parse().ok()?;

    Some((parts[0].to_string(), device_id, speed))
}

pub struct PtzEncode;

impl PtzEncode {
    pub fn direction_8(direction: &PtzCommandType, speed: u8) -> String {
        let s = (speed as u16).min(255) as u8;

        match direction {
            PtzCommandType::Stop => Self::to_hex_command(0, 0, 0, 0, 0),
            PtzCommandType::Left => Self::to_hex_command(0x81, s, 0, 0, 0),
            PtzCommandType::Right => Self::to_hex_command(0x01, s, 0, 0, 0),
            PtzCommandType::Up => Self::to_hex_command(0, 0, 0x01, s, 0),
            PtzCommandType::Down => Self::to_hex_command(0, 0, 0x81, s, 0),
            PtzCommandType::LeftUp => Self::to_hex_command(0x81, s, 0x01, s, 0),
            PtzCommandType::LeftDown => Self::to_hex_command(0x81, s, 0x81, s, 0),
            PtzCommandType::RightUp => Self::to_hex_command(0x01, s, 0x01, s, 0),
            PtzCommandType::RightDown => Self::to_hex_command(0x01, s, 0x81, s, 0),
            PtzCommandType::ZoomIn => Self::to_hex_command(0, 0, 0, 0, 0x21),
            PtzCommandType::ZoomOut => Self::to_hex_command(0, 0, 0, 0, 0x41),
            _ => Self::to_hex_command(0, 0, 0, 0, 0),
        }
    }

    pub fn stop() -> String {
        Self::to_hex_command(0, 0, 0, 0, 0)
    }

    pub fn to_hex_command(pan_action: u8, pan_speed: u8, tilt_action: u8, tilt_speed: u8, _zoom_data: u8) -> String {
        format!("A5{:02X}{:02X}{:02X}{:02X}AF", pan_action, pan_speed, tilt_action, tilt_speed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum FrontEndCommand {
    CruiseStart,
    CruiseStop,
    SetPoint,
    DeletePoint,
    SetSpeed,
    SetTime,
    ScanStart,
    ScanStop,
    SetLeft,
    SetRight,
    WiperStart,
    WiperStop,
    AuxSwitchOn,
    AuxSwitchOff,
}

impl FrontEndCommand {
    pub fn to_xml(&self, device_id: &str, channel_id: &str, param: Option<u32>, sn: u32) -> String {
        let cmd = match self {
            FrontEndCommand::CruiseStart => "CruiseStart",
            FrontEndCommand::CruiseStop => "CruiseStop",
            FrontEndCommand::SetPoint => "SetPoint",
            FrontEndCommand::DeletePoint => "DeletePoint",
            FrontEndCommand::SetSpeed => "SetSpeed",
            FrontEndCommand::SetTime => "SetTime",
            FrontEndCommand::ScanStart => "ScanStart",
            FrontEndCommand::ScanStop => "ScanStop",
            FrontEndCommand::SetLeft => "SetLeft",
            FrontEndCommand::SetRight => "SetRight",
            FrontEndCommand::WiperStart => "WiperStart",
            FrontEndCommand::WiperStop => "WiperStop",
            FrontEndCommand::AuxSwitchOn => "AuxSwitchOn",
            FrontEndCommand::AuxSwitchOff => "AuxSwitchOff",
        };

        let param_xml = param.map(|p| format!("<Param>{}</Param>", p)).unwrap_or_default();

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Control>
<CmdType>DeviceControl</CmdType>
<SN>{}</SN>
<DeviceID>{}</DeviceID>
<FrontEndCmd>
<ChannelID>{}</ChannelID>
<Cmd>{}</Cmd>
{}
</FrontEndCmd>
</Control>"#,
            sn, device_id, channel_id, cmd, param_xml
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ptz_stop() {
        let cmd = PtzEncode::stop();
        assert!(cmd.starts_with("A5"));
        assert!(cmd.ends_with("AF"));
    }

    #[test]
    fn test_front_end_cruise_xml() {
        let xml = FrontEndCommand::CruiseStart.to_xml("dev1", "ch1", Some(1), 10);
        assert!(xml.contains("CruiseStart"));
        assert!(xml.contains("DeviceControl"));
    }
}
