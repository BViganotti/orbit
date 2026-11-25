# Security

Orbit is a local developer tool. It is not a multi-user service and should not
be exposed to a network.

## Controls

- Binds only to `127.0.0.1`.
- Generates a 128-bit random token from `/dev/urandom` at launch (fallback:
  time/pid-seeded xorshift, documented as weaker).
- Requires the token on every `/api/*` request, in the query string or
  `X-Orbit-Token` header.
- Uses a constant-time-style byte comparison for tokens.
- Limits request headers to 32 KiB, bodies to 2 MiB, query strings to 4 KiB,
  query parameters to 64 and headers to 64.
- Rejects `Transfer-Encoding` and duplicate `Content-Length`.
- Sends `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`,
  `Referrer-Policy: no-referrer`, a restrictive `Permissions-Policy` and a CSP
  that allows only same-origin scripts and styles.
- Serves only embedded assets; there is no filesystem browsing endpoint.
- Limits retained runs (16), SSE subscribers (32), stored events (2,000) and
  concurrent connections (64).

## Non-goals

- TLS. Loopback traffic is not encrypted.
- Authentication for static assets. They contain no secrets.
- Sandboxing the simulator. Rocket inputs are validated and bounded, but this is
  not a hardened multi-tenant execution environment.
- Defence against a local attacker who can read process memory or the printed
  URL.

## Reporting

This is a self-contained educational repository with no production deployment.
If you find a panic, unbounded allocation or token bypass, add a minimal case to
`fuzz/corpus` and document it in an issue or pull request.
