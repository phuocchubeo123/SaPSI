use std::net::TcpStream;
use std::io::{Write, Read};
use std::convert::TryInto;
use anyhow::{anyhow, Result};

pub struct TcpChannel {
    stream: TcpStream,
    bytes_sent: u64,
    bytes_received: u64,
}

impl TcpChannel {
    /// Creates a new TcpChannel
    pub fn new(stream: TcpStream) -> Self {
        Self { 
            stream, 
            bytes_sent: 0, 
            bytes_received: 0 
        }
    }
}

impl TcpChannel {
    /// Sends an array of bits over the TCP channel.
    /// This function will return the number of bytes sent.
    pub fn send_bits(&mut self, bits: &[bool]) -> Result<()> {
        // Serialize bits into bytes
        let mut buf = Vec::new();
        buf.extend_from_slice(&(bits.len() as u64).to_le_bytes()); // Store the length of bits
        let mut current_byte = 0u8;
        for (i, &bit) in bits.iter().enumerate() {
            if bit {
                current_byte |= 1 << (i % 8);
            }
            if i % 8 == 7 || i == bits.len() - 1 {
                buf.push(current_byte);
                current_byte = 0;
            }
        }

        self.send_u8(&buf)
            .map_err(|e| anyhow!("Failed to send bits: {:?}", e))?;

        Ok(())  // Return the number of bits sent
    }

    /// Receives an array of bits over the TCP channel.
    pub fn receive_bits(&mut self) -> Result<Vec<bool>> {
        let buf = self.receive_u8()
            .map_err(|e| anyhow!("Failed to receive bits: {:?}", e))?;

        // Deserialize bytes back into bits
        let num_bits = u64::from_le_bytes(buf[0..8].try_into().unwrap()) as usize;
        let mut bits = Vec::with_capacity(num_bits);
        for (i, &byte) in buf[8..].iter().enumerate() {
            for j in 0..8 {
                if i * 8 + j >= num_bits {
                    break;
                }
                bits.push((byte & (1 << j)) != 0);
            }
        }

        Ok(bits)
    }

    /// Sends a fixed-size block of data over the TCP channel.
    /// This function will return the number of bytes sent.
    pub fn send_block<const N: usize>(&mut self, data: &[[u8; N]]) -> Result<()> {
        let mut buf = Vec::new();
        for block in data {
            buf.extend_from_slice(block);
        }
        self.send_u8(&buf)
            .map_err(|e| anyhow!("Failed to send block: {:?}", e))?;
        Ok(())
    }

    /// Receives a fixed-size block of data over the TCP channel.
    pub fn receive_block<const N: usize>(&mut self) -> Result<Vec<[u8; N]>> {
        let buf = self.receive_u8()
            .map_err(|e| anyhow!("Failed to receive block: {:?}", e))?;
        let num_blocks = buf.len() / N;
        let mut data = Vec::with_capacity(num_blocks);
        for i in 0..num_blocks {
            let mut block = [0u8; N];
            block.copy_from_slice(&buf[i * N..(i + 1) * N]);
            data.push(block);
        }

        Ok(data)
    }

    /// Sends an array of 16-byte blocks over the TCP channel.
    pub fn send_16byte_block<const N: usize>(&mut self, data: &[[u128; N]]) -> Result<()> {
        let mut buf = Vec::new();
        for block in data {
            for &elem in block {
                buf.extend_from_slice(&elem.to_le_bytes());
            }
        }
        self.send_u8(&buf)
            .map_err(|e| anyhow!("Failed to send 16-byte block: {:?}", e))?;
        Ok(())
    }

    /// Receives an array of 16-byte blocks over the TCP channel.
    pub fn receive_16byte_block<const N: usize>(&mut self) -> Result<Vec<[u128; N]>> {
        let buf = self.receive_u8()
            .map_err(|e| anyhow!("Failed to receive 16-byte block: {:?}", e))?;
        let num_blocks = buf.len() / (16 * N);

        let mut data = Vec::with_capacity(num_blocks);
        for i in 0..num_blocks {
            let mut block = [0u128; N];
            for j in 0..N {
                let start = i * 16 * N + j * 16;
                let end = start + 16;
                block[j] = u128::from_le_bytes(buf[start..end].try_into().unwrap());
            }
            data.push(block);
        }

        Ok(data)
    }

    /// Sends a slice of u8 data over the TCP channel.
    /// This function will return the number of bytes sent.
    pub fn send_u8(&mut self, data: &[u8]) -> Result<()> {
        const CHUNK_SIZE: usize = 1024; // Define the chunk size in bytes

        // Send the total size first
        self.stream.write_all(&(data.len() as u64).to_le_bytes())?;

        // Send data in chunks
        for data_chunk in data.chunks(CHUNK_SIZE) {
            self.stream.write_all(data_chunk)?;
        }
        self.bytes_sent += 8 + data.len() as u64;
        self.flush()
            .map_err(|e| anyhow!("Failed to flush stream: {:?}", e))?;

        Ok(())
    }

    /// Receives u8 data over the TCP channel.
    pub fn receive_u8(&mut self) -> Result<Vec<u8>> {
        const CHUNK_SIZE: usize = 1024; // Define the chunk size in bytes

        // Read the total size of the data
        let mut size_buf = [0u8; 8];
        self.stream.read_exact(&mut size_buf)
            .map_err(|e| anyhow!("Failed to read size: {:?}", e))?;
        let total_size = u64::from_le_bytes(size_buf) as usize;

        // Receive the data in chunks
        let mut raw_data = vec![0u8; total_size];
        let mut received = 0;
        while received < total_size {
            let end = (received + CHUNK_SIZE).min(total_size);
            self.stream.read_exact(&mut raw_data[received..end])
                .map_err(|e| anyhow!("Failed to read data chunk: {:?}", e))?;
            received = end;
        }

        self.bytes_received += 8 + total_size as u64;

        Ok(raw_data)
    }

    /// Flushes the TCP stream.
    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }

    pub fn get_bytes_sent(&self) -> u64 {
        self.bytes_sent
    }

    pub fn get_bytes_received(&self) -> u64 {
        self.bytes_received
    }
}
