use crate::types::{FormatParser, DataValue};
use rayon::prelude::*;

// ─────────────────────────────────────────────────────────────────────────────
// PCAP / PCAPNG File Signatures
// ─────────────────────────────────────────────────────────────────────────────
const PCAP_MAGIC_LE: u32      = 0xA1B2C3D4; // Little-Endian PCAP
const PCAP_MAGIC_BE: u32      = 0xD4C3B2A1; // Big-Endian PCAP
const PCAP_MAGIC_NANO_LE: u32 = 0xA1B23C4D; // Nanosecond-precision PCAP (LE)
const PCAPNG_MAGIC: u32       = 0x0A0D0D0A; // PCAPNG Section Header Block

const GLOBAL_HEADER_SIZE: usize = 24;
const PACKET_HEADER_SIZE: usize = 16;

/// Common Ethernet/IP magic bytes for Nitro-Payload-Scan
const MAGIC_PE:  &[u8] = b"MZ";       // Windows Portable Executable
const MAGIC_ZIP: &[u8] = b"PK\x03\x04"; // ZIP / Office documents
const MAGIC_PDF: &[u8] = b"%PDF";     // PDF documents
const MAGIC_ELF: &[u8] = b"\x7fELF"; // Linux ELF binary
const MAGIC_PNG: &[u8] = b"\x89PNG";  // PNG image

/// All magic bytes to scan for (for lateral exfiltration detection)
const FORENSIC_SIGNATURES: &[&[u8]] = &[
    MAGIC_PE, MAGIC_ZIP, MAGIC_PDF, MAGIC_ELF, MAGIC_PNG,
];

// ─────────────────────────────────────────────────────────────────────────────
// Zen-Flow: SIMD 5-Tuple Flow Key Hashing
// ─────────────────────────────────────────────────────────────────────────────

/// Network flow identifier (5-tuple) con soporte completo IPv6
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlowKey {
    pub src_ip:   u128,
    pub dst_ip:   u128,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: u8,
}

impl FlowKey {
    /// Bidirectional key: ensures A->B and B->A map to the same flow
    pub fn canonical(self) -> FlowKey {
        if (self.src_ip, self.src_port) > (self.dst_ip, self.dst_port) {
            FlowKey {
                src_ip:   self.dst_ip,
                dst_ip:   self.src_ip,
                src_port: self.dst_port,
                dst_port: self.src_port,
                protocol: self.protocol,
            }
        } else {
            self
        }
    }

    /// Zen-Flow hash: Optimized for 128-bit IPs
    #[inline(always)]
    pub fn zen_hash(&self) -> u64 {
        // Fold 128-bit IPs into 64-bit using XOR
        let src_folded = (self.src_ip as u64) ^ (self.src_ip >> 64) as u64;
        let dst_folded = (self.dst_ip as u64) ^ (self.dst_ip >> 64) as u64;
        
        let ip_bits = src_folded ^ (dst_folded.wrapping_shl(7) | dst_folded.wrapping_shr(57));
        let port_bits = (self.src_port as u64).wrapping_shl(16) | (self.dst_port as u64);
        let proto_bits = self.protocol as u64;

        let combined = ip_bits ^ (port_bits.wrapping_shl(17) | port_bits.wrapping_shr(47))
            ^ (proto_bits.wrapping_shl(59));

        let mut h = combined;
        h ^= h.wrapping_shr(33);
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h.wrapping_shr(33);
        h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
        h ^= h.wrapping_shr(33);
        h
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Packet Header (in PCAP global/record format)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct PcapPacket {
    pub ts_sec:   u32,
    pub ts_usec:  u32,
    pub incl_len: u32,
    pub orig_len: u32,
    pub payload_offset: usize,
}

/// Nitro-Payload-Scan: Escaneo AVX2 de firmas binarias
#[inline(always)]
pub fn nitro_payload_scan(payload: &[u8]) -> Option<&'static str> {
    if payload.is_empty() { return None; }

    // Fast path: signatures at the very beginning
    for (sig, name) in FORENSIC_SIGNATURES.iter().zip(["PE Executable", "ZIP/Office", "PDF Document", "ELF Binary", "PNG Image"]) {
        if payload.starts_with(sig) {
            return Some(name);
        }
    }

    // Nitro AVX2 Depth Scan (first 256 bytes)
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return unsafe { scan_payload_avx2(payload) };
        }
    }

    // Fallback scalar scan
    let scan_limit = payload.len().min(256);
    if scan_limit > 4 {
        for (sig, name) in FORENSIC_SIGNATURES.iter().zip(["PE Executable", "ZIP/Office", "PDF Document", "ELF Binary", "PNG Image"]) {
            let first = sig[0];
            let mut i = 1;
            while i <= scan_limit - sig.len() {
                if payload[i] == first && payload[i..].starts_with(sig) {
                    return Some(name);
                }
                i += 1;
            }
        }
    }
    None
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn scan_payload_avx2(payload: &[u8]) -> Option<&'static str> {
    use std::arch::x86_64::*;
    let scan_limit = payload.len().min(256);
    if scan_limit < 32 { return None; }

    for (sig, name) in FORENSIC_SIGNATURES.iter().zip(["PE Executable", "ZIP/Office", "PDF Document", "ELF Binary", "PNG Image"]) {
        let first_byte = _mm256_set1_epi8(sig[0] as i8);
        
        // Scan in 32-byte chunks
        for i in (0..(scan_limit - sig.len() + 1).min(scan_limit - 31)).step_by(32) {
            let chunk = _mm256_loadu_si256(payload.as_ptr().add(i) as *const __m256i);
            let mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, first_byte)) as u32;
            
            let mut temp_mask = mask;
            while temp_mask != 0 {
                let bit = temp_mask.trailing_zeros();
                let pos = i + bit as usize;
                if pos + sig.len() <= payload.len() && payload[pos..].starts_with(sig) {
                    return Some(name);
                }
                temp_mask &= !(1 << bit);
            }
        }
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// PcapNitroStream: Zero-copy PCAP iterator
// ─────────────────────────────────────────────────────────────────────────────

pub struct PcapNitroStream<'a> {
    data:       &'a [u8],
    cursor:     usize,
    swap_bytes: bool,
}

impl<'a> PcapNitroStream<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        if data.len() < GLOBAL_HEADER_SIZE { return None; }
        let magic = u32::from_le_bytes(data[0..4].try_into().ok()?);
        match magic {
            PCAP_MAGIC_LE | PCAP_MAGIC_NANO_LE => Some(Self { data, cursor: GLOBAL_HEADER_SIZE, swap_bytes: false }),
            PCAP_MAGIC_BE => Some(Self { data, cursor: GLOBAL_HEADER_SIZE, swap_bytes: true }),
            PCAPNG_MAGIC => Some(Self { data, cursor: 28, swap_bytes: false }),
            _ => None,
        }
    }

    #[inline(always)]
    fn read_u32(&self, offset: usize) -> u32 {
        let raw = u32::from_le_bytes(self.data[offset..offset+4].try_into().unwrap_or([0;4]));
        if self.swap_bytes { raw.swap_bytes() } else { raw }
    }

    pub fn scan_packets<F>(&mut self, mut cb: F)
    where
        F: FnMut(PcapPacket, &[u8]),
    {
        while self.cursor + PACKET_HEADER_SIZE <= self.data.len() {
            let ts_sec   = self.read_u32(self.cursor);
            let incl_len = self.read_u32(self.cursor + 8);
            let orig_len = self.read_u32(self.cursor + 12);

            let payload_offset = self.cursor + PACKET_HEADER_SIZE;
            let incl_len_usize = incl_len as usize;

            if payload_offset + incl_len_usize > self.data.len() { break; }
            let packet_bytes = &self.data[payload_offset..payload_offset + incl_len_usize];
            cb(PcapPacket { ts_sec, ts_usec: 0, incl_len, orig_len, payload_offset }, packet_bytes);
            self.cursor = payload_offset + incl_len_usize;
        }
    }

    pub fn extract_flow_key(packet: &[u8]) -> Option<FlowKey> {
        if packet.len() < 14 { return None; }
        let mut eth_offset = 12;
        let mut ether_type = u16::from_be_bytes([packet[eth_offset], packet[eth_offset+1]]);
        
        if ether_type == 0x8100 && packet.len() >= 18 {
            eth_offset += 4;
            ether_type = u16::from_be_bytes([packet[eth_offset], packet[eth_offset+1]]);
        }

        let ip_header_start = eth_offset + 2;
        let (ip_header_len, protocol, src_ip, dst_ip) = match ether_type {
            0x0800 => {
                if packet.len() < ip_header_start + 20 { return None; }
                let ihl = (packet[ip_header_start] & 0x0F) as usize * 4;
                let proto = packet[ip_header_start + 9];
                let src = u32::from_be_bytes(packet[ip_header_start + 12..ip_header_start + 16].try_into().ok()?);
                let dst = u32::from_be_bytes(packet[ip_header_start + 16..ip_header_start + 20].try_into().ok()?);
                // Map IPv4 to IPv6-mapped address (::ffff:a.b.c.d)
                (ihl, proto, src as u128 | 0x0000_0000_0000_0000_0000_FFFF_0000_0000, dst as u128 | 0x0000_0000_0000_0000_0000_FFFF_0000_0000)
            },
            0x86DD => {
                if packet.len() < ip_header_start + 40 { return None; }
                let proto = packet[ip_header_start + 6];
                let src = u128::from_be_bytes(packet[ip_header_start + 8..ip_header_start + 24].try_into().ok()?);
                let dst = u128::from_be_bytes(packet[ip_header_start + 24..ip_header_start + 40].try_into().ok()?);
                (40, proto, src, dst)
            },
            _ => return None,
        };

        let l4_offset = ip_header_start + ip_header_len;
        let (src_port, dst_port) = match protocol {
            6 | 17 => {
                if l4_offset + 4 > packet.len() { (0, 0) } else {
                    (
                        u16::from_be_bytes([packet[l4_offset], packet[l4_offset+1]]),
                        u16::from_be_bytes([packet[l4_offset+2], packet[l4_offset+3]]),
                    )
                }
            },
            _ => (0, 0),
        };

        Some(FlowKey { src_ip, dst_ip, src_port, dst_port, protocol }.canonical())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parallel PCAP Analysis
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
pub struct PcapSummary {
    pub total_packets:    u64,
    pub total_bytes:      u64,
    pub unique_flows:     u64,
    pub tcp_packets:      u64,
    pub udp_packets:      u64,
    pub suspicious_payloads: u64,
}

pub fn analyze_pcap(data: &[u8]) -> PcapSummary {
    if data.len() < 128 { return PcapSummary::default(); }
    
    // Divide buffer into chunks for Rayon
    let num_chunks = rayon::current_num_threads().max(1);
    let chunk_size = data.len() / num_chunks;
    
    let summaries: Vec<PcapSummary> = (0..num_chunks).into_par_iter().map(|idx| {
        let start = idx * chunk_size;
        let end = if idx == num_chunks - 1 { data.len() } else { (idx + 1) * chunk_size };
        let sub_data = &data[start..end];
        
        let mut local_summary = PcapSummary::default();
        let mut flow_set = std::collections::HashSet::<u64>::new();
        
        // Parallel PCAP Sync Heuristic
        let mut cursor = 0;
        if idx > 0 {
            // Find next valid packet header
            while cursor + 16 < sub_data.len() {
                let incl = u32::from_le_bytes(sub_data[cursor+8..cursor+12].try_into().unwrap_or([0;4]));
                let orig = u32::from_le_bytes(sub_data[cursor+12..cursor+16].try_into().unwrap_or([0;4]));
                if incl > 0 && incl <= 65535 && incl <= orig {
                    break;
                }
                cursor += 1;
            }
        } else {
            cursor = GLOBAL_HEADER_SIZE;
        }

        let mut stream = PcapNitroStream { data: sub_data, cursor, swap_bytes: false };
        stream.scan_packets(|pkt, bytes| {
            local_summary.total_packets += 1;
            local_summary.total_bytes   += pkt.orig_len as u64;

            if let Some(key) = PcapNitroStream::extract_flow_key(bytes) {
                flow_set.insert(key.zen_hash());
                match key.protocol {
                    6  => local_summary.tcp_packets += 1,
                    17 => local_summary.udp_packets += 1,
                    _  => {},
                }
            }
            if nitro_payload_scan(bytes).is_some() {
                local_summary.suspicious_payloads += 1;
            }
        });
        local_summary.unique_flows = flow_set.len() as u64;
        local_summary
    }).collect();

    // Reduce summaries
    summaries.into_iter().fold(PcapSummary::default(), |mut acc, x| {
        acc.total_packets += x.total_packets;
        acc.total_bytes   += x.total_bytes;
        acc.tcp_packets   += x.tcp_packets;
        acc.udp_packets   += x.udp_packets;
        acc.suspicious_payloads += x.suspicious_payloads;
        acc.unique_flows += x.unique_flows;
        acc
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Trait Implementation
// ─────────────────────────────────────────────────────────────────────────────

pub struct PcapParser;
impl PcapParser {
    pub fn new() -> Self { Self }
}

impl FormatParser for PcapParser {
    fn probe(&self, buffer: &[u8]) -> bool {
        if buffer.len() < 4 { return false; }
        let magic = u32::from_le_bytes(buffer[0..4].try_into().unwrap_or([0;4]));
        matches!(magic, PCAP_MAGIC_LE | PCAP_MAGIC_BE | PCAP_MAGIC_NANO_LE | PCAPNG_MAGIC)
    }

    fn find_boundaries(&self, _mmap: &[u8], start: usize, end: usize) -> (usize, usize) {
        (start, end)
    }

    fn parse_unit(&self, data: &[u8]) -> anyhow::Result<Vec<DataValue>> {
        if data.len() < PACKET_HEADER_SIZE { return Ok(vec![DataValue::Null]); }
        Ok(vec![DataValue::Int(u32::from_le_bytes(data[0..4].try_into()?) as i64)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_key_canonical_symmetry() {
        let a = FlowKey { src_ip: 10, dst_ip: 20, src_port: 1234, dst_port: 80, protocol: 6 };
        let b = FlowKey { src_ip: 20, dst_ip: 10, src_port: 80, dst_port: 1234, protocol: 6 };
        assert_eq!(a.canonical().zen_hash(), b.canonical().zen_hash());
    }

    #[test]
    fn test_nitro_payload_scan_pe() {
        let mut payload = vec![0u8; 64];
        payload[10] = b'M'; payload[11] = b'Z'; // Offset scan
        assert_eq!(nitro_payload_scan(&payload), Some("PE Executable"));
    }
}
