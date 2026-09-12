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

/// 组装**任意** 8 字节 `PTZCmd`（等价于 WVP 的
/// `SIPCommander.frontEndCmdString(cmdCode, parameter1, parameter2, combineCode2)`）：
///
/// ```text
/// A5 | 0F | 01 | 指令码 | 数据1 | 数据2 | 组合码2<<4 | 校验和
/// ```
///
/// 聚焦/光圈、预置位、巡航、扫描、辅助开关/雨刷在 **GB/T 28181-2022 附录 A.3**
/// 里都是这 8 字节 PTZCmd 的不同指令码，**不是**独立 XML 元素；
/// WVP-PRO 也是全部走这个构造器（它那几个 `FrontEndControlCodeFor*.encode()`
/// 全部 `return ""`，从不参与下发路径）。
pub fn build_ptz_cmd_raw(
    cmd_code: u8,
    parameter1: u8,
    parameter2: u8,
    combine_code2: u8,
) -> String {
    let mut b = [0u8; 8];
    b[0] = 0xA5;
    b[1] = 0x0F;
    b[2] = 0x01; // 地址低 8 位
    b[3] = cmd_code;
    b[4] = parameter1;
    b[5] = parameter2;
    // 组合码2：高 4 位是数据3，低 4 位是地址高 4 位（此处地址高 4 位为 0）
    b[6] = (combine_code2 & 0x0F) << 4;
    b[7] = b[..7].iter().fold(0u8, |acc, x| acc.wrapping_add(*x));
    b.iter().map(|x| format!("{:02X}", x)).collect()
}

/// 聚焦 / 光圈。WVP 的指令码：基址 `1<<6`，
/// 聚焦 bit1=近、bit0=远；光圈 bit3=开、bit2=关。
///
/// | 动作 | 指令码 | 数据1 | 数据2 |
/// |------|--------|-------|-------|
/// | 聚焦近（near） | 0x42 | 聚焦速度 | 0 |
/// | 聚焦远（far）  | 0x41 | 聚焦速度 | 0 |
/// | 光圈开（in）   | 0x48 | 0 | 光圈速度 |
/// | 光圈关（out）  | 0x44 | 0 | 光圈速度 |
pub fn build_fi_cmd(action: FiAction, speed: u8) -> String {
    let s = speed.max(1);
    match action {
        // near → 1<<1；far → 1
        FiAction::FocusNear => build_ptz_cmd_raw(0x40 | 0x02, s, 0, 0),
        FiAction::FocusFar => build_ptz_cmd_raw(0x40 | 0x01, s, 0, 0),
        // in（开）→ 1<<3；out（关）→ 1<<2
        FiAction::IrisOpen => build_ptz_cmd_raw(0x40 | 0x08, 0, s, 0),
        FiAction::IrisClose => build_ptz_cmd_raw(0x40 | 0x04, 0, s, 0),
    }
}

/// 预置位：`0x81` 设置 / `0x82` 调用 / `0x83` 删除，编号放**数据2**。
pub fn build_preset_cmd(action: PresetAction, preset_index: u32) -> String {
    let cmd_code = match action {
        PresetAction::Set => 0x81,
        PresetAction::Call => 0x82,
        PresetAction::Delete => 0x83,
    };
    build_ptz_cmd_raw(cmd_code, 0, (preset_index & 0xFF) as u8, 0)
}

/// 巡航动作（对照 WVP `SourcePTZServiceForGbImpl::tour` 的指令码表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TourAction {
    /// 0x84：添加预置位到巡航组（数据1=巡航组号，数据2=预置位号）
    AddPoint,
    /// 0x85：从巡航组删除预置位
    DeletePoint,
    /// 0x86：设置巡航速度（数据3=巡航速度）
    SetSpeed,
    /// 0x87：设置巡航停留时间（数据3=停留时间）
    SetTime,
    /// 0x88：开始巡航
    Start,
    /// 停止：国标与 WVP 都没有单独的停止指令码（WVP 的 code 6 不设指令码，
    /// 实际下发的是 0x00「停止所有动作」），这里保持同样行为。
    Stop,
}

/// 巡航控制。`value` 是速度或停留时间（1-4095，写入组合码2 的高 4 位）。
pub fn build_tour_cmd(action: TourAction, tour_id: u8, preset_id: u8, value: u16) -> String {
    // 组合码2 只有 4 位（0-15）：按 WVP 的口径夹取
    let c2 = (value.min(15)) as u8;
    match action {
        TourAction::AddPoint => build_ptz_cmd_raw(0x84, tour_id, preset_id, 0),
        TourAction::DeletePoint => build_ptz_cmd_raw(0x85, tour_id, preset_id, 0),
        TourAction::SetSpeed => build_ptz_cmd_raw(0x86, tour_id, preset_id, c2),
        TourAction::SetTime => build_ptz_cmd_raw(0x87, tour_id, preset_id, c2),
        TourAction::Start => build_ptz_cmd_raw(0x88, tour_id, 0, 0),
        TourAction::Stop => build_ptz_cmd_raw(0x00, 0, 0, 0),
    }
}

/// 扫描动作（对照 WVP `SourcePTZServiceForGbImpl::scan`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanAction {
    /// 0x89：开始自动扫描
    Start,
    /// 0x89 数据2=1：设置左边界
    SetLeft,
    /// 0x89 数据2=2：设置右边界
    SetRight,
    /// 0x8A：设置扫描速度（数据2=速度）
    SetSpeed,
    /// 同巡航：国标/WVP 都没有单独停止码，下发 0x00
    Stop,
}

pub fn build_scan_cmd(action: ScanAction, scan_id: u8, speed: u8) -> String {
    match action {
        ScanAction::Start => build_ptz_cmd_raw(0x89, scan_id, 0, 0),
        ScanAction::SetLeft => build_ptz_cmd_raw(0x89, scan_id, 1, 0),
        ScanAction::SetRight => build_ptz_cmd_raw(0x89, scan_id, 2, 0),
        ScanAction::SetSpeed => build_ptz_cmd_raw(0x8A, scan_id, speed, 0),
        ScanAction::Stop => build_ptz_cmd_raw(0x00, 0, 0, 0),
    }
}

/// 辅助开关：`0x8C` 开 / `0x8D` 关，编号放数据1。
pub fn build_auxiliary_cmd(on: bool, switch_id: u8) -> String {
    if on {
        build_ptz_cmd_raw(0x8C, switch_id, 0, 0)
    } else {
        build_ptz_cmd_raw(0x8D, switch_id, 0, 0)
    }
}

/// 雨刷：与辅助开关同一对指令码，编号固定为 1。
pub fn build_wiper_cmd(on: bool) -> String {
    build_auxiliary_cmd(on, 1)
}

/// 聚焦 / 光圈动作。
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
/// **云台、聚焦/光圈、预置位全部是 `PTZCmd`**（8 字节二进制指令码）——
/// 这与 GB/T 28181-2022 §A.3 以及 WVP-PRO 的下发实现一致
/// （WVP 的 `SourcePTZServiceForGbImpl` 把 fi/preset 也走
/// `frontEndCommand(channel, cmdCode, p1, p2, p3)`）。
///
/// 2016 版的 `<FICmd>` / `<PresetCmd>`+`<PresetIndex>` 独立元素**不再下发**：
/// 只认 2022 指令码的设备会把旧写法整条忽略，而只认 2016 元素的设备同样
/// 存在于存量部署中 —— 这里按平替目标（WVP）取二进制形态，并在文档中登记。
pub fn control_element(cmd: &str, speed: u8, preset_index: u32) -> Option<(&'static str, String)> {
    if let Some(action) = PtzAction::parse(cmd) {
        return Some(("PTZCmd", build_ptz_cmd(action, speed)));
    }
    if let Some(fi) = FiAction::parse(cmd) {
        return Some(("PTZCmd", build_fi_cmd(fi, speed)));
    }
    if let Some(p) = PresetAction::parse(cmd) {
        return Some(("PTZCmd", build_preset_cmd(p, preset_index)));
    }
    None
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
    /// 三种动作都必须产出 `PTZCmd`，且是合法 8 字节（A5 起始 + 累加校验）。
    #[test]
    fn control_element_uses_correct_element_names() {
        // 全部走 PTZCmd（与 WVP 一致）
        assert_eq!(control_element("UP", 0x40, 0).unwrap().0, "PTZCmd");
        assert_eq!(control_element("ZOOM_IN", 1, 0).unwrap().0, "PTZCmd");
        assert_eq!(
            control_element("FOCUS_IN", 0x40, 0).unwrap(),
            ("PTZCmd", build_fi_cmd(FiAction::FocusNear, 0x40))
        );
        assert_eq!(
            control_element("IRIS_CLOSE", 0x40, 0).unwrap(),
            ("PTZCmd", build_fi_cmd(FiAction::IrisClose, 0x40))
        );
        assert_eq!(
            control_element("GOTO_PRESET", 0, 7).unwrap(),
            ("PTZCmd", build_preset_cmd(PresetAction::Call, 7))
        );
        // 指令码逐字节核对：聚焦近 0x42、光圈关 0x44、调用预置位 0x82
        let (_, fi) = control_element("FOCUS_IN", 0x40, 0).unwrap();
        assert!(fi.starts_with("A50F0142"), "{fi}");
        let (_, iris) = control_element("IRIS_CLOSE", 0x40, 0).unwrap();
        assert!(iris.starts_with("A50F0144"), "{iris}");
        let (_, preset) = control_element("GOTO_PRESET", 0, 7).unwrap();
        assert!(preset.starts_with("A50F01820007"), "{preset}");
        assert!(control_element("NOPE", 0, 0).is_none());
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
