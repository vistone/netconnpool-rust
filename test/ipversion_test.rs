// Copyright (c) 2025, vistone
// All rights reserved.

#[cfg(test)]
mod tests {
    use netconnpool::*;
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

    #[test]
    fn test_ip_version() {
        assert_eq!(IPVersion::IPv4.to_string(), "IPv4");
        assert_eq!(IPVersion::IPv6.to_string(), "IPv6");
        assert_eq!(IPVersion::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_detect_ip_version() {
        let addr_v4 = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 8080);
        assert_eq!(DetectIPVersion(&addr_v4), IPVersion::IPv4);

        let addr_v6 = SocketAddr::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1).into(), 8080);
        assert_eq!(DetectIPVersion(&addr_v6), IPVersion::IPv6);
    }

    #[test]
    fn test_parse_ip_version() {
        assert_eq!(ParseIPVersion("IPv4"), IPVersion::IPv4);
        assert_eq!(ParseIPVersion("ipv4"), IPVersion::IPv4);
        assert_eq!(ParseIPVersion("4"), IPVersion::IPv4);
        assert_eq!(ParseIPVersion("IPv6"), IPVersion::IPv6);
        assert_eq!(ParseIPVersion("ipv6"), IPVersion::IPv6);
        assert_eq!(ParseIPVersion("6"), IPVersion::IPv6);
        assert_eq!(ParseIPVersion("unknown"), IPVersion::Unknown);
    }
}
