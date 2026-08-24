#!/usr/bin/env python3
"""Serve the landing page on localhost without booting the API.

`cargo run` needs Postgres and an RPC endpoint before it binds, which is a lot
of ceremony for looking at a static page. This reads the LANDING_HTML literal
straight out of the Rust source on every request -- so the preview cannot drift
from what the handler returns, and editing index.rs then hitting refresh shows
the change immediately.

    python3 scripts/preview-landing.py [port]
"""

import http.server
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "src" / "api" / "handlers" / "index.rs"
ASSETS = ROOT / "assets"
LOCKUP = ASSETS / "lockup.svg"

# Matches the `concat!(r##"..."##, include_str!(...), r##"..."##)` form the
# handler uses. Group 1 and 2 are the two literal chunks.
LITERAL = re.compile(
    r'static LANDING_HTML: &str = concat!\(\s*r##"(.*?)"##,\s*'
    r'include_str!\([^)]*\),\s*r##"(.*?)"##\s*\);',
    re.S,
)

CONTENT_TYPES = {".woff2": "font/woff2", ".svg": "image/svg+xml"}


def render() -> bytes:
    """Rebuild the page exactly as the Rust handler would."""
    match = LITERAL.search(SOURCE.read_text())
    if match is None:
        raise SystemExit(
            f"Could not find the LANDING_HTML literal in {SOURCE}.\n"
            "The handler's shape changed -- update LITERAL in this script."
        )
    head, tail = match.groups()
    return (head + LOCKUP.read_text() + tail).encode()


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self) -> None:  # noqa: N802 - stdlib naming
        path = self.path.split("?", 1)[0]
        if path == "/":
            self.send(200, "text/html; charset=utf-8", render(), cache="no-store")
        elif path.startswith("/assets/"):
            self.send_asset(path[len("/assets/") :])
        else:
            self.send(404, "text/plain", b"not found")

    def send_asset(self, name: str) -> None:
        target = ASSETS / name
        # Resolve and confirm the result is still inside assets/ before reading.
        if "/" in name or not target.is_file():
            self.send(404, "text/plain", b"not found")
            return
        content_type = CONTENT_TYPES.get(target.suffix, "application/octet-stream")
        self.send(200, content_type, target.read_bytes())

    def send(self, status: int, content_type: str, body: bytes, cache: str = "") -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        if cache:
            self.send_header("Cache-Control", cache)
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, fmt: str, *args: object) -> None:
        sys.stderr.write(f"  {fmt % args}\n")


def main() -> None:
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8787
    render()  # Fail fast and loudly rather than serving a blank page.
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
    print(f"Landing page preview: http://127.0.0.1:{port}")
    print(f"Reading {SOURCE.relative_to(ROOT)} on every request. Ctrl-C to stop.")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nstopped")


if __name__ == "__main__":
    main()
