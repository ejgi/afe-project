#[inline(always)]
pub fn parse_numeric_fast(bytes: &[u8]) -> Option<f64> {
    let mut i = 0;
    let mut end = bytes.len();
    
    // Handle quotes
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        i = 1;
        end = bytes.len() - 1;
    }

    while i < end && bytes[i].is_ascii_whitespace() { i += 1; }
    if i == end { return None; }
    
    let mut sign = 1.0;
    if bytes[i] == b'-' { sign = -1.0; i += 1; }
    else if bytes[i] == b'+' { i += 1; }
    
    let mut num = 0.0;
    let mut frac = 0.0;
    let mut frac_div = 1.0;
    let mut has_dot = false;
    let mut has_digits = false;
    
    while i < end {
        let b = bytes[i];
        if b >= b'0' && b <= b'9' {
            has_digits = true;
            if has_dot {
                frac = frac * 10.0 + (b - b'0') as f64;
                frac_div *= 10.0;
            } else {
                num = num * 10.0 + (b - b'0') as f64;
            }
        } else if b == b'.' && !has_dot {
            has_dot = true;
        } else {
            break;
        }
        i += 1;
    }
    
    if has_digits { Some(sign * (num + frac / frac_div)) } else { None }
}

#[inline(always)]
pub fn parse_date_fast(bytes: &[u8]) -> Option<u32> {
    let mut b = bytes;
    if b.first() == Some(&b'"') && b.last() == Some(&b'"') {
        if b.len() >= 2 { b = &b[1..b.len()-1]; }
    }
    if b.len() >= 10 && b[4] == b'-' && b[7] == b'-' {
        let mut all_digits = true;
        for i in [0, 1, 2, 3, 5, 6, 8, 9] {
            if !b[i].is_ascii_digit() {
                all_digits = false;
                break;
            }
        }
        if all_digits {
            let y = (b[0] - b'0') as u32 * 1000 + (b[1] - b'0') as u32 * 100 + (b[2] - b'0') as u32 * 10 + (b[3] - b'0') as u32;
            let m = (b[5] - b'0') as u32 * 10 + (b[6] - b'0') as u32;
            let d = (b[8] - b'0') as u32 * 10 + (b[9] - b'0') as u32;
            if y <= 9999 && m >= 1 && m <= 12 && d >= 1 && d <= 31 {
                return Some(y * 10000 + m * 100 + d);
            }
        }
    }
    None
}

#[inline(always)]
pub fn is_ip_fast(bytes: &[u8]) -> bool {
    let mut dots = 0;
    let mut colons = 0;
    let mut digits = 0;
    for &b in bytes {
        if b == b'.' { dots += 1; }
        else if b == b':' { colons += 1; }
        else if (b >= b'0' && b <= b'9') || (b >= b'a' && b <= b'f') || (b >= b'A' && b <= b'F') { digits += 1; }
    }
    (dots == 3 && digits >= 3) || (colons >= 2 && digits >= 3)
}

#[inline(always)]
pub fn is_mac_fast(bytes: &[u8]) -> bool {
    if bytes.len() != 17 { return false; }
    let mut separators = 0;
    for i in 0..bytes.len() {
        let b = bytes[i];
        if i % 3 == 2 {
            if b == b':' || b == b'-' { separators += 1; }
            else { return false; }
        } else {
            if !b.is_ascii_hexdigit() { return false; }
        }
    }
    separators == 5
}

use std::path::Path;
use sysinfo::{System, Disks};
use crate::types::HardwareSpecs;

#[inline(always)]
pub fn compute_hash_fast(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub fn is_rotational<P: AsRef<Path>>(path: P) -> bool {
    // Robust check for Linux: probe /sys/block via device number
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(metadata) = std::fs::metadata(path.as_ref()) {
            let dev = metadata.dev();
            let major = (dev >> 8) & 0xfff;
            let minor = (dev & 0xff) | ((dev >> 12) & 0xfff00);
            
            // Try to find the block device in /sys/dev/block/
            let base_path = format!("/sys/dev/block/{}:{}", major, minor);
            
            // 1. Direct block device check
            let sys_path = format!("{}/queue/rotational", base_path);
            if let Ok(content) = std::fs::read_to_string(&sys_path) {
                return content.trim() == "1";
            }
            
            // 2. Partition parent check (e.g. sda1 -> sda) via sysfs parent directory
            let parent_sys_path = format!("{}/../queue/rotational", base_path);
            if let Ok(content) = std::fs::read_to_string(&parent_sys_path) {
                return content.trim() == "1";
            }

            // 3. Robust sysfs traversal (follow symlink to device)
            if let Ok(real_path) = std::fs::read_link(&base_path) {
                let mut p = std::path::PathBuf::from("/sys/dev/block");
                p.push(real_path);
                
                let mut current = p.as_path();
                for _ in 0..4 {
                    if let Some(parent) = current.parent() {
                        let q = parent.join("queue/rotational");
                        if let Ok(content) = std::fs::read_to_string(&q) {
                            return content.trim() == "1";
                        }
                        current = parent;
                    } else { break; }
                }
            }
        }
    }
    
    // 4. Cross-platform fallback using sysinfo
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let abs_path = std::fs::canonicalize(path.as_ref()).unwrap_or_else(|_| path.as_ref().to_path_buf());
    
    let mut best_mount: Option<&std::path::Path> = None;
    let mut is_hdd_found = false;
    
    for disk in &disks {
        let mount = disk.mount_point();
        if abs_path.starts_with(mount) {
            if best_mount.is_none() || mount.as_os_str().len() > best_mount.unwrap().as_os_str().len() {
                best_mount = Some(mount);
                is_hdd_found = disk.kind() == sysinfo::DiskKind::HDD;
            }
        }
    }
    
    if best_mount.is_some() {
        return is_hdd_found;
    }

    // Fallback to path hints
    let path_str = path.as_ref().to_string_lossy().to_lowercase();
    path_str.contains("descargas") || path_str.contains("/mnt/hdd") || path_str.contains("/media/hdd") || path_str.contains("ext-")
}

pub fn get_gpu_info() -> Option<crate::types::HardwareGpu> {
    None
}

pub fn get_hardware_specs() -> crate::types::HardwareSpecs {
    let mut sys = System::new_all();
    sys.refresh_all();
    sys.refresh_cpu_all();
    let os_name = System::name().unwrap_or_else(|| {
        if cfg!(target_os = "windows") { "Windows".to_string() }
        else if cfg!(target_os = "macos") { "macOS".to_string() }
        else { "Linux".to_string() }
    });
    
    let os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());
    
    // Convert to GB accurately
    const BYTES_IN_GB: f64 = (1024 * 1024 * 1024) as f64;
    let total_memory = sys.total_memory() as f64 / BYTES_IN_GB;
    let free_memory = (sys.total_memory() - sys.used_memory()) as f64 / BYTES_IN_GB;
    
    let cpu_cores = sys.cpus().len();
    
    let mut cpu_brand = sys.cpus().first()
        .map(|c| c.brand().trim().to_string())
        .unwrap_or_else(|| "General Purpose Processor".to_string());
    
    if cpu_brand.is_empty() || cpu_brand == "Unknown" {
        cpu_brand = "Zen-Compatible Processor".to_string();
    }
    
    // Clean up brand string
    cpu_brand = cpu_brand.replace("(R)", "").replace("(TM)", "").replace("  ", " ").trim().to_string();

    // Cross-platform storage detection
    let disks = Disks::new_with_refreshed_list();
    let mut storage_type = "SSD/M.2".to_string();
    
    for disk in &disks {
        if disk.is_removable() { continue; }
        match disk.kind() {
            sysinfo::DiskKind::HDD => {
                storage_type = "HDD".to_string();
                break;
            }
            _ => {
                // On Windows, if kind is Unknown, we might check partition names or assume SSD
                #[cfg(target_os = "linux")]
                if is_rotational(disk.mount_point()) {
                    storage_type = "HDD".to_string();
                    break;
                }
            }
        }
    }

    HardwareSpecs {
        os_name,
        os_version,
        total_memory_gb: total_memory,
        free_memory_gb: free_memory,
        cpu_cores,
        cpu_brand,
        storage_type,
        gpu: get_gpu_info(),
    }
}

pub fn log_telemetry(path: &Path, op: &str, duration: std::time::Duration, size: u64) {
    if size < 1_000_000_000 { return; } // Only log files > 1GB
    
    let hw = get_hardware_specs();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    let throughput = (size as f64 / 1024.0 / 1024.0 / 1024.0) / duration.as_secs_f64();
    
    let telemetry_path = "/home/archtech/programas/zen_engine_SDK_FINAL_READY/zen_telemetry.csv";
    let file_exists = Path::new(telemetry_path).exists();
    
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(telemetry_path)
        .ok();
        
    if let Some(ref mut f) = file {
        if !file_exists {
            let _ = writeln!(f, "timestamp,os,cpu,cores,ram_gb,storage,path,size_mb,op,duration_sec,throughput_gbs");
        }
        let _ = writeln!(f, "{},{},{},{},{:.1},{},{},{},{},{:.3},{:.3}",
            ts, hw.os_name, hw.cpu_brand, hw.cpu_cores, hw.total_memory_gb,
            hw.storage_type, path.display(), size / 1024 / 1024, op, 
            duration.as_secs_f64(), throughput);
    }
}
