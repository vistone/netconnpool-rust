// Copyright (c) 2025, vistone
// All rights reserved.

#[cfg(test)]
mod tests {
    use netconnpool::*;

    #[test]
    fn test_protocol() {
        assert_eq!(Protocol::TCP.to_string(), "TCP");
        assert_eq!(Protocol::UDP.to_string(), "UDP");
        assert_eq!(Protocol::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_parse_protocol() {
        assert_eq!(parse_protocol("TCP"), Protocol::TCP);
        assert_eq!(parse_protocol("UDP"), Protocol::UDP);
        assert_eq!(parse_protocol("tcp"), Protocol::TCP);
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
