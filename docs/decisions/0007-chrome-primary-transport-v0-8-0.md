# ADR-0007: Chrome Headed as Primary Search Transport (v0.8.0)


## Status
- Accepted (2026-06-21). Note: wreq references in this ADR are historical; wreq was replaced by reqwest+rustls in v0.8.6 (ADR-0008)


## Context
- wreq HTTP client with BoringSSL TLS (JA3 fingerprint) was blocked by Cloudflare
- Chrome headless mode was detected by Cloudflare anti-bot (6 stealth deficiencies)
- Chrome headless=new was detected by rendering pipeline differences
- Canvas, WebGL, AudioContext fingerprints were deterministic in headless mode
- outerHeight=0 and empty PluginArray exposed headless Chrome


## Decision
- Chrome headed mode via `xvfb-run` is the PRIMARY search transport
- 17 JavaScript stealth signals are injected via CDP before page navigation
- `xvfb-run` provides a virtual X11 display on headless Linux servers
- wreq remains ONLY for `--fetch-content` and `--probe` HTTP requests
- Headless mode is FALLBACK when neither DISPLAY nor xvfb-run is available


## Stealth Signals (17)
- `navigator.webdriver` set to `false`
- Canvas fingerprint noise injection
- WebGL renderer and vendor spoofing
- AudioContext noise injection
- `navigator.plugins` populated with realistic entries
- `navigator.languages` matches identity pool
- `chrome` runtime object spoofed
- `navigator.connection` set to realistic values
- `navigator.maxTouchPoints` set to realistic values
- `window.outerHeight` and `window.outerWidth` set to realistic values
- `navigator.hardwareConcurrency` set to 8
- `navigator.deviceMemory` set to 8
- `Notification.permission` set to `default`
- `navigator.permissions` query spoofed
- `WebGLRenderingContext.getParameter` spoofed
- `HTMLCanvasElement.toDataURL` noise injection
- `OfflineAudioContext` noise injection


## Consequences
- Linux servers MUST have `xvfb-run` installed
- Chrome or Chromium MUST be installed on all platforms
- Binary size increases by ~20 MB (BoringSSL + chromiumoxide)
- Search latency increases by ~500ms (Chrome startup + navigation)
- Cloudflare anti-bot is bypassed on 2026-06-21 test environment


## Alternatives Considered
- Headless=new with more stealth: REJECTED (rendering pipeline differences detectable)
- Playwright/Puppeteer: REJECTED (Node.js dependency, not pure Rust)
- wreq with better TLS emulation: REJECTED (JA3 alone is insufficient)
- Rotating proxies: REJECTED (operational complexity, cost)
