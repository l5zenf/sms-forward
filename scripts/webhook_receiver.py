#!/usr/bin/env python3
"""gg-guard webhook receiver.

Pinned via env:
    GG_WEBHOOK_PORT=8080
    GG_WEBHOOK_TOKEN=<optional bearer>

Send/poll like this:
    uv run webhook_receiver.py        # if uv installed
    python3 webhook_receiver.py       # stdlib only

Same machine as gg-guard:
    GG_GUARD_WEBHOOK_URL=http://127.0.0.1:8080/sms
"""
from __future__ import annotations

import json
import os
import sys
from collections import Counter
from datetime import datetime, timezone
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse

PORT = int(os.environ.get("GG_WEBHOOK_PORT", "8080"))
TOKEN = os.environ.get("GG_WEBHOOK_TOKEN", "")
STORE = os.environ.get("GG_WEBHOOK_STORE", "received.jsonl")

_seen_keys: set[str] = set()
_stats = Counter()


def _log(msg: str) -> None:
    print(f"[{datetime.now().isoformat(timespec='seconds')}] {msg}", flush=True)


def _accept(handler: BaseHTTPRequestHandler) -> bool:
    if not TOKEN:
        return True
    auth = handler.headers.get("Authorization", "")
    return auth == f"Bearer {TOKEN}"


class Handler(BaseHTTPRequestHandler):
    server_version = "gg-guard-webhook/1.0"

    def _send(self, code: int, body: bytes = b"") -> None:
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if body:
            self.wfile.write(body)

    # ── GET: /  → heartbeat with running stats
    def do_GET(self) -> None:
        if urlparse(self.path).path != "/":
            return self._send(HTTPStatus.NOT_FOUND)
        payload = {
            "ok": True,
            "received_total": _stats["total"],
            "dedup_hits": _stats["dup"],
            "by_status": dict(_stats),
        }
        self._send(HTTPStatus.OK, json.dumps(payload).encode())

    # ── POST: /sms → record one forwarded SMS
    def do_POST(self) -> None:
        path = urlparse(self.path).path
        if path not in ("/sms", "/"):
            return self._send(HTTPStatus.NOT_FOUND)

        if not _accept(self):
            return self._send(HTTPStatus.UNAUTHORIZED)

        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length else b""

        try:
            payload = json.loads(raw.decode("utf-8"))
        except Exception as e:
            _log(f"BAD-JSON err={e} raw={raw!r}")
            return self._send(HTTPStatus.BAD_REQUEST)

        _stats["total"] += 1
        dedupe_key = payload.get("dedupe_key", "")
        if dedupe_key and dedupe_key in _seen_keys:
            _stats["dup"] += 1
            _log(f"DUP key={dedupe_key[:12]}…")
            return self._send(HTTPStatus.OK, b'{"ok":true,"dup":true}')

        if dedupe_key:
            _seen_keys.add(dedupe_key)

        # Persist raw line for replay/debugging.
        with open(STORE, "a", encoding="utf-8") as f:
            f.write(json.dumps(payload, ensure_ascii=False) + "\n")

        sender = payload.get("sender", "?")
        content = payload.get("content", "")
        sms_time = payload.get("sms_time", "")
        encoding = payload.get("encoding", "?")
        msg_id = payload.get("id")

        _log(
            f"SMS id={msg_id} sender={sender} enc={encoding} time={sms_time}\n"
            f"     content={content!r}"
        )

        self._send(
            HTTPStatus.OK,
            json.dumps({"ok": True, "id": msg_id}).encode("utf-8"),
        )

    def log_message(self, fmt: str, *args) -> None:  # silence default access log
        return


def main() -> None:
    _log(f"webhook receiver on :{PORT}  store={STORE}  token={'yes' if TOKEN else 'no'}")
    httpd = ThreadingHTTPServer(("0.0.0.0", PORT), Handler)
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        _log("shutting down")
        sys.exit(0)


if __name__ == "__main__":
    main()
