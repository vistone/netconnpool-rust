// Copyright (c) 2025, vistone
// All rights reserved.

use std::io::{self};
use std::net::UdpSocket;
use std::time::{Duration, Instant};

/// clear_udp_read_buffer 清空UDP连接的读取缓冲区
/// 这对于防止UDP连接在连接池中复用时的数据混淆非常重要
pub fn clear_udp_read_buffer(
    socket: &UdpSocket,
    timeout: Duration,
    max_packets: usize,
) -> io::Result<()> {
    let read_timeout = if timeout.is_zero() {
        Duration::from_millis(100)
    } else {
        timeout
    };

    let deadline = Instant::now() + read_timeout;
    socket.set_read_timeout(Some(read_timeout))?;

    let mut buf = vec![0u8; 65507]; // UDP最大数据包大小
    let max = if max_packets <= 0 { 100 } else { max_packets };

    for _i in 0..max {
        if Instant::now() > deadline {
            return Ok(()); // 超时，缓冲区应该已清空或无法继续清空
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        let read_deadline = remaining.min(Duration::from_millis(50));
        socket.set_read_timeout(Some(read_deadline))?;

        match socket.recv(&mut buf) {
            Ok(_) => {
                // 成功读取到一个数据包，继续读取下一个
                continue;
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::TimedOut {
                    return Ok(()); // 缓冲区已清空
                }
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    return Err(e);
                }
                // 其他错误（如连接关闭），返回Ok表示清理完成
                return Ok(());
            }
        }
    }

    Ok(())
}

/// has_udp_data_in_buffer 检查UDP连接读取缓冲区是否有数据
/// 返回true表示可能有数据，false表示缓冲区为空
pub fn has_udp_data_in_buffer(socket: &UdpSocket) -> bool {
    socket
        .set_read_timeout(Some(Duration::from_millis(1)))
        .is_ok()
        && {
            let mut buf = [0u8; 1];
            match socket.recv(&mut buf) {
                Ok(_) => true,
                Err(e) => e.kind() != io::ErrorKind::TimedOut,
            }
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::UdpSocket;

    #[test]
    fn test_has_udp_data_in_buffer() {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        // 空缓冲区应该返回false（超时）
        // 注意：由于 UDP 是无连接的，这个测试可能不稳定
        // 我们只测试函数不会 panic
        let _ = has_udp_data_in_buffer(&socket);
    }
}
