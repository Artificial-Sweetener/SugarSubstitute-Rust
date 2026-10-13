# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Extract exact reviewed native owner bodies for isolated boundary tests."""
import argparse
from pathlib import Path


def between(source, start, end):
    """Reject missing/nonunique owner boundaries before emitting any source."""
    if source.count(start) != 1 or source.count(end) != 1:
        raise ValueError("Native test owner boundary is missing or nonunique")
    first, last = source.index(start), source.index(end)
    if first >= last:
        raise ValueError("Native test owner boundaries are out of order")
    return source[first:last]


def extract(client, helper):
    """Return the actual generated Common/helper/installer, never a model copy."""
    client = client.decode("utf-8")
    helper = helper.decode("utf-8")
    common = between(client, "bool CommonInProcessInitialization(",
                     "\n}  // namespace\n\nCrashpadClient::CrashpadClient()")
    methods = between(client, "bool CrashpadClient::InitializeCapturedAttach(",
                      "bool CrashpadClient::SetHandlerIPCPipe(")
    native = between(helper, "namespace {", "}  // namespace crashpad")
    return {"capture_runtime_common.inc": common.encode("utf-8"),
            "capture_runtime_methods.inc": methods.encode("utf-8"),
            "capture_runtime_helper.inc": native.encode("utf-8")}


def generate(client, helper, output):
    """Write derived test source only to the reviewed D: artifact tree."""
    if output.resolve().drive.upper() != "D:":
        raise ValueError("Native generated test source must remain on D:")
    parts = extract(client.read_bytes(), helper.read_bytes())
    output.mkdir(parents=True, exist_ok=True)
    for name, raw in parts.items():
        (output / name).write_bytes(raw)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--client", required=True, type=Path)
    parser.add_argument("--helper", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()
    generate(arguments.client, arguments.helper, arguments.output)
