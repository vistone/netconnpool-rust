// Copyright (c) 2025, vistone
// All rights reserved.

use std::net::SocketAddr;

/// IPVersion IP版本类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IPVersion {
    /// IPVersionUnknown 未知IP版本
    Unknown = 0,
    /// IPVersionIPv4 IPv4
    IPv4 = 1,
    /// IPVersionIPv6 IPv6
    IPv6 = 2,
}

impl Default for IPVersion {
    fn default() -> Self {
        IPVersion::Unknown
    }
}

impl std::fmt::Display for IPVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IPVersion::IPv4 => write!(f, "IPv4"),
            IPVersion::IPv6 => write!(f, "IPv6"),
            IPVersion::Unknown => write!(f, "Unknown"),
        }
    }
}

/// detect_ip_version 检测连接的IP版本
/// 如果conn是实现了获取地址的trait，则检测其远程地址的IP版本
pub fn detect_ip_version(addr: &SocketAddr) -> IPVersion {
    match addr {
        SocketAddr::V4(_) => IPVersion::IPv4,
        SocketAddr::V6(_) => IPVersion::IPv6,
    }
}

/// parse_ip_version 从字符串解析IP版本
pub fn parse_ip_version(s: &str) -> IPVersion {
    match s.to_lowercase().as_str() {
        "ipv4" | "4" => IPVersion::IPv4,
        "ipv6" | "6" => IPVersion::IPv6,
        _ => IPVersion::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_ip_version_display() {
        assert_eq!(IPVersion::IPv4.to_string(), "IPv4");
        assert_eq!(IPVersion::IPv6.to_string(), "IPv6");
        assert_eq!(IPVersion::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_detect_ip_version() {
        let addr_v4 = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 8080);
        assert_eq!(detect_ip_version(&addr_v4), IPVersion::IPv4);

        let addr_v6 = SocketAddr::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1).into(), 8080);
        assert_eq!(detect_ip_version(&addr_v6), IPVersion::IPv6);
    }

    #[test]
    fn test_parse_ip_version() {
        assert_eq!(parse_ip_version("IPv4"), IPVersion::IPv4);
        assert_eq!(parse_ip_version("ipv4"), IPVersion::IPv4);
        assert_eq!(parse_ip_version("4"), IPVersion::IPv4);
        assert_eq!(parse_ip_version("IPv6"), IPVersion::IPv6);
        assert_eq!(parse_ip_version("ipv6"), IPVersion::IPv6);
        assert_eq!(parse_ip_version("6"), IPVersion::IPv6);
        assert_eq!(parse_ip_version("unknown"), IPVersion::Unknown);
    }
}
