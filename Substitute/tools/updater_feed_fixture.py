# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Serve explicit signed QA assets on literal loopback, with a controllable package transfer.

The application still authenticates every response. This fixture supplies no trust
override. A compiled local-qualification distribution selects its reserved port.
"""
import argparse
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
from pathlib import Path
import re
import time


def interval(value, size):
    """Accept one explicit byte interval, including the updater's resumable open end."""
    if value is None:
        return 0, size - 1, 200
    match = re.fullmatch(r'bytes=(0|[1-9][0-9]*)-(0|[1-9][0-9]*)?', value)
    if not match:
        raise ValueError('Expected one canonical byte range')
    start = int(match[1])
    end = size - 1 if match[2] is None else min(int(match[2]), size - 1)
    if start >= size or end < start:
        raise ValueError('Range outside asset')
    return start, end, 206


def asset(routes, request):
    """Resolve only exact manifest names; request paths never become filesystem paths."""
    if not re.fullmatch(r'/[a-zA-Z0-9_.-]+', request):
        return None
    value = routes.get(request[1:])
    if value is None:
        return None
    path = Path(value)
    if not path.is_absolute():
        raise ValueError('Fixture routes require absolute file paths')
    return path if path.is_file() else None


class Feed(HTTPServer):
    """One owned process serves a campaign without background threads or a service.

    The campaign's owner ends it by creating ``stop`` in the control folder; elapsed time never
    stops the feed.
    """
    def __init__(self, routes, control, port=0):
        if type(port) is not int or not 0 <= port <= 65535:
            raise ValueError('Fixture port is outside the native TCP range')
        self.routes = routes
        self.control = control
        super().__init__(('127.0.0.1', port), Handler)
        self.timeout = 0.25

    def record(self, **event):
        with (self.control / 'requests.jsonl').open('a', encoding='utf-8') as output:
            output.write(json.dumps({'time': time.time(), **event}) + '\n')

    def stopping(self):
        return (self.control / 'stop').exists()


class Handler(BaseHTTPRequestHandler):
    """Stream exact immutable bytes; package gating preserves client idle-progress semantics."""
    def setup(self):
        self.request.settimeout(5)
        super().setup()

    def log_message(self, *_):
        pass

    def do_GET(self):
        try:
            routes = json.loads(self.server.routes.read_text(encoding='utf-8'))
            path = asset(routes, self.path)
            if path is None:
                self.send_error(404)
                self.server.record(path=self.path, status=404)
                return
            with path.open('rb') as source:
                size = path.stat().st_size
                try:
                    start, end, status = interval(self.headers.get('Range'), size)
                except ValueError:
                    self.send_error(416)
                    return
                self.send_response(status)
                self.send_header('Content-Length', str(end - start + 1))
                self.send_header('Content-Type', 'application/octet-stream')
                if status == 206:
                    self.send_header('Content-Range', f'bytes {start}-{end}/{size}')
                self.end_headers()
                self.server.record(path=self.path, status=status, start=start, end=end)
                source.seek(start)
                remaining = end - start + 1
                if self.path.endswith('.ssupdate') and (self.server.control / 'hold').exists():
                    (self.server.control / 'download-started.json').write_text(
                        json.dumps({'path': self.path, 'start': start, 'size': size}), encoding='utf-8')
                    heartbeat = 0.0
                    while not (self.server.control / 'release').exists():
                        if self.server.stopping():
                            raise TimeoutError('Fixture stopped with a held transfer')
                        now = time.monotonic()
                        if now >= heartbeat:
                            if remaining <= 1:
                                raise ValueError('Held transfer requires a larger fixture asset')
                            self.wfile.write(source.read(1))
                            self.wfile.flush()
                            remaining -= 1
                            heartbeat = now + 5
                        time.sleep(0.05)
                while remaining:
                    if self.server.stopping():
                        raise TimeoutError('Fixture campaign ended during transfer')
                    chunk = source.read(min(64 * 1024, remaining))
                    if not chunk:
                        raise EOFError('Fixture asset changed during transfer')
                    self.wfile.write(chunk)
                    remaining -= len(chunk)
                self.wfile.flush()
                self.server.record(path=self.path, completed=True)
        except (OSError, ValueError, EOFError) as cause:
            self.server.record(path=self.path, failure=str(cause))
            self.close_connection = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--routes', required=True, type=Path)
    parser.add_argument('--control', required=True, type=Path)
    parser.add_argument('--port', type=int, default=0,
        help='Reuse a qualification build’s literal loopback port; fails if already occupied')
    args = parser.parse_args()
    if (not args.routes.is_absolute() or not args.routes.is_file()
            or not args.control.is_absolute() or args.control.exists()):
        raise ValueError('Use an existing absolute routes manifest and a new absolute control folder')
    args.control.mkdir()
    with Feed(args.routes, args.control, args.port) as server:
        (args.control / 'endpoint.json').write_text(json.dumps({
            'host': '127.0.0.1', 'port': server.server_port}), encoding='utf-8')
        while not server.stopping():
            server.handle_request()


if __name__ == '__main__':
    main()
