use super::invite_session::StreamType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdpDirection {
    SendOnly,
    RecvOnly,
    SendRecv,
    Inactive,
}

impl SdpDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            SdpDirection::SendOnly => "sendonly",
            SdpDirection::RecvOnly => "recvonly",
            SdpDirection::SendRecv => "sendrecv",
            SdpDirection::Inactive => "inactive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdpSetup {
    Active,
    Passive,
    ActPass,
}

impl SdpSetup {
    pub fn as_str(&self) -> &'static str {
        match self {
            SdpSetup::Active => "active",
            SdpSetup::Passive => "passive",
            SdpSetup::ActPass => "actpass",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportMode {
    Udp,
    Tcp,
}

pub struct SdpBuilder {
    ip: String,
    media_port: u16,
    stream_type: StreamType,
    ssrc: String,
    direction: SdpDirection,
    transport: TransportMode,
    setup: Option<SdpSetup>,
    start_time: Option<String>,
    end_time: Option<String>,
    session_id: u64,
}

impl SdpBuilder {
    pub fn new(ip: &str, media_port: u16, stream_type: StreamType, ssrc: &str) -> Self {
        let direction = match stream_type {
            StreamType::Play | StreamType::Playback | StreamType::Download => SdpDirection::RecvOnly,
            StreamType::Talk | StreamType::Broadcast => SdpDirection::SendRecv,
        };
        Self {
            ip: ip.to_string(),
            media_port,
            stream_type,
            ssrc: ssrc.to_string(),
            direction,
            transport: TransportMode::Udp,
            setup: None,
            start_time: None,
            end_time: None,
            session_id: 0,
        }
    }

    pub fn direction(mut self, direction: SdpDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn transport(mut self, transport: TransportMode) -> Self {
        self.transport = transport;
        self
    }

    pub fn setup(mut self, setup: SdpSetup) -> Self {
        self.setup = Some(setup);
        self
    }

    pub fn time_range(mut self, start: &str, end: &str) -> Self {
        self.start_time = Some(start.to_string());
        self.end_time = Some(end.to_string());
        self
    }

    pub fn session_id(mut self, id: u64) -> Self {
        self.session_id = id;
        self
    }

    pub fn build(self) -> String {
        let session_name = match self.stream_type {
            StreamType::Play => "Play",
            StreamType::Playback => "Playback",
            StreamType::Download => "Download",
            StreamType::Talk => "Talk",
            StreamType::Broadcast => "Broadcast",
        };

        let proto = match self.transport {
            TransportMode::Udp => "RTP/AVP",
            TransportMode::Tcp => "TCP/RTP/AVP",
        };

        let t_field = match (&self.start_time, &self.end_time) {
            (Some(s), Some(e)) if s != "0" => format!("{} {}", s, e),
            _ => "0 0".to_string(),
        };

        let mut sdp = format!(
            "v=0\r\n\
             o=- {} 0 IN IP4 {}\r\n\
             s={}\r\n\
             c=IN IP4 {}\r\n\
             t={}\r\n",
            self.session_id, self.ip, session_name, self.ip, t_field
        );

        match self.stream_type {
            StreamType::Talk | StreamType::Broadcast => {
                sdp.push_str(&format!(
                    "m=audio {} {} 8 0 101\r\n\
                     a=rtpmap:8 PCMA/8000\r\n\
                     a=rtpmap:0 PCMU/8000\r\n\
                     a=rtpmap:101 telephone-event/8000\r\n"
                    , self.media_port, proto
                ));
            }
            _ => {
                sdp.push_str(&format!(
                    "m=video {} {} 96\r\n\
                     a=rtpmap:96 PS/90000\r\n"
                    , self.media_port, proto
                ));
            }
        }

        sdp.push_str(&format!("a={}\r\n", self.direction.as_str()));

        if self.transport == TransportMode::Tcp {
            if let Some(ref setup) = self.setup {
                sdp.push_str(&format!("a=setup:{}\r\n", setup.as_str()));
            } else {
                sdp.push_str("a=setup:passive\r\n");
            }
            sdp.push_str("a=connection:new\r\n");
        }

        if let (Some(s), Some(_)) = (&self.start_time, &self.end_time) {
            if s != "0" {
                sdp.push_str(&format!("a=range:npt={},{}\r\n", 
                    self.start_time.as_deref().unwrap_or("0"),
                    self.end_time.as_deref().unwrap_or("0")
                ));
            }
        }

        sdp.push_str(&format!("y={}\r\n", self.ssrc));

        match self.stream_type {
            StreamType::Talk | StreamType::Broadcast => {}
            _ => {
                sdp.push_str("f=v/1/96/1/2/1/1/0\r\n");
            }
        }

        sdp
    }
}

pub fn play_sdp(ip: &str, media_port: u16, ssrc: &str) -> String {
    SdpBuilder::new(ip, media_port, StreamType::Play, ssrc)
        .build()
}

pub fn playback_sdp(ip: &str, media_port: u16, ssrc: &str, start_time: &str, end_time: &str) -> String {
    SdpBuilder::new(ip, media_port, StreamType::Playback, ssrc)
        .time_range(start_time, end_time)
        .build()
}

pub fn download_sdp(ip: &str, media_port: u16, ssrc: &str, start_time: &str, end_time: &str) -> String {
    SdpBuilder::new(ip, media_port, StreamType::Download, ssrc)
        .time_range(start_time, end_time)
        .build()
}

pub fn talk_sdp(ip: &str, audio_port: u16, ssrc: &str) -> String {
    SdpBuilder::new(ip, audio_port, StreamType::Talk, ssrc)
        .build()
}

pub fn broadcast_sdp(ip: &str, audio_port: u16, ssrc: &str) -> String {
    SdpBuilder::new(ip, audio_port, StreamType::Broadcast, ssrc)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_play_sdp() {
        let sdp = play_sdp("192.168.1.100", 50000, "0100000001");
        assert!(sdp.contains("s=Play"));
        assert!(sdp.contains("c=IN IP4 192.168.1.100"));
        assert!(sdp.contains("m=video 50000 RTP/AVP 96"));
        assert!(sdp.contains("a=recvonly"));
        assert!(sdp.contains("y=0100000001"));
        assert!(sdp.contains("f=v/1/96/1/2/1/1/0"));
    }

    #[test]
    fn test_playback_sdp() {
        let sdp = playback_sdp("192.168.1.100", 50000, "0100000001", "1700000000", "1700003600");
        assert!(sdp.contains("s=Playback"));
        assert!(sdp.contains("t=1700000000 1700003600"));
        assert!(sdp.contains("a=range:npt=1700000000,1700003600"));
    }

    #[test]
    fn test_download_sdp() {
        let sdp = download_sdp("192.168.1.100", 50000, "0100000001", "1700000000", "1700003600");
        assert!(sdp.contains("s=Download"));
        assert!(sdp.contains("a=range:npt=1700000000,1700003600"));
    }

    #[test]
    fn test_talk_sdp() {
        let sdp = talk_sdp("192.168.1.100", 50002, "0200005678");
        assert!(sdp.contains("s=Talk"));
        assert!(sdp.contains("m=audio 50002 RTP/AVP 8 0 101"));
        assert!(sdp.contains("a=sendrecv"));
        assert!(sdp.contains("y=0200005678"));
        assert!(!sdp.contains("f=v/1"));
    }

    #[test]
    fn test_tcp_transport() {
        let sdp = SdpBuilder::new("192.168.1.100", 50000, StreamType::Play, "0100000001")
            .transport(TransportMode::Tcp)
            .setup(SdpSetup::Passive)
            .build();
        assert!(sdp.contains("TCP/RTP/AVP"));
        assert!(sdp.contains("a=setup:passive"));
        assert!(sdp.contains("a=connection:new"));
    }

    #[test]
    fn test_sendonly_direction() {
        let sdp = SdpBuilder::new("192.168.1.100", 50000, StreamType::Play, "0100000001")
            .direction(SdpDirection::SendOnly)
            .build();
        assert!(sdp.contains("a=sendonly"));
    }
}
