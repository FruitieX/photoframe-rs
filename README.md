## photoframe-rs

Rust-based photo frame orchestration server for networked (e.g. Spectra E6 / multi‑color e‑ink) displays that periodically (or on demand) selects photos from configured sources, processes them (rotate → scale → pad → adjust → dither / palette reduce → encode), and pushes the result to networked e‑ink / multi‑color frames over HTTP.

---
### Quick start (server)
```bash
cp photoframe.example.toml photoframe.toml   # edit to match your devices & sources
cargo run -p photoframe-server               # binds 0.0.0.0:8080 by default
```

### Quick start (web UI)
```bash
npm install --prefix photoframe-nextjs
npm run dev --prefix photoframe-nextjs
```

### Supported dithering algorithms
Type | Identifiers
-----|------------
Diffusion | `floyd_steinberg`, `jarvis_judice_ninke`, `stucki`, `burkes`, `sierra_3`, `sierra_2`, `sierra_1`, `atkinson`, `reduced_atkinson`
Ordered / Other | `ordered_bayer_2`, `ordered_bayer_4`, `ordered_bayer_8`, `ordered_blue_256`, `stark`, `yliluoma1`, `yliluoma2`

### Sources
Kind | Config Block | Notes
-----|--------------|------
Filesystem | `[sources.<id>.filesystem]` | Glob expanded once at startup; orientation via dimensions.
Immich | `[sources.<id>.immich]` | Needs `base_url`, `api_key`; optional `filters` for [https://immich.app/docs/api/search-assets](searchAssets) request body parameters.

Immich snippet:
```toml
[sources.family]
kind = "immich"
[sources.family.immich]
base_url = "http://immich.local:2283"
api_key = "YOUR_KEY"
order = "random"
filters = { personIds = ["uuid1", "uuid2"] } # Finds photos containing both persons "uuid1" AND "uuid2"
```

Hint: You can configure multiple immich sources if you want different sets of filters (for example photos containing persons "uuid1" OR "uuid2" must be done with two separate immich sources)
