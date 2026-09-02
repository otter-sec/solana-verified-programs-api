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
import stat
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "src" / "api" / "handlers" / "index.rs"
ASSETS = ROOT / "assets" / "landing"
LOCKUP = ASSETS / "lockup.svg"

# Matches the `concat!(r##"..."##, include_str!(...), r##"..."##)` form the
# handler uses. Group 1 and 2 are the two literal chunks.
LITERAL = re.compile(
    r'static LANDING_HTML: &str = concat!\(\s*r##"(.*?)"##,\s*'
    r'include_str!\([^)]*\),\s*r##"(.*?)"##\s*\);',
    re.S,
)

# Mirror the content types the Rust asset handler sets. A wrong type here is
# not cosmetic: browsers refuse to execute a script served as octet-stream.
CONTENT_TYPES = {
    ".woff2": "font/woff2",
    ".svg": "image/svg+xml",
    ".js": "text/javascript; charset=utf-8",
}

# Production serves the checked-in bundle through a content-versioned URL so
# its immutable cache header stays honest. Keep the source filename convenient
# for the documented rebuild command while mirroring that public URL here.
ASSET_SOURCES = {"grain-6bef640a.js": "grain.js"}


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
        self.handle_request(write_body=True)

    def do_HEAD(self) -> None:  # noqa: N802 - stdlib naming
        self.handle_request(write_body=False)

    def handle_request(self, *, write_body: bool) -> None:
        path = self.path.split("?", 1)[0]
        if path == "/":
            self.send(
                200,
                "text/html; charset=utf-8",
                render(),
                cache="no-store",
                write_body=write_body,
            )
        elif path.startswith("/assets/"):
            self.send_asset(path.removeprefix("/assets/"), write_body=write_body)
        else:
            self.send(404, "text/plain", b"not found", write_body=write_body)

    def send_asset(self, name: str, *, write_body: bool) -> None:
        target = ASSETS / ASSET_SOURCES.get(name, name)
        if "/" in name:
            self.send(404, "text/plain", b"not found", write_body=write_body)
            return

        try:
            if write_body:
                body = target.read_bytes()
                content_length = len(body)
            else:
                metadata = target.stat()
                if not stat.S_ISREG(metadata.st_mode):
                    raise IsADirectoryError(target)
                body = b""
                content_length = metadata.st_size
        except (FileNotFoundError, IsADirectoryError):
            self.send(404, "text/plain", b"not found", write_body=write_body)
            return

        content_type = CONTENT_TYPES.get(target.suffix, "application/octet-stream")
        self.send(
            200,
            content_type,
            body,
            content_length=content_length,
            write_body=write_body,
        )

    def send(
        self,
        status: int,
        content_type: str,
        body: bytes,
        cache: str = "",
        *,
        content_length: int | None = None,
        write_body: bool = True,
    ) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header(
            "Content-Length",
            str(len(body) if content_length is None else content_length),
        )
        if cache:
            self.send_header("Cache-Control", cache)
        self.end_headers()
        if write_body:
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
