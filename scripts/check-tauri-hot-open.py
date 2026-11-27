#!/usr/bin/env python3
"""Verify Linux file association with real Tauri processes in a private display and D-Bus."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import contextlib
import fcntl
import http.client
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request


APP_ID = "com.atha.reader"
READY = """
const state = new URLSearchParams(location.search).get("state");
const progress = state && JSON.parse(localStorage.getItem(`atha.reader.progress.${state}.v1`));
if (document.documentElement.dataset.status !== "pass" ||
    !document.documentElement.hasAttribute("data-reader-ready") ||
    document.visibilityState !== "visible" || !progress) return null;
return { state, origin: performance.timeOrigin, anchor: JSON.parse(progress.locator).start };
"""


def require(predicate, message):
    if not predicate:
        raise RuntimeError(message)


def wait(check, failure, timeout=20):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = check()
        if result:
            return result
        time.sleep(0.03)
    raise RuntimeError(failure)


@contextlib.contextmanager
def private_bus(log):
    # No service directories: do not activate desktop portals, keyrings or
    # desktop services in the user's active session.
    with tempfile.TemporaryDirectory(prefix="atha-dbus-") as directory:
        config = Path(directory) / "session.conf"
        config.write_text(f"""
<busconfig>
  <type>session</type><listen>unix:tmpdir={directory}</listen>
  <policy context="default">
    <allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/>
  </policy>
</busconfig>
""")
        with subprocess.Popen(
            ["dbus-daemon", "--nofork", "--print-address", "--config-file", str(config)],
            stdout=subprocess.PIPE, stderr=log, text=True,
        ) as bus:
            try:
                address = bus.stdout.readline().strip()
                require(address, "private D-Bus did not start")
                os.environ["DBUS_SESSION_BUS_ADDRESS"] = address
                yield
            finally:
                bus.terminate()


def profile(root):
    values = {
        "HOME": root / "home",
        **{f"XDG_{name.upper()}_HOME": root / name for name in ("data", "cache", "config", "state")},
        "XDG_RUNTIME_DIR": root / "runtime",
    }
    for directory in values.values():
        directory.mkdir(parents=True, mode=0o700)
    return {
        **os.environ, **{key: str(value) for key, value in values.items()},
        "GDK_BACKEND": "x11", "GSETTINGS_BACKEND": "memory", "NO_AT_BRIDGE": "1",
        "WEBKIT_DISABLE_DMABUF_RENDERER": "1", "LIBGL_ALWAYS_SOFTWARE": "1",
        "ATHA_READER_GUI_GATE": "1", "ATHA_READER_GUI_VIEWPORT": "1000x760",
    }, values["XDG_DATA_HOME"] / APP_ID


@contextlib.contextmanager
def launch(command, env, log):
    process = subprocess.Popen(command, env=env, stdout=log, stderr=log, start_new_session=True)
    try:
        yield process
    finally:
        with contextlib.suppress(ProcessLookupError):
            os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()


class Driver:
    def __init__(self, port):
        self.origin = f"http://127.0.0.1:{port}"
        self.http = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        self.session = None

    def send(self, method, path, payload=None):
        request = urllib.request.Request(
            self.origin + path, None if payload is None else json.dumps(payload).encode(),
            {"Content-Type": "application/json"}, method=method,
        )
        try:
            with self.http.open(request, timeout=25) as response:
                return json.load(response)["value"]
        except urllib.error.HTTPError as error:
            raise RuntimeError(f"{method} {path}: {error.read().decode()}") from error

    def call(self, method, path, payload=None):
        return self.send(method, f"/session/{self.session}{path}", payload)

    def start(self, wrapper):
        self.session = self.send("POST", "/session", {
            "capabilities": {"alwaysMatch": {
                "tauri:options": {"application": str(wrapper)},
            }},
        })["sessionId"]

    def js(self, script):
        # A document change can reset the inspector connection. Report no
        # observation to the polling wait; never retry trusted input.
        try:
            return self.call("POST", "/execute/sync", {"script": script, "args": []})
        except (http.client.RemoteDisconnected, ConnectionResetError):
            return None

    def reader_authority(self):
        return self.call("POST", "/execute/async", {
            "script": """
              const done = arguments[0];
              async function inspect() {
                const response = await fetch("atha-book://localhost/.atha-reader.json", {cache: "no-store"});
                const manifest = await response.json();
                const edition = await window.athaMessages.edition(manifest.contentVersion);
                return [manifest.contentVersion, edition.title];
              }
              inspect().then(done, () => done(null));
            """,
            "args": [],
        })

    def click(self, css):
        item = self.call("POST", "/element", {"using": "css selector", "value": css})
        element = item["element-6066-11e4-a52e-4f735466cecf"]
        self.call("POST", f"/element/{element}/click", {})

    def center_click(self):
        x, y = self.js("""
          const box = document.querySelector(".reader").getBoundingClientRect();
          return [Math.round(box.left + box.width / 2), Math.round(box.top + box.height / 2)];""")
        self.call("POST", "/actions", {"actions": [{
            "type": "pointer", "id": "mouse", "parameters": {"pointerType": "mouse"},
            "actions": [{"type": "pointerMove", "x": x, "y": y, "origin": "viewport"},
                        {"type": "pointerDown", "button": 0}, {"type": "pointerUp", "button": 0}],
        }]})


@contextlib.contextmanager
def webdriver(binary, env, root, log, first=None):
    pid_file = root / "primary.pid"
    wrapper = root / "primary"
    argv = [str(binary)] + ([str(first)] if first else [])
    wrapper.write_text(f"""#!{sys.executable}
import os
with open({str(pid_file)!r}, "w") as f:
    f.write(str(os.getpid()))
os.execv({argv[0]!r}, {argv!r})
""")
    wrapper.chmod(0o700)
    with socket.socket() as port, socket.socket() as native:
        for reserved in (port, native):
            reserved.bind(("127.0.0.1", 0))
        host, internal = (reserved.getsockname()[1] for reserved in (port, native))
    cmd = [os.getenv("ATHA_TAURI_DRIVER", "tauri-driver"), "--port", str(host),
           "--native-port", str(internal)]
    with launch(cmd, env, log):
        driver = Driver(host)

        def listening():
            try:
                driver.send("GET", "/status")
                return True
            except OSError:
                return False

        wait(listening, "Tauri WebDriver did not start")
        try:
            yield driver, wrapper, pid_file
        finally:
            if driver.session:
                with contextlib.suppress(OSError, http.client.HTTPException):
                    driver.call("DELETE", "")


def app_log(root):
    return "".join(path.read_text(errors="replace") for path in root.rglob("*.log"))


def associated(binary, env, files, *paths):
    completed = subprocess.run([binary, *paths], cwd=files, env=env,
                               capture_output=True, text=True, timeout=12)
    require(completed.returncode == 0, f"secondary exit {completed.returncode}: {completed.stderr}")
    require(not completed.stdout and not completed.stderr, "secondary unexpectedly ran application setup")


def book_id(data, title, *, full=False):
    for path in (data / "Library").glob("*.json"):
        book = json.loads(path.read_text())
        if book["title"] == title:
            return book["id"] if full else book["id"][:16]
    return None


def reading(driver, expected=None):
    current = driver.js(READY)
    return current if current and (expected is None or current["state"] == expected) else None


def exercise(binary, root, log):
    files = root / "launch from here"
    files.mkdir()
    for name in ("Alpha Book", "Beta 书", "Gamma Book", "Blocked Book", "First Load Book"):
        (files / f"{name}.md").write_text("".join(
            f"# {name} Section {i}\n\n" + (f"Synthetic {name} body. " * 220) + "\n\n"
            for i in range(1, 5)
        ), encoding="utf-8")
    (files / "Broken Book.epub").write_bytes(b"not a zip")
    alpha = files / "Alpha Book.md"
    env, data = profile(root / "primary-profile")

    with webdriver(binary, env, root, log, alpha) as (driver, wrapper, primary_pid):
        driver.start(wrapper)
        pid = wait(lambda: primary_pid.read_text() if primary_pid.exists() else None, "missing primary PID")
        first = wait(lambda: reading(driver), "cold file association did not open a book")
        require(first["state"] == book_id(data, "Alpha Book"), "cold first book identity mismatch")
        alpha_edition = book_id(data, "Alpha Book", full=True)
        alpha_authority = driver.reader_authority()
        require(alpha_authority and alpha_authority[0] == alpha_edition
                and alpha_authority[1] not in ("", "未命名书籍"),
                f"cold reader authority did not identify the fixture: {alpha_authority}")
        driver.call("POST", "/actions", {"actions": [{
            "type": "key", "id": "keyboard", "actions": [
                {"type": "keyDown", "value": "\ue00f"}, {"type": "keyUp", "value": "\ue00f"},
            ],
        }]})
        progress = wait(
            lambda: changed_anchor(driver, first), "real PageDown did not move the saved Locator"
        )

        aliased = {**env, "XDG_DATA_HOME": env["XDG_DATA_HOME"] + "/"}
        associated(binary, aliased, files, "Beta 书.md")
        beta = wait(lambda: book_id(data, "Beta 书"), "relative file was not imported from sender cwd")
        try:
            second = wait(lambda: reading(driver, beta), "secondary process did not switch the original reader")
        except RuntimeError:
            print(driver.js("return [location.href, document.body.innerText.slice(0, 300)];"),
                  file=sys.stderr)
            raise
        require(primary_pid.read_text() == pid, "the original window process changed")
        require(second["origin"] != progress["origin"], "reader document did not switch")
        print("cold_file=passed secondary_relative_unicode_spaces=passed xdg_alias=passed", flush=True)

        before = app_log(root / "primary-profile").count("code=already-reading")
        associated(binary, env, files, (files / "Beta 书.md").as_uri(), "Beta 书.md")
        wait(lambda: app_log(root / "primary-profile").count("code=already-reading") > before,
             "duplicate launch was not recognized")
        require(reading(driver, beta) == second, "same edition reloaded or moved")
        require(len(list((data / "Library").glob("*.json"))) == 2, "duplicate created another book")

        associated(binary, env, files)
        associated(binary, env, files, *(["Gamma Book.md"] * 33))
        wait(lambda: "code=invalid-library-import" in app_log(root / "primary-profile"),
             "oversized path batch was not rejected")
        require(book_id(data, "Gamma Book") is None, "oversized batch was staged")
        associated(binary, env, files, "Broken Book.epub")
        wait(lambda: "code=no-prepared-book" in app_log(root / "primary-profile"),
             "invalid EPUB did not reach explicit prepare failure")
        require(reading(driver, beta) == second, "invalid file replaced the reader or its Locator")
        print("duplicate=passed no_arguments=passed oversized_batch=passed bad_book=passed", flush=True)

        for marker in (".atha-data-restore.json", ".atha-delete-" + "f" * 64):
            guard = data / marker
            guard.write_text("{}")
            try:
                before = app_log(root / "primary-profile").count("code=local-data-busy")
                associated(binary, env, files, "Blocked Book.md")
                wait(lambda: app_log(root / "primary-profile").count("code=local-data-busy") > before,
                     "unfinished maintenance did not reject the association")
                require(book_id(data, "Blocked Book") is None, "blocked book was staged")
                require(reading(driver, beta) == second, "maintenance guard replaced current book")
            finally:
                guard.unlink()

        isolated_env, isolated_data = profile(root / "other-profile")
        with launch([binary, files / "Gamma Book.md"], isolated_env, log):
            wait(lambda: book_id(isolated_data, "Gamma Book"), "independent data home was hijacked")
            require(book_id(data, "Gamma Book") is None and reading(driver, beta) == second,
                    "isolated launch contaminated the primary profile")
        print("restore_guard=passed deletion_guard=passed xdg_isolation=passed", flush=True)

        driver.center_click()
        wait(lambda: driver.js("""
          const controls = getComputedStyle(document.querySelector(".reader-controls"));
          return controls.visibility === "visible" && controls.opacity === "1";"""),
             "reader toolbar did not become visible")
        driver.click("#reader-back")
        # Shelf entries carry the complete identity, not the short state key.
        first_selector = f'.shelf-catalog [data-shelf-open^="{first["state"]}"]'
        wait(lambda: driver.js(f"return !!document.querySelector({json.dumps(first_selector)});"),
             "Back after hot open did not return to the shelf")
        driver.js("history.back(); return true;")
        until = time.monotonic() + 0.65
        while time.monotonic() < until:
            require(driver.js("return !location.search && location.pathname === '/';"),
                    "a second Back displayed a stale reader document")
            time.sleep(0.03)
        driver.click(first_selector)
        reopened = wait(lambda: reading(driver, first["state"]), "first book could not be reopened")
        require(reopened["anchor"] == progress["anchor"], "first book lost its saved Locator")
        require(driver.reader_authority() == alpha_authority,
                "first book reopened with the wrong book root or edition")
        print("back_to_shelf=passed stale_history_blocked=passed first_book_locator=passed", flush=True)

        app_profile = root / "primary-profile"
        with ThreadPoolExecutor(max_workers=1) as hooks:
            lock_ready = hooks.submit(fail_shelf_initialization, data, app_profile, "stage=prepared")
            associated(binary, env, files, "Gamma Book.md")
            lock_ready.result(timeout=15)
        restored = wait(lambda: reading(driver, first["state"]), "shelf failure did not restore A")
        require(restored["anchor"] == progress["anchor"] and restored["origin"] != reopened["origin"],
                "shelf rollback lost A's locator or did not restore its page")
        require(driver.reader_authority() == alpha_authority, "shelf rollback restored the wrong book root")
        wait(lambda: count_log(app_profile, "stage=rollback outcome=ok") == 1,
             "shelf failure did not reach one rollback")
        require(count_log(app_profile, "code=shelf-not-ready") == 1, "shelf failure emitted duplicate terminal results")

        with ThreadPoolExecutor(max_workers=1) as hooks:
            removed = hooks.submit(fail_first_read, data, app_profile)
            associated(binary, env, files, "First Load Book.md")
            removed.result(timeout=15)
        reader_restored = wait(lambda: reading(driver, first["state"]), "first-read failure did not restore A")
        require(reader_restored["anchor"] == progress["anchor"]
                and reader_restored["origin"] != restored["origin"], "first-read rollback lost A's locator")
        require(driver.reader_authority() == alpha_authority, "first-read rollback restored the wrong book root")
        wait(lambda: count_log(app_profile, "stage=rollback outcome=ok") == 2,
             "first-read rollback did not complete")
        require(count_log(app_profile, "code=first-read-failed") == 1, "failed first read emitted duplicate terminal results")
        associated(binary, env, files, "Gamma Book.md")
        gamma = wait(lambda: book_id(data, "Gamma Book"), "post-rollback file was not registered")
        wait(lambda: reading(driver, gamma), "post-rollback request could not use FIFO")
        print("shelf_failure_rollback=passed first_read_failure_rollback=passed next_request=passed", flush=True)

    cold_failure(binary, files, root, log)
    pre_ready(binary, files, root, log)
    from_shelf(binary, files, root, log)
    application_log = app_log(root)
    for private_value in (str(files), "Alpha Book", "Beta 书", "Gamma Book", "Broken Book", "First Load Book"):
        require(private_value not in application_log, "application log exposed a fixture path or title")
    print("log_privacy=passed", flush=True)


def count_log(profile_root, needle):
    return app_log(profile_root).count(needle)


def wait_new_log(profile_root, event, before, timeout=8):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if count_log(profile_root, event) > before:
            return
        time.sleep(0.002)
    raise RuntimeError(f"application did not log {event}")


def fail_shelf_initialization(data, profile_root, start):
    # Hold the real local-data lock only after successful preparation /
    # endpoint readiness, not during staging. All files are generated.
    slot = count_log(profile_root, start)
    failed = "outcome=failed code=shelf-not-ready"
    before = count_log(profile_root, failed)
    wait_new_log(profile_root, start, slot)
    with (data / ".atha-data.lock").open("r+b") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        wait_new_log(profile_root, failed, before)


def fail_first_read(data, profile_root):
    ready = count_log(profile_root, "stage=prepared")
    wait_new_log(profile_root, "stage=prepared", ready)
    edition = book_id(data, "First Load Book", full=True)
    require(edition, "test fixture was not staged")
    manifest = data / "ImportedBooks" / edition / ".atha-reader.json"
    held = manifest.with_name(".held-by-native-regression-test")
    manifest.rename(held)
    try:
        wait_new_log(profile_root, "code=first-read-failed", 0)
    finally:
        held.rename(manifest)


def cold_failure(binary, files, root, log):
    cold = root / "cold-failure"
    env, data = profile(cold)
    with webdriver(binary, env, cold, log, files / "Alpha Book.md") as (driver, wrapper, _):
        with ThreadPoolExecutor(max_workers=1) as hooks:
            failed = hooks.submit(fail_shelf_initialization, data, cold, "application_start stage=ready")
            driver.start(wrapper)
            failed.result(timeout=15)
        associated(binary, env, files, "Beta 书.md")
        beta = wait(lambda: book_id(data, "Beta 书"), "cold startup failure left FIFO occupied")
        wait(lambda: reading(driver, beta), "later handoff did not recover from cold startup failure")
    print("cold_pre_open_failure=passed", flush=True)


def pre_ready(binary, files, root, log):
    """Queue two real processes as soon as the first endpoint owns its bus name."""
    early = root / "early"
    env, data = profile(early)
    interface = [
        "gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
        "--object-path", "/org/freedesktop/DBus", "--method", "org.freedesktop.DBus.ListNames",
    ]

    def published():
        result = subprocess.check_output(interface, env=env, text=True, timeout=2)
        return APP_ID + ".Profile." in result

    with webdriver(binary, env, early, log, files / "Alpha Book.md") as (driver, wrapper, _):
        with ThreadPoolExecutor(max_workers=1) as startup:
            primary = startup.submit(driver.start, wrapper)
            wait(published, "single-instance endpoint did not publish")
            require("event=reader_ready" not in app_log(early), "first handoff missed startup interval")
            associated(binary, env, files, "Beta 书.md")
            require("operation=desktop_open outcome=ok" not in app_log(early),
                    "second handoff missed overlap interval")
            associated(binary, env, files, "Gamma Book.md")
            primary.result(timeout=25)
        gamma = wait(lambda: book_id(data, "Gamma Book"), "queued final book was not staged")
        wait(lambda: reading(driver, gamma), "queued final file did not win in submission order")
        wait(lambda: count_log(early, "code=reader-ready") == 2, "queued reader success was not reported")
        events = []
        for line in app_log(early).splitlines():
            if "event=reader_ready" in line:
                events.append("ready")
            elif "operation=desktop_open stage=activated" in line:
                events.append("activated")
            elif "operation=desktop_open outcome=ok code=reader-ready" in line:
                events.append("completed")
        require(events == ["ready", "activated", "ready", "completed", "activated", "ready", "completed"],
                f"queued requests reordered or interrupted startup: {events}")
    print("pre_ready_fifo=passed", flush=True)


def from_shelf(binary, files, root, log):
    shelf = root / "shelf"
    env, data = profile(shelf)
    with webdriver(binary, env, shelf, log) as (driver, wrapper, _):
        driver.start(wrapper)
        wait(lambda: driver.js("""
          return location.pathname === "/" &&
                 document.querySelector("main.library-shell")?.getAttribute("aria-busy") === "false";
        """), "normal no-argument primary did not show the shelf")
        associated(binary, env, files, "Beta 书.md")
        beta = wait(lambda: book_id(data, "Beta 书"), "shelf handoff did not register the book")
        wait(lambda: reading(driver, beta), "hot open from the shelf did not reach the reader")
    print("ordinary_shelf_start=passed hot_from_shelf=passed", flush=True)


def changed_anchor(driver, first):
    current = reading(driver, first["state"])
    return current if current and current["anchor"] != first["anchor"] else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("application", type=Path)
    parser.add_argument("--receipt-dir", type=Path, required=True)
    parser.add_argument("--inside-isolation", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    binary = args.application.resolve(strict=True)
    receipt = args.receipt_dir.resolve()
    receipt.mkdir(parents=True, exist_ok=True)
    if not args.inside_isolation:
        return subprocess.call([
            "xvfb-run", "-a", "-s", "-screen 0 1280x900x24",
            sys.executable, Path(__file__).resolve(),
            binary, "--receipt-dir", receipt, "--inside-isolation",
        ])
    with tempfile.TemporaryDirectory(prefix="atha-hot-open-", dir=receipt) as directory:
        root = Path(directory)
        with (receipt / "webdriver.log").open("w") as log, private_bus(log):
            try:
                exercise(binary, root, log)
            finally:
                (receipt / "application.log").write_text(app_log(root))
    print("tauri_hot_open=passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
