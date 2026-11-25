# HTTP API

The Studio binds `127.0.0.1` on an OS-assigned port (or `--port N`) and prints
`ORBIT_URL=http://127.0.0.1:PORT/?token=…`. All API routes require the token:

- query parameter `?token=…`, or
- header `X-Orbit-Token: …`.

Static assets (`/`, `/app.css`, `/app.js`, `/favicon.ico`) are public. No route
accepts a non-loopback connection because the listener is bound to
`127.0.0.1`.

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| GET | `/api/health` | version and model identity |
| GET | `/api/presets` | six built-in rockets with summaries |
| GET | `/api/scripts` | twenty mission catalogue entries |
| POST | `/api/validate` | parse a rocket and run engineering checks |
| POST | `/api/mission/run` | start a mission, returns `{run_id}` |
| POST | `/api/demo` | run synchronously and return the result |
| GET | `/api/runs/:id` | status, optional result, failure message |
| GET | `/api/export/csv?run=:id` | trajectory CSV |
| GET | `/api/export/report?run=:id` | text mission report |
| GET | `/api/events?run=:id` | SSE stream for a run |
| GET | `/api/conservation` | 30-day coupled n-body conservation check |

## Examples

```sh
TOKEN=...; PORT=...
curl "http://127.0.0.1:$PORT/api/health?token=$TOKEN"

curl -X POST "http://127.0.0.1:$PORT/api/validate?token=$TOKEN" \
  -H 'Content-Type: application/json' \
  --data @examples/rockets/atlas.json

curl -X POST "http://127.0.0.1:$PORT/api/mission/run?token=$TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"script_id":"lunar-atlas-100","rocket_id":"atlas"}'

curl "http://127.0.0.1:$PORT/api/runs/1?token=$TOKEN"
curl -o trajectory.csv "http://127.0.0.1:$PORT/api/export/csv?run=1&token=$TOKEN"
```

## Validation response

```json
{
  "ok": true,
  "report": {
    "ok": true,
    "errors": 0,
    "warnings": 1,
    "issues": [
      { "severity": "info", "code": "nominal", "message": "…" }
    ],
    "summary": {
      "liftoff_mass": 788500,
      "total_delta_v": 15448.5,
      "liftoff_twr": 1.778
    }
  },
  "rocket": { "…": "normalized rocket" }
}
```

Invalid JSON or an invalid rocket returns HTTP 400 with
`{"error":"…"}`. Invalid input never reaches the simulator.

## SSE

Each mission run is a bounded record with at most 2,000 stored events and 32
subscribers.

```
event: progress
data: {"message":"validating vehicle and mission"}

event: mission
data: {"t":4663,"phase":"transfer","kind":"TliIgnition","detail":"TLI burn 3012 m/s"}

event: done
data: {"run_id":1,"success":true,"status":"success","message":"lunar orbit achieved"}
```

`GET /api/events` replays stored events, then streams live events and sends a
`: ping` comment every 12 seconds. The stream closes after `done` or `error`.

## Bounds

- request headers: 32 KiB, 64 headers
- request body: 2 MiB
- URL: 4,096 bytes; query string 4,096 bytes
- retained runs: 16
- SSE events per run: 2,000
- SSE subscribers per run: 32
- response body is generated from bounded simulation state

FastAPI-style OpenAPI is not provided; this document and the strict JSON parser
are the contract.
