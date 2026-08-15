# Async-first with `primp` for TLS browser impersonation

We build the HTTP core async-first on `tokio` + `primp`, and expose a blocking facade for callers who don't want to write async. `primp` is a pure-Rust client (patched `rustls` + `h2`) that spoofs a Chrome TLS/HTTP2 fingerprint via `Impersonate::ChromeV146`, replicating what Python `yfinance` does with `curl_cffi`.

**Considered Options**
- `primp` (chosen): pure Rust, actively maintained, async/tokio, no C toolchain, no `openssl-sys` link conflict. Fingerprint is patched-rustls rather than byte-identical to BoringSSL, but still defeats Yahoo rate-limiting.
- `wreq` (BoringSSL, the maintained `reqwest-impersonate` successor): byte-closest to `curl_cffi`, but carries rename/yank history, requires a BoringSSL C build, and **conflicts with `openssl-sys` at link time** — a real footgun for a data crate that pulls OpenSSL-using deps.
- `impersonate-rs` (blocking FFI over `curl-impersonate`): spiritually closest to `curl_cffi`, but synchronous-only, very early (0.1.3), and needs system `libcurl-impersonate`.
- Plain `reqwest` (no impersonation): simplest, but Yahoo throttles/blocks non-impersonated clients — exactly why `yfinance` moved to `curl_cffi`.

**Consequences**
- The crate requires Rust ≥ 1.89 (primp's floor); current toolchain is 1.97.1.
- A `wreq` backend remains a viable fallback if Yahoo ever starts fingerprint-checking at the BoringSSL byte level.
