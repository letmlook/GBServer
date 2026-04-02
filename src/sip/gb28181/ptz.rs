use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzCommand {
    pub device_id: String,
    pub channel_id: String,
    pub command: PtzCommandType,
    pub speed: u8,
    pub extra: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
