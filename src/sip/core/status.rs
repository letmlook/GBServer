use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseClass {
    Provisional,
    Success,
    Redirection,
    ClientError,
    ServerError,
    GlobalError,
}

impl ResponseClass {
    pub fn from_code(code: u16) -> Self {
        match code {
            100..=199 => Self::Provisional,
            200..=299 => Self::Success,
            300..=399 => Self::Redirection,
            400..=499 => Self::ClientError,
            500..=599 => Self::ServerError,
            600..=699 => Self::GlobalError,
            _ => Self::ClientError,
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    pub fn is_error(&self) -> bool {
        matches!(
            self,
            Self::ClientError | Self::ServerError | Self::GlobalError
        )
    }

    pub fn is_final(&self) -> bool {
        !matches!(self, Self::Provisional)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum StatusCode {
    Trying = 100,
    Ringing = 180,
    CallIsBeingForwarded = 181,
    Queued = 182,
    SessionProgress = 183,
    EarlySession = 199,
    Ok = 200,
    Accepted = 202,
    MultipleChoices = 300,
    MovedPermanently = 301,
    MovedTemporarily = 302,
    UseProxy = 305,
    AlternativeService = 380,
    BadRequest = 400,
    Unauthorized = 401,
    PaymentRequired = 402,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    NotAcceptable = 406,
    ProxyAuthenticationRequired = 407,
    RequestTimeout = 408,
    Gone = 410,
    ConditionRequired = 417,
    LoopDetected = 482,
    TooManyHops = 483,
    AddressIncomplete = 484,
    Ambiguous = 485,
    BusyHere = 486,
    RequestTerminated = 487,
    NotAcceptableHere = 488,
    BadEvent = 489,
    RequestPending = 491,
    Undecipherable = 493,
    ServerInternalError = 500,
    NotImplemented = 501,
    BadGateway = 502,
    ServiceUnavailable = 503,
    ServerTimeout = 504,
    VersionNotSupported = 505,
    MessageTooLarge = 513,
    BusyEverywhere = 600,
    Decline = 603,
    DoesNotExistAnywhere = 604,
    SessionNotAcceptable = 606,
}

impl StatusCode {
    pub fn from_code(code: u16) -> Self {
        match code {
            100 => Self::Trying,
            180 => Self::Ringing,
            181 => Self::CallIsBeingForwarded,
            182 => Self::Queued,
            183 => Self::SessionProgress,
            199 => Self::EarlySession,
            200 => Self::Ok,
            202 => Self::Accepted,
            300 => Self::MultipleChoices,
            301 => Self::MovedPermanently,
            302 => Self::MovedTemporarily,
            305 => Self::UseProxy,
            380 => Self::AlternativeService,
            400 => Self::BadRequest,
            401 => Self::Unauthorized,
            402 => Self::PaymentRequired,
            403 => Self::Forbidden,
            404 => Self::NotFound,
            405 => Self::MethodNotAllowed,
            406 => Self::NotAcceptable,
            407 => Self::ProxyAuthenticationRequired,
            408 => Self::RequestTimeout,
            410 => Self::Gone,
            417 => Self::ConditionRequired,
            482 => Self::LoopDetected,
            483 => Self::TooManyHops,
            484 => Self::AddressIncomplete,
            485 => Self::Ambiguous,
            486 => Self::BusyHere,
            487 => Self::RequestTerminated,
            488 => Self::NotAcceptableHere,
            489 => Self::BadEvent,
            491 => Self::RequestPending,
            493 => Self::Undecipherable,
            500 => Self::ServerInternalError,
            501 => Self::NotImplemented,
            502 => Self::BadGateway,
            503 => Self::ServiceUnavailable,
            504 => Self::ServerTimeout,
            505 => Self::VersionNotSupported,
            513 => Self::MessageTooLarge,
            600 => Self::BusyEverywhere,
            603 => Self::Decline,
            604 => Self::DoesNotExistAnywhere,
            606 => Self::SessionNotAcceptable,
            _ => Self::BadRequest,
        }
    }

    pub fn code(&self) -> u16 {
        *self as u16
    }

    pub fn reason(&self) -> &'static str {
        match self {
            Self::Trying => "Trying",
            Self::Ringing => "Ringing",
            Self::CallIsBeingForwarded => "Call Is Being Forwarded",
            Self::Queued => "Queued",
            Self::SessionProgress => "Session Progress",
            Self::EarlySession => "Early Session",
            Self::Ok => "OK",
            Self::Accepted => "Accepted",
            Self::MultipleChoices => "Multiple Choices",
            Self::MovedPermanently => "Moved Permanently",
            Self::MovedTemporarily => "Moved Temporarily",
            Self::UseProxy => "Use Proxy",
            Self::AlternativeService => "Alternative Service",
            Self::BadRequest => "Bad Request",
            Self::Unauthorized => "Unauthorized",
            Self::PaymentRequired => "Payment Required",
            Self::Forbidden => "Forbidden",
            Self::NotFound => "Not Found",
            Self::MethodNotAllowed => "Method Not Allowed",
            Self::NotAcceptable => "Not Acceptable",
            Self::ProxyAuthenticationRequired => "Proxy Authentication Required",
            Self::RequestTimeout => "Request Timeout",
            Self::Gone => "Gone",
            Self::ConditionRequired => "Condition Required",
            Self::LoopDetected => "Loop Detected",
            Self::TooManyHops => "Too Many Hops",
            Self::AddressIncomplete => "Address Incomplete",
            Self::Ambiguous => "Ambiguous",
            Self::BusyHere => "Busy Here",
            Self::RequestTerminated => "Request Terminated",
            Self::NotAcceptableHere => "Not Acceptable Here",
            Self::BadEvent => "Bad Event",
            Self::RequestPending => "Request Pending",
            Self::Undecipherable => "Undecipherable",
            Self::ServerInternalError => "Server Internal Error",
            Self::NotImplemented => "Not Implemented",
            Self::BadGateway => "Bad Gateway",
            Self::ServiceUnavailable => "Service Unavailable",
            Self::ServerTimeout => "Server Timeout",
            Self::VersionNotSupported => "Version Not Supported",
            Self::MessageTooLarge => "Message Too Large",
            Self::BusyEverywhere => "Busy Everywhere",
            Self::Decline => "Decline",
            Self::DoesNotExistAnywhere => "Does Not Exist Anywhere",
            Self::SessionNotAcceptable => "Session Not Acceptable",
        }
    }

    pub fn class(&self) -> ResponseClass {
        ResponseClass::from_code(self.code())
    }

    pub fn is_provisional(&self) -> bool {
        matches!(
            self,
            Self::Trying
                | Self::Ringing
                | Self::CallIsBeingForwarded
                | Self::Queued
                | Self::SessionProgress
                | Self::EarlySession
        )
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Ok | Self::Accepted)
    }

    pub fn is_final(&self) -> bool {
        self.class().is_final()
    }

    pub fn is_error(&self) -> bool {
        self.class().is_error()
    }

    pub fn requires_reliable(&self) -> bool {
        self.is_provisional() && self.code() != 100
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.code(), self.reason())
    }
}

impl Default for StatusCode {
    fn default() -> Self {
        Self::BadRequest
    }
}

pub fn status_line(code: u16) -> String {
    let status = StatusCode::from_code(code);
    format!("SIP/2.0 {} {}\r\n", status.code(), status.reason())
}
