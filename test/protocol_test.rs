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
        assert_eq!(ParseProtocol("TCP"), Protocol::TCP);
        assert_eq!(ParseProtocol("UDP"), Protocol::UDP);
        assert_eq!(ParseProtocol("tcp"), Protocol::TCP);
        assert_eq!(ParseProtocol("unknown"), Protocol::Unknown);
    }

    #[test]
    fn test_protocol_methods() {
        assert!(Protocol::TCP.IsTCP());
        assert!(!Protocol::TCP.IsUDP());
        assert!(Protocol::UDP.IsUDP());
        assert!(!Protocol::UDP.IsTCP());
    }
}
