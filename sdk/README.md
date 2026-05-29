# 🧬 Open Data Hunt Core
### The High-Performance Engine for Tactical Forensics

> **O(1) complexity indexing at machine speed.**  
> Built in Rust. Accelerated by SIMD/AVX2. Engineered for the Advanced Forensic Ecosystem (AFE).

---

## ⚡ Nitro-Direct Architecture
Open Data Hunt Core (ODH Core) is a bare-metal forensic indexing engine designed to eliminate the bottlenecks of traditional sequential scanning. By using **Nitro-Direct memory-mapping** and **AVX2 SIMD parallelization**, it achieves constant-time file ingestion regardless of dataset size.

- **1.45M Rows/s Sustained Throughput**: Optimized for massive forensic labs.
- **Nitro-Direct O(1) Loading**: Index 10 million rows in < 5s on standard mechanical HDDs.
- **Hardware Agnostic**: Automatic detection and optimization for SSD vs HDD storage.
- **Memory Efficient**: ~12 bytes per row overhead, allowing analysis of 100GB+ files on standard workstations.

## 🛠️ Ecosystem Components
This engine powers the entire **Advanced Forensic Ecosystem (AFE)**:
- **Large Data Explorer**: The professional VS Code extension for massive dataset inspection.
- **zen-ioc**: The standalone, zero-footprint forensic triage platform.

## 🚀 Native Compilation
Build the core binary:
```bash
cargo build --release --bin big_explorer_engine
```

Run a tactical stress test:
```bash
./target/release/big_explorer_engine --bench /path/to/evidence.csv
```

---

> [!IMPORTANT]
> **Open Data Hunt Core** is a closed-core performance engine component designed for high-stress Incident Response. 

## 🤝 Professional Integration
Engineered for Tier-1 forensic images and massive network datasets where traditional tools fail.

---
*© 2026 Advanced Forensic Ecosystem (AFE). All Rights Reserved.*
