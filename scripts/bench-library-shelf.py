#!/usr/bin/env python3
"""Measure the public synthetic shelf with a release backend or the Linux Tauri driver."""

import argparse
import contextlib
import ctypes
import http.client
import json
import mmap
import os
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

PROFILE = "com.atha.reader"
MARKER = ".atha-shelf-benchmark-v1"
BOOKS = 256
USER_DATA = ("Library", "ImportedBooks", "SourceBooks", "BookOrganization")
RETURN_SENTINEL = "Benchmark 255 refreshed"


def fixture_root(path):
    root = Path(path).resolve(strict=True)
    marker = root / MARKER
    if marker.is_symlink() or not re.fullmatch(r"[a-f0-9]{64}", marker.read_text()):
        raise ValueError("use the synthetic fixture created by shelf_benchmark seed")
    return root


def prepare_file_cache(profile, state):
    """Target only fixture file pages. Directory/inode caches and other processes are untouched."""
    libc = ctypes.CDLL(None, use_errno=True)
    libc.mincore.argtypes = (ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p)
    resident = total = 0
    for directory in USER_DATA:
        for path in (profile / directory).rglob("*"):
            if path.is_symlink() or not path.is_file():
                continue
            with path.open("rb") as file:
                size = os.fstat(file.fileno()).st_size
                if not size:
                    continue
                if state == "warm":
                    file.read()
                else:
                    os.fsync(file.fileno())
                    os.posix_fadvise(file.fileno(), 0, 0, os.POSIX_FADV_DONTNEED)
                pages = (size + mmap.PAGESIZE - 1) // mmap.PAGESIZE
                result = (ctypes.c_ubyte * pages)()
                # ACCESS_COPY allows taking the address without faulting in a page.
                with mmap.mmap(file.fileno(), size, access=mmap.ACCESS_COPY) as mapping:
                    probe = ctypes.c_char.from_buffer(mapping)
                    try:
                        if libc.mincore(ctypes.addressof(probe), size, result) != 0:
                            raise OSError(
                                ctypes.get_errno(), f"mincore failed for {path}"
                            )
                    finally:
                        del probe
                total += pages
                resident += sum(page & 1 for page in result)
    if (
        not total
        or (state == "warm" and resident != total)
        or (state == "evicted" and resident != 0)
    ):
        raise RuntimeError(
            f"unexpected {state} fixture file residency: {resident}/{total}"
        )
    return {"fixture_resident_pages": resident, "fixture_total_pages": total}


def output(sample):
    print(json.dumps(sample, ensure_ascii=False), flush=True)


def candidates(args, sample):
    pair = [("after", args.binary)]
    if args.baseline:
        pair.insert(0, ("before", args.baseline))
    return pair if sample % 2 == 0 else reversed(pair)


def backend(args, fixture):
    for i in range(args.samples):
        for cache in ("evicted", "warm"):
            for phase in ("startup", "return") if cache == "warm" else ("startup",):
                for candidate, binary in candidates(args, i):
                    residency = prepare_file_cache(fixture / PROFILE, cache)
                    result = subprocess.check_output(
                        [binary, phase, fixture], text=True
                    )
                    output(
                        {
                            "sample": i + 1,
                            "candidate": candidate,
                            "file_cache": cache,
                            **residency,
                            **json.loads(result),
                        }
                    )


@contextlib.contextmanager
def private_bus(root, log):
    config = root / "bus.conf"
    config.write_text(f"""
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir={root}</listen>
  <policy context="default">
    <allow send_destination="*"/>
    <allow receive_sender="*"/>
    <allow own="*"/>
  </policy>
</busconfig>
""")
    with subprocess.Popen(
        ["dbus-daemon", "--nofork", "--print-address", "--config-file", str(config)],
        stdout=subprocess.PIPE,
        stderr=log,
        text=True,
    ) as process:
        address = process.stdout.readline().strip()
        try:
            if not address:
                raise RuntimeError("isolated D-Bus did not start; see the driver log")
            yield address
        finally:
            if process.poll() is None:
                process.terminate()
            process.wait(timeout=5)


@contextlib.contextmanager
def webdriver(log, env):
    with socket.socket() as first, socket.socket() as second:
        first.bind(("127.0.0.1", 0))
        second.bind(("127.0.0.1", 0))
        port, native_port = first.getsockname()[1], second.getsockname()[1]
    driver = os.environ.get("ATHA_TAURI_DRIVER", "tauri-driver")
    with subprocess.Popen(
        [driver, "--port", str(port), "--native-port", str(native_port)],
        env=env,
        stdout=log,
        stderr=log,
        start_new_session=True,
    ) as process:
        # Do not route loopback WebDriver requests through a proxy.
        url = f"http://127.0.0.1:{port}"
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

        def request(method, path, data=None, timeout=30):
            body = None if data is None else json.dumps(data).encode()
            request = urllib.request.Request(
                url + path,
                data=body,
                method=method,
                headers={"Content-Type": "application/json"},
            )
            try:
                with opener.open(request, timeout=timeout) as response:
                    return json.load(response)["value"]
            except urllib.error.HTTPError as error:
                if path == "/status":
                    raise
                raise RuntimeError(
                    f"WebDriver {method} {path}: {error.read().decode()}"
                ) from error

        try:
            deadline = time.monotonic() + 10
            while True:
                if process.poll() is not None or time.monotonic() >= deadline:
                    raise RuntimeError(
                        "tauri-driver did not become ready; see the driver log"
                    )
                try:
                    request("GET", "/status", timeout=1)
                    break
                except (OSError, http.client.HTTPException):
                    time.sleep(0.02)
            yield request
        finally:
            # Own only this isolated driver process group and its children.
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()


SHELF_READY = """
const [id, sentinel, count] = arguments;
const card = document.querySelector('.shelf-catalog [data-shelf-open="' + id + '"]');
const shell = card?.closest("main.library-shell");
const rect = card?.getBoundingClientRect();
const hit = rect && document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2);
const usable = shell?.getAttribute("aria-busy") === "false" && !card.disabled
  && shell.querySelectorAll(".shelf-catalog [data-shelf-open]").length === count
  && document.visibilityState === "visible" && card.contains(hit)
  && (!sentinel || (document.activeElement === card
    && shell.querySelector('.shelf-catalog [data-shelf-open][title="' + sentinel + '"]')));
return usable ? {
  time_ms: performance.timeOrigin + performance.now(),
  clk_ms: Number(sessionStorage.getItem("shelf-bench-back")) || null,
  page_origin_ms: performance.timeOrigin
} : null;
"""

READER_READY = """
return document.documentElement.dataset.status === "pass"
  && document.documentElement.hasAttribute("data-reader-ready")
  && document.querySelector(".reader-shell")?.getAttribute("aria-busy") === "false"
  && document.querySelector(".reader-startup")?.getAttribute("aria-hidden") === "true";
"""


def native_sample(request, binary, root, fixture, cache):
    data = root / "data"
    shutil.copytree(fixture / PROFILE, data / PROFILE)
    (root / "home").mkdir()
    start = root / "launch_ns"
    launcher = root / "launch"
    # Stamp immediately before exec, excluding driver/session creation.
    isolated_environment = {
        key: str(root / name)
        for key, name in [
            ("HOME", "home"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_STATE_HOME", "state"),
        ]
    }
    launcher.write_text(f"""#!{sys.executable}
import os, sys, time
os.environ.update({isolated_environment!r})
with open({str(start)!r}, "w") as f: f.write(str(time.time_ns()))
os.execv({str(binary)!r}, [{str(binary)!r}, *sys.argv[1:]])
""")
    launcher.chmod(0o700)
    residency = prepare_file_cache(data / PROFILE, cache)
    session = request(
        "POST",
        "/session",
        {
            "capabilities": {
                "alwaysMatch": {"tauri:options": {"application": str(launcher)}}
            }
        },
    )
    path = f"/session/{session['sessionId']}"
    try:

        def execute(script, *arguments):
            return request(
                "POST", path + "/execute/sync", {"script": script, "args": arguments}
            )

        def wait(script, *arguments):
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline:
                state = execute(script, *arguments)
                if state:
                    return state
                time.sleep(0.01)
            raise RuntimeError(
                "native UI did not reach the benchmark readiness condition"
            )

        def click(selector):
            element = request(
                "POST", path + "/element", {"using": "css selector", "value": selector}
            )
            element_id = element["element-6066-11e4-a52e-4f735466cecf"]
            request("POST", f"{path}/element/{element_id}/click", {})

        book_id = (fixture / MARKER).read_text()
        ready = wait(SHELF_READY, book_id, None, BOOKS)
        launch_ms = int(start.read_text()) / 1_000_000
        click(f'.shelf-catalog [data-shelf-open="{book_id}"]')
        wait(READER_READY)
        # A BFCache restore can expose an enabled old shelf before its async list returns.
        # Change one separate staged fixture record to prove the observed shelf re-listed.
        for record in (data / PROFILE / "Library").glob("*.json"):
            stored = json.loads(record.read_text())
            if stored["title"] == "Benchmark 255":
                stored["title"] = RETURN_SENTINEL
                replacement = record.with_suffix(".bench")
                replacement.write_text(json.dumps(stored))
                replacement.replace(record)
                break
        else:
            raise RuntimeError(
                "staged return sentinel is missing from the synthetic shelf"
            )
        x, y = execute("""
          const rect = document.querySelector(".reader").getBoundingClientRect();
          return [Math.round((rect.left + rect.right) / 2), Math.round((rect.top + rect.bottom) / 2)];
        """)
        request(
            "POST",
            path + "/actions",
            {
                "actions": [
                    {
                        "type": "pointer",
                        "id": "mouse",
                        "parameters": {"pointerType": "mouse"},
                        "actions": [
                            {
                                "type": "pointerMove",
                                "duration": 0,
                                "origin": "viewport",
                                "x": x,
                                "y": y,
                            },
                            {"type": "pointerDown", "button": 0},
                            {"type": "pointerUp", "button": 0},
                        ],
                    }
                ]
            },
        )
        wait("""
          const toolbar = getComputedStyle(document.querySelector(".reader-controls"));
          return toolbar.visibility === "visible" && toolbar.opacity === "1";
        """)
        execute("""
          document.querySelector("#reader-back").addEventListener("click", event => {
            if (event.isTrusted)
              sessionStorage.setItem("shelf-bench-back", String(performance.timeOrigin + performance.now()));
          }, {capture: true, once: true});
        """)
        click("#reader-back")
        returned = wait(SHELF_READY, book_id, RETURN_SENTINEL, BOOKS)
        if not returned["clk_ms"] or returned["time_ms"] < returned["clk_ms"]:
            raise RuntimeError("missing trusted reader-back click timestamp")
        startup_messages = [
            match
            for log in (data / PROFILE / "logs").glob("*.log*")
            for match in re.findall(
                r"event=application_start stage=ready mode=library duration_ms=(\d+)",
                log.read_text(errors="replace"),
            )
        ]
        if len(startup_messages) != 1 or ready["time_ms"] <= launch_ms:
            raise RuntimeError(
                "missing native setup receipt or inconsistent launch clock"
            )
        return {
            **residency,
            "startup_file_cache": cache,
            "return_file_cache": "after_start_and_open",
            "books": BOOKS,
            "webdriver_browser_version": session["capabilities"].get("browserVersion"),
            "native_setup_ready_ms": int(startup_messages[0]),
            "startup_usable_observed_ms": ready["time_ms"] - launch_ms,
            "return_usable_observed_ms": returned["time_ms"] - returned["clk_ms"],
            "history_same_document": returned["page_origin_ms"]
            == ready["page_origin_ms"],
        }
    finally:
        request("DELETE", path, timeout=10)


def native(args, fixture):
    # The documented invocation supplies a private Xvfb display.
    with (
        args.driver_log.open("x") as log,
        tempfile.TemporaryDirectory(
            prefix="atha-shelf-", dir=args.driver_log.resolve().parent
        ) as run,
        tempfile.TemporaryDirectory(prefix="atha-shelf-runtime-") as runtime,
    ):
        root = Path(run)
        env = dict(
            os.environ,
            GDK_BACKEND="x11",
            ATHA_READER_GUI_GATE="1",
            ATHA_READER_GUI_VIEWPORT="1000x760",
            WEBKIT_DISABLE_COMPOSITING_MODE="1",
            WEBKIT_SKIA_ENABLE_CPU_RENDERING="1",
            XDG_RUNTIME_DIR=runtime,
            GTK_USE_PORTAL="0",
            GSETTINGS_BACKEND="memory",
            GIO_USE_VFS="local",
            NO_AT_BRIDGE="1",
        )
        with (
            private_bus(Path(runtime), log) as address,
            webdriver(log, dict(env, DBUS_SESSION_BUS_ADDRESS=address)) as request,
        ):
            for i in range(args.samples):
                for cache in ("evicted", "warm"):
                    for candidate, binary in candidates(args, i):
                        # Remove profiles after the driver group stops; WebKit may still be
                        # writing on exit just after a session has acknowledged DELETE.
                        sample = Path(tempfile.mkdtemp(dir=root))
                        result = native_sample(request, binary, sample, fixture, cache)
                        output({"sample": i + 1, "candidate": candidate, **result})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["backend", "native"])
    parser.add_argument("binary", type=lambda path: Path(path).resolve(strict=True))
    parser.add_argument("fixture", type=fixture_root)
    parser.add_argument(
        "--baseline",
        type=lambda path: Path(path).resolve(strict=True),
        help="interleave a second binary as the before candidate",
    )
    parser.add_argument("--samples", type=int, default=10)
    parser.add_argument(
        "--driver-log", type=Path, help="new file, required in native mode"
    )
    args = parser.parse_args()
    if args.samples < 1:
        parser.error("sample count must be positive")
    if args.mode == "native" and args.driver_log is None:
        parser.error("--driver-log is required in native mode")
    (backend if args.mode == "backend" else native)(args, args.fixture)


if __name__ == "__main__":
    main()
