//! GB28181 **前端设备控制**（附录 A.2）报文构造。
//!
//! 这是云台/镜头/预置位控制的**唯一实现**，此前散落在三个 handler 里且格式不对：
//!
//! * `handlers/device_control.rs::build_ptz_xml` 生成 `05 01 00 00 00 ss FF`（6 字节），
//! * `handlers/common_channel.rs` 同样格式，聚焦/光圈也塞进 `<PTZCmd>`，
//! * `handlers/device_batch.rs` 直接硬编码 `"A500000000AF"`（5 字节）。
//!
//! 国标要求的 `PTZCmd` 是**固定 8 字节**、以 `0xA5` 开头、末字节为累加校验：
//!
//! ```text
//! 字节1  0xA5                              起始码
//! 字节2  组合码1（高 4 位版本=0，低 4 位校验位=0xF）
//! 字节3  地址低 8 位
//! 字节4  指令码
//! 字节5  数据1（水平速度）
//! 字节6  数据2（垂直速度）
//! 字节7  组合码2（高 4 位=数据3，低 4 位=地址高 4 位）
//! 字节8  校验码 = 前 7 字节之和 % 256
//! ```
//!
//! 参考实现给出的标准样例（本模块的单测逐字节对齐）：
//!
//! ```text
//! 向上 A50F0108 001F00 DC     向下 A50F0104 001F00 D8
//! 向左 A50F0102 1F0000 D6     向右 A50F0101 1F0000 D5
//! 放大 A50F0110 000010 D5     缩小 A50F0120 000010 E5
//! ```
//!
//! 另外，聚焦/光圈与预置位在国标里各有**独立元素**（`FICmd` / `PresetCmd`），
//! 不能借用 `PTZCmd`。

/// 云台/镜头动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtzAction {
    Up,
    Down,
    Left,
    Right,
    ZoomIn,
    ZoomOut,
    Stop,
}

impl PtzAction {
    /// 从上/下/左/右/放大/缩小/停止等常见写法解析。
    pub fn parse(cmd: &str) -> Option<Self> {
        match cmd.trim().to_ascii_uppercase().replace('-', "_").as_str() {
            "UP" | "TILT_UP" | "PTZ_UP" => Some(Self::Up),
            "DOWN" | "TILT_DOWN" | "PTZ_DOWN" => Some(Self::Down),
            "LEFT" | "PAN_LEFT" | "PTZ_LEFT" => Some(Self::Left),
            "RIGHT" | "PAN_RIGHT" | "PTZ_RIGHT" => Some(Self::Right),
            "ZOOM_IN" | "ZOOMIN" | "ZOOM_+" => Some(Self::ZoomIn),
            "ZOOM_OUT" | "ZOOMOUT" | "ZOOM_-" => Some(Self::ZoomOut),
            "STOP" | "PTZ_STOP" | "STOP_MOVE" => Some(Self::Stop),
            _ => None,
        }
    }

    fn code_byte(self) -> u8 {
        match self {
            // 指令码位定义：bit0 右 / bit1 左 / bit2 下 / bit3 上 /
            //               bit4 变倍+ / bit5 变倍-
            Self::Right => 0x01,
            Self::Left => 0x02,
            Self::Down => 0x04,
            Self::Up => 0x08,
            Self::ZoomIn => 0x10,
            Self::ZoomOut => 0x20,
            Self::Stop => 0x00,
        }
    }

    fn is_pan_tilt(self) -> bool {
        matches!(self, Self::Up | Self::Down | Self::Left | Self::Right)
    }

    fn is_lens(self) -> bool {
        matches!(self, Self::ZoomIn | Self::ZoomOut)
    }
}

/// 构造 8 字节 `PTZCmd`（大写十六进制串）。
///
/// `speed` 取值范围 1~255，对云台动作写入水平/垂直速度字节；
/// 对变倍动作写入组合码2 的高 4 位（数据3），值域 1~15。
pub fn build_ptz_cmd(action: PtzAction, speed: u8) -> String {
    let mut b = [0u8; 8];
    b[0] = 0xA5;
    b[1] = 0x0F; // 组合码1：版本 0，校验位 0xF
    b[2] = 0x01; // 地址低 8 位
    b[3] = action.code_byte();
    if action == PtzAction::Stop {
        // 停止：速度为 0，指令码为 0
        b[3] = 0x00;
    } else if action.is_pan_tilt() {
        let s = speed.max(1);
        // 水平速度放数据1，垂直速度放数据2（对角线同时给两个方向）
        if matches!(action, PtzAction::Left | PtzAction::Right) {
            b[4] = s;
        } else {
            b[5] = s;
        }
    } else if action.is_lens() {
        // 数据3 = 变倍速度（1~15）
        let nibble = (speed.clamp(1, 15)) & 0x0F;
        b[6] = nibble << 4;
    }
    b[7] = b[..7].iter().fold(0u8, |acc, x| acc.wrapping_add(*x));
    b.iter().map(|x| format!("{:02X}", x)).collect()
}

/// 聚焦 / 光圈动作 —— 国标用独立的 `<FICmd>` 元素。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiAction {
    FocusNear,
    FocusFar,
    IrisOpen,
    IrisClose,
}

impl FiAction {
    pub fn parse(cmd: &str) -> Option<Self> {
        match cmd.trim().to_ascii_uppercase().replace('-', "_").as_str() {
            "FOCUS_IN" | "FOCUS_NEAR" => Some(Self::FocusNear),
            "FOCUS_OUT" | "FOCUS_FAR" => Some(Self::FocusFar),
            "IRIS_IN" | "IRIS_OPEN" | "IRIS_ON" => Some(Self::IrisOpen),
            "IRIS_OUT" | "IRIS_CLOSE" | "IRIS_OFF" => Some(Self::IrisClose),
            _ => None,
        }
    }

    pub fn as_cmd_value(self) -> &'static str {
        match self {
            Self::FocusNear => "FocusNear",
            Self::FocusFar => "FocusFar",
            Self::IrisOpen => "IrisOpen",
            Self::IrisClose => "IrisClose",
        }
    }
}

/// 预置位动作 —— 国标用独立的 `<PresetCmd>` + `<PresetIndex>`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetAction {
    Set,
    Call,
    Delete,
}

impl PresetAction {
    pub fn parse(cmd: &str) -> Option<Self> {
        match cmd.trim().to_ascii_uppercase().replace('-', "_").as_str() {
            "SET_PRESET" | "SETPRESET" | "ADD_PRESET" => Some(Self::Set),
            "GOTO_PRESET" | "CALL_PRESET" | "CALlPRESET" | "GOTOPRESET" => Some(Self::Call),
            "CLEAR_PRESET" | "DEL_PRESET" | "DELETEPRESET" => Some(Self::Delete),
            _ => None,
        }
    }

    pub fn as_cmd_value(self) -> &'static str {
        match self {
            Self::Set => "SetPreset",
            Self::Call => "CallPreset",
            Self::Delete => "DelPreset",
        }
    }
}

/// 设备控制（MANSCDP）里 `<Control>` 的子元素：`(元素名, 元素内容)`。
///
/// 元素名必须按国标选择，不能一律用 `PTZCmd`：
/// 云台/变倍是 `PTZCmd`，聚焦/光圈是 `FICmd`，预置位是 `PresetCmd`
/// （另带 `PresetIndex`）。
pub fn control_element(cmd: &str, speed: u8, preset_index: u32) -> Option<(&'static str, String)> {
    if let Some(action) = PtzAction::parse(cmd) {
        return Some(("PTZCmd", build_ptz_cmd(action, speed)));
    }
    if let Some(fi) = FiAction::parse(cmd) {
        return Some(("FICmd", fi.as_cmd_value().to_string()));
    }
    if let Some(p) = PresetAction::parse(cmd) {
        return Some(("PresetCmd", format!("{}|{}", p.as_cmd_value(), preset_index)));
    }
    None
}

/// 预置位需要额外的 `<PresetIndex>` 元素，单独暴露给 XML 组装方。
pub fn preset_index_element(cmd: &str, preset_index: u32) -> Option<String> {
    PresetAction::parse(cmd).map(|_| format!("<PresetIndex>{}</PresetIndex>", preset_index))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 与参考资料给出的标准样例**逐字节**对齐。
    #[test]
    fn ptz_cmd_matches_reference_samples() {
        assert_eq!(build_ptz_cmd(PtzAction::Right, 0x1F), "A50F01011F0000D5");
        assert_eq!(build_ptz_cmd(PtzAction::Left, 0x1F), "A50F01021F0000D6");
        assert_eq!(build_ptz_cmd(PtzAction::Down, 0x1F), "A50F0104001F00D8");
        assert_eq!(build_ptz_cmd(PtzAction::Up, 0x1F), "A50F0108001F00DC");
        // 变倍：速度写在组合码2 高 4 位（参考样例为 0x10 = 变倍速度 1）
        assert_eq!(build_ptz_cmd(PtzAction::ZoomIn, 1), "A50F0110000010D5");
        assert_eq!(build_ptz_cmd(PtzAction::ZoomOut, 1), "A50F0120000010E5");
    }

    /// 固定 8 字节、A5 开头、末字节为前 7 字节累加和。
    #[test]
    fn ptz_cmd_shape_and_checksum() {
        for (action, speed) in [
            (PtzAction::Up, 0x40),
            (PtzAction::Down, 0xFF),
            (PtzAction::Left, 0x01),
            (PtzAction::Right, 0x7F),
            (PtzAction::ZoomIn, 0x0A),
            (PtzAction::ZoomOut, 0x0F),
            (PtzAction::Stop, 0),
        ] {
            let cmd = build_ptz_cmd(action, speed);
            assert_eq!(cmd.len(), 16, "PTZCmd 必须是 8 字节(16 hex): {}", cmd);
            assert!(cmd.starts_with("A5"), "PTZCmd 必须以 A5 开头: {}", cmd);
            let bytes: Vec<u8> = (0..8)
                .map(|i| u8::from_str_radix(&cmd[i * 2..i * 2 + 2], 16).unwrap())
                .collect();
            let sum = bytes[..7].iter().fold(0u8, |a, b| a.wrapping_add(*b));
            assert_eq!(bytes[7], sum, "校验码错误: {}", cmd);
            assert!(cmd.chars().all(|c| !c.is_ascii_lowercase()), "必须大写: {}", cmd);
        }
    }

    /// 停止指令必须把所有速度清零，否则云台会一直转。
    #[test]
    fn stop_zeroes_speeds() {
        assert_eq!(build_ptz_cmd(PtzAction::Stop, 0xFF), "A50F0100000000B5");
    }

    /// 元素名不能一律用 PTZCmd：聚焦/光圈是 FICmd，预置位是 PresetCmd。
    #[test]
    fn control_element_uses_correct_element_names() {
        assert_eq!(control_element("UP", 0x40, 0).unwrap().0, "PTZCmd");
        assert_eq!(control_element("ZOOM_IN", 1, 0).unwrap().0, "PTZCmd");
        assert_eq!(
            control_element("FOCUS_IN", 0x40, 0).unwrap(),
            ("FICmd", "FocusNear".to_string())
        );
        assert_eq!(
            control_element("IRIS_CLOSE", 0x40, 0).unwrap(),
            ("FICmd", "IrisClose".to_string())
        );
        let (name, value) = control_element("GOTO_PRESET", 0, 7).unwrap();
        assert_eq!(name, "PresetCmd");
        assert_eq!(value, "CallPreset|7");
        assert_eq!(
            preset_index_element("GOTO_PRESET", 7).as_deref(),
            Some("<PresetIndex>7</PresetIndex>")
        );
        assert!(control_element("NOPE", 0, 0).is_none());
        assert!(preset_index_element("UP", 1).is_none());
    }

    #[test]
    fn parses_action_aliases() {
        for (input, expect) in [
            ("up", PtzAction::Up),
            ("TILT_UP", PtzAction::Up),
            ("pan-left", PtzAction::Left),
            ("zoomin", PtzAction::ZoomIn),
            ("ptz_stop", PtzAction::Stop),
        ] {
            assert_eq!(PtzAction::parse(input), Some(expect), "{}", input);
        }
        assert_eq!(PtzAction::parse("unknown"), None);
    }
}
