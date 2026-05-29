# ⚠️ DISCLAIMER — Advanced Forensic Ecosystem (AFE) / Zen Engine SDK

## Intended Use

The **Advanced Forensic Ecosystem (AFE)** and its **Zen Engine SDK** are designed
exclusively for:

- Authorized digital forensic investigations
- Incident Response (IR) operations on systems you own or are explicitly authorized to analyze
- Academic and security research in controlled environments
- Enterprise log analysis and big-data analytics on owned infrastructure

---

## Hardware & System Risk Warning

This software uses **advanced low-level hardware interfaces** that may cause
**irreversible data loss or hardware instability** if used incorrectly:

| Technology | Risk |
|:---|:---|
| **`mmap2` (memory-mapped I/O)** | Direct memory access; writing to a mapped region modifies the underlying file. Data corruption is possible on crash. |
| **`io_uring` (async I/O bypass)** | Bypasses standard OS I/O queues. Incorrect configuration can saturate NVMe controllers or cause kernel panics. |
| **Hugepages / Pinned Memory** | Allocates non-swappable physical RAM. May cause system instability on memory-constrained machines. |
| **GPU Compute (wgpu/Vulkan/DX12)** | Submits compute workloads directly to the GPU. Sustained compute loads may trigger thermal throttling or hardware protection circuits. |
| **PCAP / Raw Packet Analysis** | Requires elevated privileges (`CAP_NET_RAW`). Misuse may violate network monitoring laws. |

> **THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
> CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
> OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE
> OR OTHER DEALINGS IN THE SOFTWARE.**

---

## Legal Responsibility

By using, downloading, compiling, or deploying any part of this software, **you
explicitly agree** that:

1. **You are solely responsible** for ensuring you have legal authorization to analyze
   any system, file, network capture, or dataset you process with this tool.

2. **The authors and contributors are NOT liable** for:
   - Data loss caused by `mmap`, `io_uring`, DMA operations, or GPU compute workloads.
   - Hardware damage resulting from sustained high-performance workloads.
   - Legal consequences of unauthorized forensic analysis or network monitoring.
   - Any direct, indirect, incidental, special, or consequential damages.

3. **Misuse of forensic capabilities** (e.g., unauthorized analysis of third-party
   systems, unauthorized network packet capture) may violate local, national, or
   international law (CFAA, GDPR, Computer Misuse Act, etc.). The authors do not
   condone or support such use.

---

## Export & Regulatory Notice

This software may be subject to export control regulations in your jurisdiction.
The authors make no representations regarding compliance with export laws.
You are responsible for verifying compliance before distributing or deploying
this software outside your jurisdiction.

---

## No Warranty

This software is provided for professional use by qualified forensic investigators
and researchers. **It is NOT intended for use on production systems without
prior testing in an isolated environment.**

The authors strongly recommend:
- Testing all features on a dedicated non-production machine first.
- Running `io_uring` features only on Linux kernel ≥ 5.10.
- Ensuring adequate cooling before running GPU compute workloads.
- Keeping full backups before running any DMA or mmap-intensive operations.

---

*© 2026 Advanced Forensic Ecosystem (AFE) — ejgi. All rights reserved.*
*This disclaimer is incorporated by reference into the Apache 2.0 LICENSE.*
