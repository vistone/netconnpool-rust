// Copyright (c) 2025, vistone
// All rights reserved.

use std::io::{self};
use std::net::UdpSocket;
use std::time::Duration;

/// clear_udp_read_buffer 清空UDP连接的读取缓冲区
/// 这对于防止UDP连接在连接池中复用时的数据混淆非常重要
/// 使用非阻塞模式快速清空缓冲区
pub fn clear_udp_read_buffer(
    socket: &UdpSocket,
    _timeout: Duration,
    max_packets: usize,
) -> io::Result<()> {
    // 切换到非阻塞模式
    socket.set_nonblocking(true)?;

    let mut buf = [0u8; 65536]; // 足够大的缓冲区
    let max = if max_packets == 0 { 100 } else { max_packets };

    for _ in 0..max {
        match socket.recv(&mut buf) {
            Ok(_) => {
                // 成功读取，继续清理
                continue;
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // 缓冲区已空
                break;
            }
            Err(_) => {
                // 其他错误（如连接重置），停止清理
                break;
            }
        }
    }

    // 恢复阻塞模式（假设默认是阻塞的，或者由调用者决定，但通常连接池中的连接默认为阻塞）
    // 注意：如果连接原本是非阻塞的，这里会强制设为阻塞。
    // 连接池中的连接通常期望是阻塞的。
    socket.set_nonblocking(false)?;

    Ok(())
}

/// has_udp_data_in_buffer 检查UDP连接读取缓冲区是否有数据
/// 返回true表示可能有数据，false表示缓冲区为空
pub fn has_udp_data_in_buffer(socket: &UdpSocket) -> bool {
    let _ = socket.set_nonblocking(true);
    let mut buf = [0u8; 1];
    let has_data = socket.peek(&mut buf).is_ok();
    let _ = socket.set_nonblocking(false);
    has_data
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::UdpSocket;

    #[test]
    fn test_has_udp_data_in_buffer() {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let _ = has_udp_data_in_buffer(&socket);
    }
}
