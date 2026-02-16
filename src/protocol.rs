// Copyright (c) 2025, vistone
// All rights reserved.

use crate::config::ConnectionType;

/// Protocol 协议类型
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    /// ProtocolUnknown 未知协议
    #[default]
    Unknown = 0,
    /// ProtocolTCP TCP协议
    TCP = 1,
    /// ProtocolUDP UDP协议
    UDP = 2,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::TCP => write!(f, "TCP"),
            Protocol::UDP => write!(f, "UDP"),
            Protocol::Unknown => write!(f, "Unknown"),
        }
    }
}

/// detect_protocol 从连接类型检测协议
pub fn detect_protocol(conn: &ConnectionType) -> Protocol {
    match conn {
        ConnectionType::Tcp(_) => Protocol::TCP,
        ConnectionType::Udp(_) => Protocol::UDP,
    }
}

/// parse_protocol 从字符串解析协议类型
pub fn parse_protocol(s: &str) -> Protocol {
    match s.to_uppercase().as_str() {
        "TCP" => Protocol::TCP,
        "UDP" => Protocol::UDP,
        _ => Protocol::Unknown,
    }
}

impl Protocol {
    /// is_tcp 检查是否为TCP协议
    pub fn is_tcp(&self) -> bool {
        matches!(self, Protocol::TCP)
    }

    /// is_udp 检查是否为UDP协议
    pub fn is_udp(&self) -> bool {
        matches!(self, Protocol::UDP)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConnectionType;
    use std::net::UdpSocket;

    #[test]
    fn test_protocol_display() {
        assert_eq!(Protocol::TCP.to_string(), "TCP");
        assert_eq!(Protocol::UDP.to_string(), "UDP");
        assert_eq!(Protocol::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_parse_protocol() {
        assert_eq!(parse_protocol("TCP"), Protocol::TCP);
        assert_eq!(parse_protocol("UDP"), Protocol::UDP);
        assert_eq!(parse_protocol("tcp"), Protocol::TCP);
        assert_eq!(parse_protocol("udp"), Protocol::UDP);
        assert_eq!(parse_protocol("unknown"), Protocol::Unknown);
    }

    #[test]
    fn test_protocol_methods() {
        assert!(Protocol::TCP.is_tcp());
        assert!(!Protocol::TCP.is_udp());
        assert!(Protocol::UDP.is_udp());
        assert!(!Protocol::UDP.is_tcp());
    }

    #[test]
    fn test_detect_protocol() {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let conn = ConnectionType::Udp(socket);
        assert_eq!(detect_protocol(&conn), Protocol::UDP);
    }

    #[test]
    fn test_detect_protocol_tcp() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stream = std::net::TcpStream::connect(addr).unwrap();
        let conn = ConnectionType::Tcp(stream);
        assert_eq!(detect_protocol(&conn), Protocol::TCP);
    }
}
