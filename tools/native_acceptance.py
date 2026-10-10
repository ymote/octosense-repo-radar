#!/usr/bin/env python3
"""Drive Repo Radar through an owned hidden Makepad shell.

All actions are native instrument input. Assertions inspect actual widgets or
the Rust component's state inside this fresh profile. No app source injection,
model configuration, credential access, approval-state writes or global input.
Every run needs a new output directory; failures are retained, never rewritten.
Developer catalogs, protected dry runs and published catalogs are distinguished.
An authentic catalog envelope alone never proves public publication.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import signal
import shutil
import socket
import subprocess
import sys
import time
import urllib.parse
import urllib.request
import urllib.error
from native_dev_fixture import APP_ID, sha


def require(value, message):
    if not value:
        raise AssertionError(message)
    return value


def catalog_scope(development, requested, public_store=False):
    if public_store:
        require(not development and requested is None, "Public store uses the host's default catalog, without a scope override")
        return "github-public"
    if development:
        require(requested is None, "Catalog scope applies only to --public-catalog")
        return "private-development"
    require(requested in ("github-verified", "github-dry-run", "github-public"),
            "Set --catalog-scope explicitly for a genuine catalog mirror")
    return requested


def json_pointer(value, pointer):
    if pointer == "":
        return value
    if not pointer.startswith("/"):
        raise ValueError("JSON pointer must begin with /")
    for field in pointer[1:].split("/"):
        field = field.replace("~1", "/").replace("~0", "~")
        value = value[int(field)] if isinstance(value, list) else value[field]
    return value


def select(rows, selector):
    allowed = {"id", "text", "type", "contains", "within_id", "min_size"}
    require(selector and set(selector) <= allowed, "Unsupported/empty widget selector")
    minimum = selector.get("min_size", [0, 0])
    require(isinstance(minimum, list) and len(minimum) == 2 and all(
        type(value) in (int, float) and math.isfinite(value) and 0 <= value <= 4096
        for value in minimum), "Invalid minimum widget size")
    container = select(rows, {"id": selector["within_id"]}) if "within_id" in selector else None
    if "within_id" in selector and container is None:
        return None
    candidates = []
    for row in rows:
        rect = row.get("r", [0, 0, 0, 0])
        if len(rect) != 4 or rect[2] <= 0 or rect[3] <= 0:
            continue
        if rect[2] < minimum[0] or rect[3] < minimum[1]:
            continue
        if selector.get("id") is not None and row.get("i") != selector["id"]:
            continue
        if selector.get("text") is not None and row.get("t") != selector["text"]:
            continue
        if selector.get("type") is not None and row.get("ty") != selector["type"]:
            continue
        if selector.get("contains") is not None and selector["contains"] not in row.get("t", ""):
            continue
        if container:
            x, y, width, height = container["r"]
            if not (x <= rect[0] and y <= rect[1] and rect[0] + rect[2] <= x + width + 1
                    and rect[1] + rect[3] <= y + height + 1):
                continue
            if row.get("window_id", row.get("w")) != container.get("window_id", container.get("w")):
                continue
        candidates.append(row)
    require(len(candidates) <= 1, "Ambiguous selector; refine it: " + str(selector))
    return candidates[0] if candidates else None


def processes():
    # Start time + command identity, never arguments/environment or credentials.
    result = subprocess.run(["ps", "-axo", "pid=,ppid=,lstart=,comm="], capture_output=True,
                            text=True, check=True, timeout=10)
    rows = {}
    for line in result.stdout.splitlines():
        fields = line.strip().split(None, 2)
        if len(fields) == 3:
            rows[int(fields[0])] = (int(fields[1]), fields[2])
    return rows


def scroll_until(ui, step, actions):
    count = step.get("max_scrolls", 8)
    dy = step.get("dy", 400)
    timeout = step.get("timeout")
    require(type(count) is int and 1 <= count <= 12 and type(dy) in (int, float)
            and math.isfinite(dy) and 0 < abs(dy) <= 1000, "Unbounded scroll request")
    require(timeout is None or (type(timeout) in (int, float) and math.isfinite(timeout)
            and 0 < timeout <= 120), "Invalid scroll observation timeout")
    for attempt in range(count + 1):
        rows = ui.rows()
        if select(rows, step["until"]):
            return
        if attempt == count:
            require(timeout is not None, "Target did not become visible after bounded scrolling")
            # Installation can finish after scrolling has stopped. Observe its
            # result without repeating input or increasing the scroll budget.
            actions.append({"op": "scroll_wait", "until": step["until"],
                            "timeout": timeout, "additional_input": False})
            ui.wait(lambda: select(ui.rows(), step["until"]), timeout=timeout)
            return
        row = select(rows, step["selector"])
        require(row is not None, "Scroll surface disappeared")
        x, y, width, height = row["r"]
        ui.call("m", k="scroll", x=x+width/2, y=y+height/2,
                w=row.get("window_id", row.get("w", 0)), dy=dy, wait=1)
        actions.append({"op": "scroll", "selector": step["selector"], "dy": dy})
        time.sleep(.15)


class Native:
    def __init__(self, binary, env, out, name, target, actions):
        require(target in ("apphub", "hub:" + APP_ID), "Only App Hub and Repo Radar can launch")
        self.out, self.name, self.actions = out, name, actions
        self.log_path = out / (name + ".log")
        self.log = self.log_path.open("w")
        self.child, self.endpoint, self.closed = None, None, False
        self.descendants, self.next_process_check = {}, 0
        self.http = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        try:
            self.child = subprocess.Popen([str(binary), "--remote=" + str(port), "--test-action", "launch-" + target],
                cwd=out, env={**env, "MAKEPAD_HIDE_WINDOWS": "1"}, stdout=self.log,
                stderr=subprocess.STDOUT, start_new_session=True)
            self.endpoint = "http://127.0.0.1:" + str(port)
            deadline = time.monotonic() + 60
            while time.monotonic() < deadline:
                require(self.child.poll() is None, "Owned shell exited before readiness")
                text = self.log_path.read_text(errors="replace")
                if re.search(r"listening on 127\.0\.0\.1:" + str(port) + r"\s+pid=" + str(self.child.pid) + r"\b", text):
                    # Bind the transport to the owned process before any input.
                    owned = subprocess.run(["/usr/sbin/lsof", "-nP", "-iTCP:" + str(port), "-sTCP:LISTEN", "-F", "p"],
                                           capture_output=True, text=True, timeout=10, check=True)
                    require(set(re.findall(r"^p(\d+)$", owned.stdout, re.M)) == {str(self.child.pid)}, "Instrument port belongs to another process")
                    self.call("s")
                    self.actions.append({"op": "launch", "session": name, "target": target, "pid": self.child.pid, "port_owner_verified": True})
                    return
                time.sleep(.1)
            raise TimeoutError("Owned native instrument did not become ready")
        except Exception:
            self.close()
            raise

    def call(self, route, **query):
        # Input is dispatched normally; assert observable state separately.
        # The frozen host may fail its optional synchronous present after input applied.
        if route in ("click", "k", "t", "m") and query.get("wait"):
            query["wait"] = 0
        require(self.child is not None and self.child.poll() is None, "Owned shell is not running")
        if time.monotonic() >= self.next_process_check:
            self.remember_children()
        url = self.endpoint + "/" + route + ("?" + urllib.parse.urlencode(query) if query else "")
        for attempt in range(8):
            try:
                with self.http.open(url, timeout=25) as response:
                    result = json.load(response)
                break
            except urllib.error.HTTPError as error:
                body = error.read().decode(errors="replace")
                self.actions.append({"op":"instrument_error","route":route,"status":error.code,"body":body,"input_replayed":False})
                if route not in ("snap", "s") or error.code != 404 or attempt == 7:
                    raise
                # Read-only observation: a starting/hidden window can be between frames.
                time.sleep(.25)
        require(not result.get("err"), "Native instrument: " + str(result.get("err")))
        return result

    def rows(self):
        return self.call("snap").get("s", [])

    def wait(self, predicate, timeout=30):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            require(self.child.poll() is None, "Owned shell exited while awaiting state")
            self.check_logs()
            result = predicate()
            if result:
                return result
            time.sleep(.1)
        raise TimeoutError("Expected state did not settle within " + str(timeout) + "s")

    def click(self, selector):
        row = self.wait(lambda: select(self.rows(), selector))
        require(row.get("enabled", True), "Target is disabled")
        x, y, width, height = row["r"]
        require(width >= 8 and height >= 8, "Target is too clipped for a reliable native click")
        window = row.get("w", row.get("window_id"))
        windows = self.call("s").get("w", [])
        candidate = next((w for w in windows if w.get("i", w.get("id")) == window), None)
        if candidate is None and len(windows) == 1:
            candidate = windows[0]
        require(candidate is not None, "Cannot bind widget to an owned window")
        size = candidate.get("sz", [])
        require(len(size) == 2 and x >= 0 and y >= 0 and x + width <= size[0] + 1
                and y + height <= size[1] + 1, "Target is clipped/outside its window; scroll first")
        query = {"x": x + width / 2, "y": y + height / 2, "wait": 1}
        if window is not None:
            query["w"] = window
        self.call("click", **query)
        self.actions.append({"op": "click", "selector": selector, "rect": row["r"], "window": window})
        return row

    def capture(self, name):
        require(re.fullmatch(r"[a-zA-Z0-9_-]+", name), "Unsafe capture name")
        # Only the app drawable; never an OS/window/screen capture fallback.
        for attempt in range(8):
            try:
                with self.http.open(self.endpoint + "/g?raw=1", timeout=25) as response:
                    data = response.read(32 * 1024 * 1024 + 1)
                break
            except urllib.error.HTTPError as error:
                body = error.read().decode(errors="replace")
                self.actions.append({"op":"capture_unavailable","name":name,"status":error.code,"body":body})
                if error.code != 404 or attempt == 7:
                    raise
                time.sleep(.25)
        require(data.startswith(b"\x89PNG\r\n\x1a\n") and len(data) <= 32 * 1024 * 1024, "Native capture did not return bounded PNG bytes")
        (self.out / (name + ".png")).write_bytes(data)
        (self.out / (name + ".snapshot.json")).write_text(json.dumps(self.call("snap"), indent=2) + "\n")
        (self.out / (name + ".geometry.json")).write_text(json.dumps(self.call("s"), indent=2) + "\n")
        self.actions.append({"op": "capture", "name": name, "png_sha256": hashlib.sha256(data).hexdigest(), "visual_review": "pending"})

    def refuse_agent(self, shell_home, app):
        """Observe and decline the custom host prompt, without editing consent."""
        require(app in ("apphub", APP_ID), "Cannot drive another app's agent consent")
        consent_path = shell_home / "approvals" / "consent.json"
        def choice():
            if not consent_path.is_file():
                return None
            return json.loads(consent_path.read_text()).get("apps", {}).get(app, {}).get("allowed")
        require(choice() is not True, "Expected an isolated profile without agent approval")
        if choice() is False:
            self.actions.append({"op": "agent_refused", "app": app, "already_refused": True})
            return
        name = "agent-consent-" + app.replace(".", "-")
        self.capture(name)
        point_path = self.out / (name + "-refuse-point.json")
        print("Inspect " + name + ".png and .geometry.json; write the observed Don't allow point in logical window coordinates to "
              + str(point_path) + ' as {"x": number, "y": number}.', flush=True)
        self.wait(point_path.is_file, timeout=300)
        point = json.loads(point_path.read_text())
        size = self.call("s")["w"][0]["sz"]
        require(set(point) == {"x", "y"} and all(type(point[k]) in (int, float)
                and math.isfinite(point[k]) and 0 <= point[k] < size[i]
                for i, k in enumerate(("x", "y"))), "Observed refusal point is outside the owned window")
        self.call("click", **point, wait=1)
        self.wait(lambda: choice() is False)
        self.actions.append({"op": "agent_refused", "app": app, "point": point,
                             "point_source": "Operator inspected this run's native frame",
                             "consent_file_read_only": True})
        self.capture(name + "-refused")

    def check_logs(self):
        text = self.log_path.read_text(errors="replace")
        markers = ("ScriptError", "callback error", "on_render closure failed", "panicked at", "thread panicked", "[ERROR]")
        hits = [line for line in text.splitlines() if any(marker in line for marker in markers)]
        require(not hits, "Native hard error; inspect the preserved session log")

    def remember_children(self):
        if self.child is None:
            return
        rows = processes()
        owners = {self.child.pid}
        while True:
            found = {pid for pid, (parent, _) in rows.items() if parent in owners} - owners
            if not found:
                break
            owners.update(found)
        for pid in owners - {self.child.pid}:
            self.descendants[pid] = rows[pid][1]
        self.next_process_check = time.monotonic() + 1

    def close(self):
        if self.closed:
            return
        forced = False
        if self.child is not None:
            self.remember_children()
            if self.child.poll() is None:
                try:
                    self.call("quit")
                except (OSError, ValueError, AssertionError):
                    pass
                try:
                    self.child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    forced = True
                    os.killpg(self.child.pid, signal.SIGTERM)
                    try:
                        self.child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(self.child.pid, signal.SIGKILL)
                        self.child.wait(timeout=5)
            def survivors():
                current = processes()
                return [pid for pid, identity in self.descendants.items()
                        if pid in current and current[pid][1] == identity]
            deadline = time.monotonic() + 2
            while survivors() and time.monotonic() < deadline:
                time.sleep(.1)
            for sig, grace in ((signal.SIGTERM, 3), (signal.SIGKILL, 2)):
                remaining = survivors()
                if remaining:
                    forced = True
                for pid in remaining:
                    try:
                        os.kill(pid, sig)
                    except ProcessLookupError:
                        pass
                deadline = time.monotonic() + grace
                while survivors() and time.monotonic() < deadline:
                    time.sleep(.1)
            self.actions.append({"op": "cleanup", "session": self.name,
                                 "exit_code": self.child.returncode, "forced": forced,
                                 "remaining_descendants": len(survivors())})
        self.log.close()
        self.closed = True


def environment(out, catalog=None, anchor=None):
    require(catalog is not None or anchor is None, "A trust anchor requires an explicit development catalog")
    env = {k: os.environ[k] for k in ("PATH", "HOME", "LANG", "LC_ALL", "SHELL") if k in os.environ}
    for var, directory in {"OCTOSENSE_HOME": "shell", "OCTOS_APP_CORE_DIR": "kernel", "OCTOSENSE_APP_DATA": "apps",
                           "RINX_DATA_DIR": "rinx", "ROBRIX_DATA_DIR": "robrix", "XDG_CONFIG_HOME": "config",
                           "XDG_DATA_HOME": "data", "XDG_CACHE_HOME": "cache", "XDG_STATE_HOME": "state", "TMPDIR": "temp"}.items():
        path = out / "profile" / directory
        path.mkdir(mode=0o700, parents=True, exist_ok=False)
        env[var] = str(path)
    env.update(OCTOSENSE_MAIL_VAULT="file", OCTOSENSE_LLM_VAULT="file", OCTOSENSE_SECRETS="file")
    if catalog is not None:
        env.update(OCTOSENSE_HUB=str(catalog), OCTOSENSE_HUB_CATALOG="legacy" if anchor else "github-v2")
    if anchor:
        env["OCTOSENSE_HUB_ANCHOR"] = anchor
    return env


def component_state(apps):
    root = apps / APP_ID
    require(not root.is_symlink(), "App root is a symlink")
    matches = list(root.rglob("repo-radar-v1.json")) if root.exists() else []
    require(len(matches) <= 1, "Ambiguous component state location")
    if not matches:
        return None
    path = matches[0]
    require(not path.is_symlink() and root.resolve() in path.resolve().parents, "Component state escapes app profile")
    require(path.stat().st_size <= 4 * 1024 * 1024, "Unexpectedly large component state")
    return json.loads(path.read_text())


def watchlist_view(state):
    """Read the component's persisted schema; never create or change it."""
    require(state.get("schema") == 1, "Unknown component storage schema")
    names = [state["repos"][str(pin["repo_id"])]["summary"]["full_name"] for pin in state["pins"]]
    selected = state.get("selected_repo_id")
    return {"pins": names, "selected": None if selected is None else state["repos"][str(selected)]["summary"]["full_name"]}


def main():
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", type=Path, required=True)
    parser.add_argument("--host-kind", choices=["development", "release-candidate", "released"], required=True)
    parser.add_argument("--expected-host-sha256", required=True)
    parser.add_argument("--hub", type=Path, required=True)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--dev-fixture", type=Path, help="fixture.json from native_dev_fixture.py")
    source.add_argument("--public-catalog", type=Path, help="Genuine catalog-v2.json mirror; no trust override")
    source.add_argument("--public-store", action="store_true", help="Use the released host's built-in public store without catalog overrides")
    parser.add_argument("--expected-catalog-sha256", help="Required with --public-store; from the separate public publication/readback receipt")
    parser.add_argument("--catalog-scope", choices=["github-verified", "github-dry-run", "github-public"],
                        help="Required with --public-catalog; a scope label is not publication proof")
    parser.add_argument("--scenario", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    scope = catalog_scope(bool(args.dev_fixture), args.catalog_scope, args.public_store)
    if args.public_store:
        require(args.host_kind == "released", "Public-store acceptance requires a verified released host")
        require(re.fullmatch(r"[0-9a-f]{64}", args.expected_catalog_sha256 or ""), "Expected public catalog hash is required")
    else:
        require(args.expected_catalog_sha256 is None, "Expected catalog hash applies only to --public-store")
    require(sys.platform == "darwin", "Only macOS native acceptance is implemented")
    binary, hub, out = args.host.resolve(strict=True), args.hub.resolve(strict=True), args.out.resolve()
    repository = Path(__file__).resolve().parents[1]
    require(repository not in out.parents and out != repository, "Evidence must stay outside the repository")
    require(re.fullmatch(r"[0-9a-f]{64}", args.expected_host_sha256), "Expected host hash is required")
    require(sha(binary) == args.expected_host_sha256, "Host differs from the explicitly selected executable")
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    # Keep the actual driver/scenario bytes too, not only hashes of files that
    # may later be improved after an original failure.
    shutil.copyfile(args.scenario, out / "scenario.json")
    harness = out / "harness"
    harness.mkdir(mode=0o700)
    for name in ("native_acceptance.py", "native_dev_fixture.py"):
        shutil.copyfile(Path(__file__).with_name(name), harness / name)
    actions, ui, checks = [], None, []
    receipt = {"schema": 1, "status": "running", "complete": False, "app_id": APP_ID, "host_kind": args.host_kind,
               "host_sha256": sha(binary), "hub_sha256": sha(hub), "scenario_sha256": sha(out / "scenario.json"),
               "harness_sha256": sha(harness / "native_acceptance.py"),
               "harness_support_sha256": sha(harness / "native_dev_fixture.py"),
               "catalog_scope": scope,
               "public_publication_proof": False, "model_or_credentials_configured": False,
               "input": "Makepad instrument synthetic native events; wait=0, separate state and pixels", "visual_review": "pending",
               "actions": actions, "checks": checks}
    try:
        anchor = None
        if args.dev_fixture:
            fixture = json.loads(args.dev_fixture.read_text())
            require(fixture.get("kind") == "private-legacy-development-fixture" and fixture.get("status") == "prepared"
                    and fixture.get("private_keys_removed") is True and fixture.get("app_id") == APP_ID, "Invalid development fixture")
            catalog = args.dev_fixture.resolve().parent / "mirror"
            anchor = fixture["anchor"]
            require(sha(catalog / "catalog.json") == fixture["catalog_sha256"], "Developer catalog changed")
            receipt["fixture_sha256"] = sha(args.dev_fixture)
            receipt["source_files_sha256"] = fixture["source_files_sha256"]
        elif args.public_store:
            catalog = None
            receipt["catalog_origin"] = "host-default-public-store"
            receipt["catalog_overrides"] = False
            receipt["expected_catalog_sha256"] = args.expected_catalog_sha256
        else:
            catalog = args.public_catalog.resolve(strict=True)
            verified = subprocess.run([str(hub), "catalog-verify", str(catalog / "catalog-v2.json")], capture_output=True, text=True, timeout=120)
            (out / "catalog-verification.log").write_text(verified.stdout + verified.stderr)
            require(verified.returncode == 0, "Public catalog proof verification failed")
        env = environment(out, catalog, anchor)
        if catalog is not None:
            receipt["catalog_sha256"] = sha(catalog / ("catalog.json" if anchor else "catalog-v2.json"))
        steps = json.loads((out / "scenario.json").read_text())["steps"]
        require(isinstance(steps, list) and 0 < len(steps) <= 300, "Scenario must contain 1–300 bounded steps")
        session = 0
        for index, step in enumerate(steps):
            op = step["op"]
            started = time.monotonic()
            if op in ("launch", "restart"):
                if ui:
                    ui.check_logs()
                    ui.close()
                session += 1
                ui = Native(binary, env, out, "session-" + str(session), step.get("target", "hub:" + APP_ID), actions)
            else:
                require(ui is not None, "Scenario must launch before input")
                if op == "click":
                    ui.click(step["selector"])
                elif op == "type":
                    selector = step["selector"]
                    ui.click(selector)
                    ui.call("k", k="press", c="KeyA", cmd=1, wait=1)
                    ui.call("t", t=step["text"], wait=1)
                    ui.wait(lambda: (select(ui.rows(), selector) or {}).get("t") == step["text"])
                    actions.append({"op": "type", "selector": selector, "text": step["text"]})
                elif op == "expect_widget":
                    def expected_widget():
                        row = select(ui.rows(), step["selector"])
                        return row if row and ("equals" not in step or row.get("t") == step["equals"]) else None
                    ui.wait(expected_widget, step.get("timeout", 45))
                elif op == "expect_stable_widget":
                    seconds = step.get("seconds", 2)
                    require(type(seconds) in (int, float) and 0 < seconds <= 5, "Bounded stability interval required")
                    end = time.monotonic() + seconds
                    samples, missing_since, present = [], None, 0
                    sample_path = out / ("stability-step-" + str(index) + ".json")
                    while time.monotonic() < end:
                        row = select(ui.rows(), step["selector"])
                        now = time.monotonic()
                        samples.append({"seconds": round(now - started, 3), "text": row.get("t") if row else None,
                                        "rect": row.get("r") if row else None})
                        sample_path.write_text(json.dumps(samples, indent=2) + "\n")
                        # A reactive subtree may briefly have no drawn area.
                        # Never accept another project's title; also fail an
                        # absent title lasting over half a second. Retain each
                        # sample so a redraw gap cannot mask a stale response.
                        if row is None:
                            missing_since = now if missing_since is None else missing_since
                            require(now - missing_since <= .5, "Selected title absent for more than 0.5s during redraw")
                        else:
                            require(row.get("t") == step["equals"], "Selected workspace changed to " + repr(row.get("t")))
                            missing_since = None
                            present += 1
                        time.sleep(.1)
                    require(missing_since is None and present >= 5, "Insufficient settled title observations")
                    actions.append({"op": op, "samples": sample_path.name, "present_samples": present})
                elif op == "expect_state":
                    def expected():
                        state = component_state(Path(env["OCTOSENSE_APP_DATA"]))
                        if state is None:
                            return False
                        try:
                            return json_pointer(state, step["pointer"]) == step["equals"]
                        except (KeyError, IndexError, TypeError):
                            return False
                    ui.wait(expected, step.get("timeout", 45))
                elif op == "expect_watchlist":
                    def expected_watchlist():
                        state = component_state(Path(env["OCTOSENSE_APP_DATA"]))
                        return state is not None and watchlist_view(state) == step["equals"]
                    ui.wait(expected_watchlist, step.get("timeout", 45))
                elif op == "expect_repo_cache":
                    require(step["field"] in ("overview", "pulls", "releases"), "Unknown repository cache kind")
                    def expected_cache():
                        state = component_state(Path(env["OCTOSENSE_APP_DATA"]))
                        return state and any(repo["summary"]["full_name"] == step["full_name"]
                            and repo.get(step["field"]) is not None for repo in state["repos"].values())
                    ui.wait(expected_cache, step.get("timeout", 60))
                elif op == "capture":
                    ui.capture(step["name"])
                elif op == "refuse_agent":
                    ui.refuse_agent(Path(env["OCTOSENSE_HOME"]), step["app"])
                elif op == "toggle_glance":
                    # Desktop's normal F9 binding. Publishing opens this
                    # host-owned overlay, which intercepts app keyboard input.
                    ui.call("k", k="press", c="F9", wait=1)
                    actions.append({"op": op, "key": "F9", "input": "owned native instrument"})
                elif op == "scroll":
                    row = ui.wait(lambda: select(ui.rows(), step["selector"]))
                    x, y, width, height = row["r"]
                    ui.call("m", k="scroll", x=x+width/2, y=y+height/2, dy=step["dy"], wait=1)
                    actions.append({"op": op, "selector": step["selector"], "dy": step["dy"]})
                elif op == "scroll_until":
                    scroll_until(ui, step, actions)
                elif op == "wait":
                    seconds = step["seconds"]
                    require(isinstance(seconds, (int, float)) and 0 <= seconds <= 5, "Only short bounded observation waits allowed")
                    time.sleep(seconds)
                else:
                    raise ValueError("Unsupported scenario action: " + op)
            checks.append({"step": index, "op": op, "label": step.get("label", ""), "passed": True,
                           "seconds": round(time.monotonic()-started, 3)})
            (out / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
        ui.check_logs()
        if args.public_store:
            cached = Path(env["OCTOSENSE_APP_DATA"]) / "catalog-v2.json"
            require(cached.is_file() and not cached.is_symlink(), "Host did not retain a public catalog")
            require(sha(cached) == args.expected_catalog_sha256, "Host catalog differs from the independently published catalog; preserve this run and investigate")
            evidence = out / "host-public-catalog-v2.json"
            shutil.copyfile(cached, evidence)
            verified = subprocess.run([str(hub), "catalog-verify", str(evidence)], capture_output=True, text=True, timeout=120)
            (out / "catalog-verification.log").write_text(verified.stdout + verified.stderr)
            require(verified.returncode == 0, "Host public catalog proof verification failed")
            receipt["catalog_sha256"] = sha(evidence)
        state = component_state(Path(env["OCTOSENSE_APP_DATA"]))
        if state is not None:
            (out / "final-component-state.json").write_text(json.dumps(state, indent=2) + "\n")
            receipt["final_component_state_sha256"] = sha(out / "final-component-state.json")
        receipt["status"] = "passed"
    except Exception as error:
        receipt["status"] = "failed"
        receipt["error"] = str(error)
        if ui and ui.child.poll() is None:
            try:
                ui.capture("original-failure")
            except Exception as capture_error:
                receipt["failure_capture_error"] = type(capture_error).__name__
    finally:
        if ui:
            ui.close()
        receipt["host_unchanged"] = sha(binary) == receipt["host_sha256"]
        receipt["complete"] = True
        if receipt["status"] == "running":
            receipt["status"] = "failed"
        if not receipt["host_unchanged"] or any(a.get("forced") or a.get("exit_code", 0) != 0 for a in actions if a["op"] == "cleanup"):
            receipt["status"] = "failed"
        receipt["not_verified"] = ["Human physical input", "Visual approval unless separately reviewed", "Android", "Model/agent answers", "Deterministic out-of-order network completion", "External browser link navigation", "Public app publication or update unless a separate proof receipt is attached"]
        (out / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(receipt["status"] + "; " + str(len(checks)) + " steps observed; receipt preserved")
    return 0 if receipt["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
