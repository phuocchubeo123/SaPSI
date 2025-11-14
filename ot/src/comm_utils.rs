use psi_network::tcp_channel::TcpChannel;
use p256::EncodedPoint;
use anyhow::{anyhow, Result};

// A wrapper around the TCP stream implemented in psi_network
// Implement sending and receiving p256 points over the channel

/// Sends an elliptic curve point over the TCP channel.
/// This function will return the number of bytes sent.
pub fn send_point(point: &EncodedPoint, channel: &mut TcpChannel) -> Result<()> {
    let point_bytes = point.as_bytes(); // Serialize the point
    channel.send_u8(point_bytes)
        .map_err(|e| anyhow!("Failed to send point: {:?}", e))?;

    Ok(())  // Return the number of bytes sent
}

/// Receives an elliptic curve point over the TCP channel.
pub fn receive_point(channel: &mut TcpChannel) -> Result<EncodedPoint> {
    let point_bytes = channel.receive_u8()
        .map_err(|e| anyhow!("Failed to receive point: {:?}", e))?;

    EncodedPoint::from_bytes(&point_bytes).map_err(|e| {
        anyhow!("Invalid point received : {:?}", e)
    })
}
