# 🔒 Security Policy — Advanced Forensic Ecosystem (AFE)

## Supported Versions

| Component | Version | Security Fixes |
|:---|:---|:---:|
| Zen Engine SDK (Rust core) | 0.1.x | ✅ Active |
| Big Data Explorer (VS Code) | 1.2.x | ✅ Active |
| Zen-IOC Desktop (Tauri) | 0.1.x | ✅ Active |

Older versions receive no security updates. Please always use the latest release.

---

## 🚨 Reporting a Vulnerability

**DO NOT open a public GitHub Issue for security vulnerabilities.**
Public disclosure before a fix is available puts all users at risk.

### Preferred Channel

Report vulnerabilities **privately** via GitHub's built-in Security Advisory system:

1. Go to the [Security tab](https://github.com/ejgi/afe-project/security) of the repository.
2. Click **"Report a vulnerability"**.
3. Fill in the details described below.

GitHub will keep your report confidential and only visible to the Maintainer.

### What to Include in Your Report

Please provide as much detail as possible:

- **Component affected**: SDK core, VS Code extension, Tauri app, or portal.
- **Vulnerability type**: e.g., memory corruption, unsafe FFI usage, path traversal,
  privilege escalation, data leakage, etc.
- **Severity assessment**: Critical / High / Medium / Low (CVSS score if possible).
- **Reproduction steps**: Exact steps, commands, or code to reproduce the issue.
- **Impact**: What an attacker could achieve by exploiting this vulnerability.
- **Suggested fix** (optional): If you have a proposed solution, include it.

### Response Timeline

| Action | Target SLA |
|:---|:---|
| Initial acknowledgment | ≤ 48 hours |
| Severity assessment | ≤ 5 business days |
| Patch development | ≤ 30 days (critical: ≤ 7 days) |
| Public disclosure | After patch is released |

The Maintainer will credit the reporter in the security advisory unless you
prefer to remain anonymous.

---

## ⚠️ Scope — What Is Considered a Vulnerability

### In Scope ✅

- Memory safety issues in Rust `unsafe` blocks (e.g., in `src/ffi.rs`, SIMD kernels).
- Path traversal in file discovery (`src/dataset/discovery.rs`).
- Privilege escalation via `io_uring` or PCAP raw socket handling.
- Hash collision attacks against the BLAKE3 tamper-detection system.
- Credential or API key exposure (e.g., telemetry backend credentials).
- Malicious `.zendx` file or PCAP/EVTX payload that causes code execution.
- GPU compute shader attacks via WGSL injection.
- FFI boundary issues exposing undefined behavior to calling code.

### Out of Scope ❌

- Performance issues that do not have a security impact.
- Vulnerabilities in third-party dependencies (report those upstream to the respective
  crate maintainer and we will update our dependency).
- Issues that require physical access to the machine being analyzed.
- Use of the tool on systems you do not own or are not authorized to analyze.

---

## 🧰 Security Architecture Notes

For researchers, these are the highest-risk areas of the codebase:

| Area | File | Risk Reason |
|:---|:---|:---|
| FFI API | [`src/ffi.rs`](./sdk/src/ffi.rs) | Raw C-compatible pointers crossing language boundaries |
| SIMD Kernels | [`src/analytics/simd.rs`](./sdk/src/analytics/simd.rs) | `unsafe` AVX2 intrinsics; alignment assumptions |
| PCAP Parser | [`src/parsers/forensics/pcap.rs`](./sdk/src/parsers/forensics/pcap.rs) | Processes untrusted network data; size assumptions |
| EVTX Scanner | [`src/parsers/forensics/evtx.rs`](./sdk/src/parsers/forensics/evtx.rs) | Binary carving of potentially malformed Windows logs |
| DMA / io_uring | [`src/compute/dma/`](./sdk/src/compute/dma/) | Low-level kernel interface; improper use can cause DoS |
| Delta Manager | [`src/dataset/delta.rs`](./sdk/src/dataset/delta.rs) | In-memory mutation map applied over mmap'd files |

---

## 🏅 Recognition

We appreciate the security research community's help in keeping AFE safe.
Confirmed vulnerability reporters will be listed in the project's
[Security Advisories](https://github.com/ejgi/afe-project/security/advisories)
with their permission.

---

*© 2026 Advanced Forensic Ecosystem (AFE) — ejgi. All rights reserved.*
