use zen_engine::utils::is_rotational;

fn main() {
    let path = "/home/archtech/Descargas/FIRST-2015_Hands-on_Network_Forensics_PCAP";
    let is_hdd = is_rotational(path);
    println!("Path: {}", path);
    println!("Is HDD: {}", is_hdd);
    
    // Also check the base Descargas folder
    let base = "/home/archtech/Descargas";
    println!("Base: {}", base);
    println!("Is HDD: {}", is_rotational(base));
}
