# 🤝 Contributing to AFE / Zen Engine SDK

Thank you for considering a contribution to the **Advanced Forensic Ecosystem (AFE)**!
We welcome bug reports, documentation improvements, new format parsers, and performance
optimizations from the community.

---

## ⚠️ Before You Start — CLA Required

All contributors **must sign the Contributor License Agreement (CLA)** before any
Pull Request can be merged. This is enforced automatically by CLA Assistant on GitHub.

👉 **Read the CLA here:** [`CLA.md`](./CLA.md)

When you open a Pull Request, a bot will ask you to accept the CLA in the PR comments.
Simply reply as instructed — it takes less than 30 seconds.

---

## 🛡️ Code of Conduct

By participating in this project, you agree to:

- Be respectful and professional in all interactions.
- **Never submit code designed to harm users, bypass security controls, or enable
  unauthorized access to systems.**
- Respect that the Maintainer has final say on all design and merge decisions.

---

## 🚀 How to Contribute

### 1. Bug Reports

Before opening an issue:
- Check if the issue already exists in [GitHub Issues](https://github.com/ejgi/afe-project/issues).
- Include: OS, Rust version (`rustc --version`), reproduction steps, and the full error output.
- **For security vulnerabilities, do NOT open a public issue.** See [`SECURITY.md`](./SECURITY.md).

### 2. Feature Requests

Open a GitHub Issue with the label `enhancement`. Describe:
- The use case (what forensic/analytical problem it solves).
- How it fits the "Extreme Performance First" philosophy.
- Whether you are willing to implement it yourself.

### 3. Pull Requests

1. **Fork** the repository and create a branch from `main`:
   ```bash
   git checkout -b feat/my-new-parser
   ```

2. **Sign the CLA** (the bot will prompt you on the PR).

3. **Follow the coding standards:**
   - Write idiomatic Rust (run `cargo clippy` with zero warnings).
   - Add new parsers in `src/parsers/<domain>/` and register them in the Dispatcher (`parsers/mod.rs`).
   - New aggregators go in `src/accumulator.rs` or `src/analytics/`.
   - All public functions must have doc comments (`///`).
   - Do not introduce `unsafe` blocks without a detailed safety comment.

4. **Test your changes:**
   ```bash
   cargo test
   cargo clippy -- -D warnings
   cargo build --release
   ```
   For performance-sensitive code, include benchmark results using one of the existing
   bin targets in `src/bin/`.

5. **Keep commits clean** — one logical change per commit, with a clear message:
   ```
   feat(parsers): add JSON-Lines streaming parser
   fix(simd): correct AVX2 alignment for non-multiple-of-4 inputs
   docs(api): clarify VirtualDataset parallelism guarantees
   ```

6. Open your PR targeting the `main` branch and fill in the PR template.

---

## 📂 Architecture Quick Reference

Adding a new file format? Follow these 3 steps:

1. Create `src/parsers/<domain>/my_format.rs` and implement the `FormatParser` trait.
2. Register it in the format Dispatcher at `src/parsers/mod.rs`.
3. Add a test binary in `src/bin/` and a sample test file.

See [`docs/analisis_completo_proyecto.md`](./analisis_completo_proyecto.md.resolved)
for a full architecture breakdown.

---

## 🚫 What We Do Not Accept

- Parsers or features that enable unauthorized access to third-party systems.
- Code that disables the forensic integrity checks (BLAKE3 tamper detection).
- Dependencies with GPL licenses (they are incompatible with our Apache 2.0 + CLA model).
- Contributions without a signed CLA.
- Breaking changes to the FFI (`src/ffi.rs`) without prior discussion in an Issue.

---

## 📬 Contact

For questions about contributing, reach out via GitHub Discussions or the
contact listed in [`SECURITY.md`](./SECURITY.md).

---

*© 2026 Advanced Forensic Ecosystem (AFE) — ejgi.*
