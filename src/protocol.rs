// Copyright (c) 2025, vistone
// All rights reserved.

use std::net::{TcpStream, UdpSocket};

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

/// detect_protocol 检测连接的协议类型
/// 支持TCP和UDP连接
/// 注意：这个函数在实际使用中会通过具体类型来判断，这里提供占位实现
pub fn detect_protocol(_conn: &dyn std::any::Any) -> Protocol {
    // 实际实现会在 connection.rs 中通过具体类型判断
    Protocol::Unknown
}

/// detect_protocol_from_addr 从地址判断协议类型
pub fn detect_protocol_from_addr(addr: &std::net::SocketAddr) -> Protocol {
    match addr {
        std::net::SocketAddr::V4(_) | std::net::SocketAddr::V6(_) => {
            // 无法从 SocketAddr 直接判断协议，需要从连接类型判断
            Protocol::Unknown
        }
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

// 辅助函数：从具体类型检测协议
pub fn detect_protocol_from_tcp(_: &TcpStream) -> Protocol {
    Protocol::TCP
}

pub fn detect_protocol_from_udp(_: &UdpSocket) -> Protocol {
    Protocol::UDP
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
