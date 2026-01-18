# juno-keys

Offline key utility for Juno Cash:

- generate a cryptographically secure ZIP32 seed (base64)
- derive a UFVK (unified full viewing key) from a seed for use with `juno-scan`

## Security notes

- Seeds are **spending keys**. Keep them offline and out of logs.
- UFVKs are **watch-only** but still sensitive (they reveal incoming transactions/values). Avoid logging or sharing them.
- `juno-scan` only needs UFVKs. It must **never** receive seeds.

## API stability

- For automation/integrations, treat `--json` output as the stable API surface. Human-oriented output may change.
- JSON outputs are versioned via `version` (currently `"v1"`).

## Usage

Generate a new 64-byte seed and write it to a file (recommended):

- `juno-keys seed new --out ./hot.seed`

Print the seed to stdout (not recommended; avoid logs):

- `juno-keys seed new --json`

Derive a UFVK from that seed (account 0) for a given network:

- `juno-keys ufvk from-seed --seed-file ./hot.seed --network mainnet`
- `juno-keys ufvk from-seed --seed-file ./hot.seed --network testnet`
- `juno-keys ufvk from-seed --seed-file ./hot.seed --network regtest`

Register the UFVK with `juno-scan`:

```sh
curl -sS -X POST http://127.0.0.1:8080/v1/wallets \
  -H 'content-type: application/json' \
  -d '{"wallet_id":"exchange-hot-001","ufvk":"<jview...>"}'
```

## JSON output

All JSON responses include:

- `version`: response schema version (string, currently `"v1"`)
- `status`: `"ok"` or `"err"`

Seed generation (`seed new --json`):

```json
{ "version": "v1", "status": "ok", "data": { "bytes": 64, "out_path": "./hot.seed" } }
```

Notes:

- When `--out` is set, the seed is written to disk and `seed_base64` is omitted unless `--print` is set.

UFVK derivation (`ufvk from-seed --json`):

```json
{ "version": "v1", "status": "ok", "data": { "ufvk": "jview1...", "ua_hrp": "j", "coin_type": 8133, "account": 0 } }
```

Errors:

```json
{ "version": "v1", "status": "err", "error": { "code": "seed_invalid", "message": "..." } }
```

## Build & test

- Build: `make build` (outputs `bin/juno-keys`)
- Test: `make test`
