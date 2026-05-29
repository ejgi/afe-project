use std::fs::File;
use std::io::Write;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let num_chunks = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);
    let num_packets = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10_000);

    println!("🚀 Generando evidencias forenses sintéticas (Chunks: {}, Packets: {})...", num_chunks, num_packets);

    generate_evtx("test_massive.evtx", num_chunks, 100)?; 
    generate_pcap("test_massive.pcap", num_packets)?; 

    println!("✅ Archivos generados.");
    Ok(())
}

fn generate_evtx(name: &str, num_chunks: usize, records_per_chunk: usize) -> anyhow::Result<()> {
    let mut file = File::create(name)?;
    
    let mut header = vec![0u8; 4096];
    header[0..8].copy_from_slice(b"ElfFile\x00");
    file.write_all(&header)?;

    for c in 0..num_chunks {
        let mut chunk = vec![0u8; 65536];
        chunk[0..8].copy_from_slice(b"ElfChnk\x00");
        chunk[8..16].copy_from_slice(&(c as u64 * records_per_chunk as u64 + 1).to_le_bytes());
        
        for i in 0..records_per_chunk {
            let offset = 512 + (i * 128);
            if offset + 128 > 65536 { break; }
            // Record Magic
            chunk[offset..offset+4].copy_from_slice(&[0x2a, 0x2a, 0x00, 0x00]);
            // Record Size (128 bytes)
            chunk[offset+4..offset+8].copy_from_slice(&128u32.to_le_bytes());
            
            chunk[offset+8..offset+16].copy_from_slice(&(c as u64 * records_per_chunk as u64 + i as u64 + 1).to_le_bytes());
            
            // Inyectar un timestamp corrupto (0) en el primer registro del primer chunk
            let ts = if c == 0 && i == 0 { 0u64 } else { 133500000000000000u64 + i as u64 };
            chunk[offset+16..offset+24].copy_from_slice(&ts.to_le_bytes());

            let p_off = offset + 24;
            chunk[p_off] = 0x05; chunk[p_off + 1] = 0x04; 
            chunk[p_off + 2] = 0xE8; chunk[p_off + 3] = 0x03;
        }
        file.write_all(&chunk)?;
    }
    Ok(())
}

fn generate_pcap(name: &str, num_packets: usize) -> anyhow::Result<()> {
    let mut file = File::create(name)?;
    
    let mut global_header = vec![0u8; 24];
    global_header[0..4].copy_from_slice(&0xA1B2C3D4u32.to_le_bytes());
    global_header[20..24].copy_from_slice(&1u32.to_le_bytes()); // Ethernet
    file.write_all(&global_header)?;

    // Generar mezcla de paquetes: IPv4, IPv6 y VLAN
    for i in 0..num_packets {
        let mut packet_header = vec![0u8; 16];
        let incl_len = 100u32;
        packet_header[0..4].copy_from_slice(&1710530000u32.to_le_bytes());
        packet_header[8..12].copy_from_slice(&incl_len.to_le_bytes());
        packet_header[12..16].copy_from_slice(&incl_len.to_le_bytes());
        file.write_all(&packet_header)?;

        let mut data = vec![0u8; 100];
        // Dest/Src MAC
        data[0..6].copy_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        data[6..12].copy_from_slice(&[0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB]);

        if i % 3 == 0 {
            // IPv4 Normal
            data[12] = 0x08; data[13] = 0x00;
            data[14] = 0x45; data[14+9] = 6; // TCP
            data[14+12..14+16].copy_from_slice(&[10,0,0,1]);
            data[14+16..14+20].copy_from_slice(&[10,0,0,2]);
            data[34] = 0x04; data[35] = 0xD2; // Source Port 1234
        } else if i % 3 == 1 {
            // VLAN tagged IPv4 (802.1Q)
            data[12] = 0x81; data[13] = 0x00; // TPID
            data[14] = 0x00; data[15] = 0x64; // TCI (VLAN 100)
            data[16] = 0x08; data[17] = 0x00; // EtherType IPv4
            data[18] = 0x45; data[18+9] = 17; // UDP
            data[18+12..18+16].copy_from_slice(&[192,168,10,1]);
            data[18+16..18+20].copy_from_slice(&[192,168,10,2]);
            data[38] = 0x04; data[39] = 0xD2; // Source Port 1234
        } else {
            // IPv6
            data[12] = 0x86; data[13] = 0xDD;
            data[14+6] = 6; // TCP
            data[14+8..14+12].copy_from_slice(&[0xFE, 0x80, 0x00, 0x01]); // Fake Src
            data[14+24..14+28].copy_from_slice(&[0xFE, 0x80, 0x00, 0x02]); // Fake Dst
            data[54] = 0x1F; data[55] = 0x90; // Source Port 8080
        }

        // Inyectar firma Nitro-Payload-Scan en el medio (para probar la optimización de escaneo)
        if i == 5 {
            data[60..62].copy_from_slice(b"MZ"); 
        }

        file.write_all(&data)?;
    }
    Ok(())
}
