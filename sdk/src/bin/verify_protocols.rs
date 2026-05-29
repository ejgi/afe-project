use zen_engine::parsers::forensics::pcap::PcapNitroStream;
use colored::*;

fn main() -> anyhow::Result<()> {
    let pcap_path = "test_massive.pcap";
    let data = std::fs::read(pcap_path)?;
    let mut stream = PcapNitroStream::new(&data).unwrap();

    println!("\n🔍 {} ...", "INSPECTOR DE PROTOCOLO NITRO".bold().cyan());
    
    let mut count = 0;
    stream.scan_packets(|_pkt, bytes| {
        if count < 10 {
            let ether_type = u16::from_be_bytes([bytes[12], bytes[13]]);
            print!("📦 Pkt #{:<2} | EtherType: 0x{:04X} | ", count, ether_type);
            
            if let Some(key) = PcapNitroStream::extract_flow_key(bytes) {
                println!("✅ Flow: {} {}:{} -> {}:{} (Proto: {}) | Hash: 0x{:016X}", 
                    "DETECTED".green(), 
                    format_ip(key.src_ip), key.src_port,
                    format_ip(key.dst_ip), key.dst_port,
                    key.protocol,
                    key.zen_hash()
                );
            } else {
                println!("❌ Flow: {}", "FAILED TO EXTRACT".red());
            }
        }
        count += 1;
    });

    Ok(())
}

fn format_ip(ip: u128) -> String {
    
    if (ip >> 32) == 0x0000_0000_0000_0000_0000_FFFF {
        // IPv4-mapped IPv6
        let v4 = (ip & 0xFFFFFFFF) as u32;
        format!("{}.{}.{}.{}", (v4 >> 24) & 0xff, (v4 >> 16) & 0xff, (v4 >> 8) & 0xff, v4 & 0xff)
    } else {
        // Native IPv6
        let bytes = ip.to_be_bytes();
        let ipv6 = std::net::Ipv6Addr::from(bytes);
        ipv6.to_string()
    }
}
