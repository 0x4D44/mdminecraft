#!/usr/bin/env python3
"""
mdminecraft automation harness client (NDJSON v1).

This is a small helper for driving a running `mdminecraft` automation server using an NDJSON
request script (stdin or file). It sends `hello` automatically and then runs requests
sequentially, printing all response events to stdout.
"""

from __future__ import annotations

import argparse
import json
import os
import socket
import sys
import time
from typing import IO, Any, Dict, Optional, Tuple


PROTOCOL_VERSION = 1


def parse_tcp_addr(raw: str) -> Tuple[str, int]:
    if ":" not in raw:
        raise ValueError("expected host:port")
    host, port_raw = raw.rsplit(":", 1)
    return host, int(port_raw)


def connect_tcp(addr: str, timeout_s: float) -> socket.socket:
    host, port = parse_tcp_addr(addr)
    deadline = time.time() + timeout_s
    last_err: Optional[Exception] = None
    while time.time() < deadline:
        try:
            sock = socket.create_connection((host, port), timeout=timeout_s)
            sock.settimeout(timeout_s)
            return sock
        except OSError as err:
            last_err = err
            time.sleep(0.05)
    raise RuntimeError(f"failed to connect to tcp {addr}: {last_err}")


def connect_uds(path: str, timeout_s: float) -> socket.socket:
    if not hasattr(socket, "AF_UNIX"):
        raise RuntimeError("unix domain sockets are not supported on this platform/python build")
    deadline = time.time() + timeout_s
    last_err: Optional[Exception] = None
    while time.time() < deadline:
        try:
            sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            sock.settimeout(timeout_s)
            sock.connect(path)
            return sock
        except OSError as err:
            last_err = err
            time.sleep(0.05)
    raise RuntimeError(f"failed to connect to uds {path}: {last_err}")


def expected_completion_event(op: str) -> str:
    if op in ("set_actions", "pulse", "set_view", "shutdown"):
        return "ok"
    if op == "command":
        return "command_result"
    if op == "get_state":
        return "state"
    if op == "step":
        return "stepped"
    if op == "screenshot":
        return "screenshot"
    if op == "hello":
        return "hello"
    return "error"


def write_line(writer: IO[bytes], value: Dict[str, Any]) -> None:
    data = (json.dumps(value, separators=(",", ":")) + "\n").encode("utf-8")
    writer.write(data)
    writer.flush()


def read_json_line(reader: IO[bytes]) -> Dict[str, Any]:
    line = reader.readline()
    if not line:
        raise EOFError("connection closed")
    decoded = line.decode("utf-8", errors="replace").strip()
    if not decoded:
        return read_json_line(reader)
    return json.loads(decoded)


def print_event(value: Dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(value, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def main() -> int:
    parser = argparse.ArgumentParser(description="mdminecraft automation harness client")
    connect_group = parser.add_mutually_exclusive_group(required=True)
    connect_group.add_argument("--tcp", help="connect to TCP automation server (host:port)")
    connect_group.add_argument("--uds", help="connect to unix domain socket automation server")
    parser.add_argument("--token", help="token to send in hello", default=None)
    parser.add_argument(
        "--timeout-seconds",
        type=float,
        default=10.0,
        help="connect/read timeout (seconds)",
    )
    parser.add_argument(
        "--script",
        type=str,
        default=None,
        help="path to NDJSON request script (default: stdin)",
    )
    parser.add_argument(
        "--auto-id",
        action="store_true",
        help="auto-assign integer ids when missing",
    )
    parser.add_argument(
        "--quiet-hello",
        action="store_true",
        help="do not print the hello response event",
    )
    args = parser.parse_args()

    if args.tcp:
        sock = connect_tcp(args.tcp, args.timeout_seconds)
    else:
        sock = connect_uds(args.uds, args.timeout_seconds)

    reader = sock.makefile("rb", buffering=0)
    writer = sock.makefile("wb", buffering=0)

    next_id = 1
    hello_req: Dict[str, Any] = {"op": "hello", "id": next_id, "version": PROTOCOL_VERSION}
    next_id += 1
    if args.token:
        hello_req["token"] = args.token
    write_line(writer, hello_req)
    hello_resp = read_json_line(reader)
    if not args.quiet_hello:
        print_event(hello_resp)
    if hello_resp.get("event") != "hello":
        return 2

    input_stream: IO[str]
    if args.script:
        input_stream = open(args.script, "r", encoding="utf-8")
    else:
        input_stream = sys.stdin

    try:
        for raw in input_stream:
            stripped = raw.strip()
            if not stripped or stripped.startswith("#"):
                continue

            req = json.loads(stripped)
            if not isinstance(req, dict):
                raise ValueError("request line must be a JSON object")

            op = req.get("op")
            if not isinstance(op, str) or not op:
                raise ValueError("missing/invalid 'op' field")

            if args.auto_id and "id" not in req:
                req["id"] = next_id
                next_id += 1

            write_line(writer, req)

            done_event = expected_completion_event(op)
            while True:
                resp = read_json_line(reader)
                print_event(resp)
                event = resp.get("event")
                if event == "error" or event == done_event:
                    break
    finally:
        if args.script:
            input_stream.close()
        try:
            writer.close()
            reader.close()
        finally:
            sock.close()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

