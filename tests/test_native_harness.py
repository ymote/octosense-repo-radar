"""Safety and evidence tests; these never launch a shell or use the network."""
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch
import urllib.error
import urllib.parse

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import native_acceptance as native
import native_dev_fixture as fixture


class HarnessTests(unittest.TestCase):
    def instrument(self):
        # Exercise the real transport methods without starting a shell/network.
        ui = object.__new__(native.Native)
        ui.child = Mock()
        ui.child.poll.return_value = None
        ui.next_process_check = float("inf")
        ui.endpoint = "http://127.0.0.1:1"
        ui.actions = []
        ui.http = Mock()
        return ui

    def unavailable(self, route, status=404):
        return urllib.error.HTTPError(
            "http://127.0.0.1:1/" + route, status, "unavailable", {},
            io.BytesIO(b"native frame unavailable"))

    def test_applied_input_with_uncertain_response_is_never_replayed(self):
        for route in ("click", "k", "t", "m"):
            with self.subTest(route=route):
                ui = self.instrument()
                applied = []

                def apply_then_fail(url, **_):
                    applied.append(urllib.parse.urlparse(url))
                    raise self.unavailable(route)

                ui.http.open.side_effect = apply_then_fail
                with patch.object(native.time, "sleep") as sleep:
                    with self.assertRaises(urllib.error.HTTPError):
                        ui.call(route, wait=1)
                self.assertEqual(len(applied), 1)
                self.assertEqual(ui.http.open.call_count, 1)
                self.assertEqual(urllib.parse.parse_qs(applied[0].query)["wait"], ["0"])
                sleep.assert_not_called()
                self.assertEqual(ui.actions, [{"op": "instrument_error", "route": route,
                    "status": 404, "body": "native frame unavailable", "input_replayed": False}])

    def test_read_only_observation_can_recover_and_records_each_failure(self):
        for route in ("snap", "s"):
            with self.subTest(route=route):
                ui = self.instrument()
                ui.http.open.side_effect = [self.unavailable(route), self.unavailable(route),
                                            io.BytesIO(b'{"observed":"settled"}')]
                with patch.object(native.time, "sleep") as sleep:
                    self.assertEqual(ui.call(route), {"observed": "settled"})
                self.assertEqual(ui.http.open.call_count, 3)
                self.assertEqual(sleep.call_count, 2)
                self.assertEqual(len(ui.actions), 2)
                self.assertTrue(all(a["status"] == 404 and a["route"] == route
                                    and a["input_replayed"] is False for a in ui.actions))

    def test_read_only_observation_exhaustion_fails_after_eight_attempts(self):
        for route in ("snap", "s"):
            with self.subTest(route=route):
                ui = self.instrument()
                ui.http.open.side_effect = [self.unavailable(route) for _ in range(8)]
                with patch.object(native.time, "sleep") as sleep:
                    with self.assertRaises(urllib.error.HTTPError):
                        ui.call(route)
                self.assertEqual(ui.http.open.call_count, 8)
                self.assertEqual(sleep.call_count, 7)
                self.assertEqual(len(ui.actions), 8)

    def test_capture_recovery_still_collects_separate_state_and_geometry(self):
        ui = self.instrument()
        png = b"\x89PNG\r\n\x1a\nsynthetic-test-frame"
        ui.http.open.side_effect = [self.unavailable("g"), io.BytesIO(png),
                                   io.BytesIO(b'{"s":[]}'), io.BytesIO(b'{"w":[]}')]
        with tempfile.TemporaryDirectory() as folder, patch.object(native.time, "sleep"):
            ui.out = Path(folder)
            ui.capture("frame")
            self.assertEqual((ui.out / "frame.png").read_bytes(), png)
            self.assertEqual(json.loads((ui.out / "frame.snapshot.json").read_text()), {"s": []})
            self.assertEqual(json.loads((ui.out / "frame.geometry.json").read_text()), {"w": []})
        self.assertEqual([urllib.parse.urlparse(c.args[0]).path for c in ui.http.open.call_args_list],
                         ["/g", "/g", "/snap", "/s"])
        self.assertEqual([a["op"] for a in ui.actions], ["capture_unavailable", "capture"])

    def test_capture_exhaustion_keeps_failures_without_fake_frame(self):
        ui = self.instrument()
        ui.http.open.side_effect = [self.unavailable("g") for _ in range(8)]
        with tempfile.TemporaryDirectory() as folder, patch.object(native.time, "sleep") as sleep:
            ui.out = Path(folder)
            with self.assertRaises(urllib.error.HTTPError):
                ui.capture("frame")
            self.assertEqual(list(ui.out.iterdir()), [])
        self.assertEqual(ui.http.open.call_count, 8)
        self.assertEqual(sleep.call_count, 7)
        self.assertEqual(len(ui.actions), 8)
        self.assertTrue(all(a["op"] == "capture_unavailable" and a["status"] == 404
                            for a in ui.actions))

    def test_observation_retry_does_not_hide_other_transport_or_native_errors(self):
        ui = self.instrument()
        ui.http.open.side_effect = self.unavailable("snap", status=503)
        with patch.object(native.time, "sleep") as sleep:
            with self.assertRaises(urllib.error.HTTPError):
                ui.call("snap")
        self.assertEqual(ui.http.open.call_count, 1)
        sleep.assert_not_called()
        ui.http.open.reset_mock(side_effect=True)
        ui.http.open.return_value = io.BytesIO(b'{"err":"failed to inspect"}')
        with self.assertRaisesRegex(AssertionError, "failed to inspect"):
            ui.call("snap")
        self.assertEqual(ui.http.open.call_count, 1)

    def test_authentic_envelope_requires_explicit_publication_scope(self):
        self.assertEqual(native.catalog_scope(True, None), "private-development")
        self.assertEqual(native.catalog_scope(False, "github-dry-run"), "github-dry-run")
        self.assertEqual(native.catalog_scope(False, "github-public"), "github-public")
        with self.assertRaisesRegex(AssertionError, "explicitly"):
            native.catalog_scope(False, None)
        with self.assertRaisesRegex(AssertionError, "only"):
            native.catalog_scope(True, "github-public")

    def test_public_store_scope_cannot_relabel_a_mirror(self):
        self.assertEqual(native.catalog_scope(False, None, public_store=True), "github-public")
        with self.assertRaisesRegex(AssertionError, "without a scope override"):
            native.catalog_scope(False, "github-dry-run", public_store=True)
        with self.assertRaises(AssertionError):
            native.catalog_scope(True, None, public_store=True)

    def test_public_store_uses_no_inherited_catalog_or_trust_configuration(self):
        with tempfile.TemporaryDirectory() as folder, patch.dict(os.environ, {
                "OCTOSENSE_HUB": "/untrusted/mirror", "OCTOSENSE_HUB_CATALOG": "legacy",
                "OCTOSENSE_HUB_ANCHOR": "untrusted", "HTTPS_PROXY": "http://private-proxy"}):
            env = native.environment(Path(folder))
            for key in ("OCTOSENSE_HUB", "OCTOSENSE_HUB_CATALOG", "OCTOSENSE_HUB_ANCHOR", "HTTPS_PROXY"):
                self.assertNotIn(key, env)
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaisesRegex(AssertionError, "explicit development catalog"):
                native.environment(Path(folder), anchor="untrusted")
            self.assertFalse((Path(folder) / "profile").exists())

    def test_repeated_project_name_requires_scoped_selector(self):
        rows = [
            {"i": "sidebar", "r": [0, 0, 280, 600], "window_id": 1},
            {"i": "", "ty": "Label", "t": "rust-lang/rust", "r": [10, 100, 180, 25], "window_id": 1},
            {"i": "project_title", "ty": "Label", "t": "rust-lang/rust", "r": [320, 100, 300, 25], "window_id": 1},
        ]
        with self.assertRaisesRegex(AssertionError, "Ambiguous"):
            native.select(rows, {"text": "rust-lang/rust"})
        self.assertEqual(native.select(rows, {"text": "rust-lang/rust", "within_id": "sidebar"})["r"], [10, 100, 180, 25])

    def test_scoping_does_not_select_a_different_window_or_clipped_row(self):
        rows = [{"i": "sidebar", "r": [0, 0, 280, 600], "window_id": 1},
                {"t": "repo", "r": [10, 100, 180, 25], "window_id": 2},
                {"t": "repo", "r": [10, 590, 180, 25], "window_id": 1}]
        self.assertIsNone(native.select(rows, {"text": "repo", "within_id": "sidebar"}))

    def test_unknown_selector_does_not_silently_match(self):
        with self.assertRaisesRegex(AssertionError, "Unsupported"):
            native.select([], {"name": "search"})

    def test_clipped_chart_sliver_cannot_pass_render_acceptance(self):
        row = {"ty": "D3BarChart", "r": [0, 0, 600, 1]}
        selector = {"type": "D3BarChart", "min_size": [240, 120]}
        self.assertIsNone(native.select([row], selector))
        row["r"][3] = 220
        self.assertIs(native.select([row], selector), row)
        with self.assertRaisesRegex(AssertionError, "Invalid minimum"):
            native.select([row], {"type": "D3BarChart", "min_size": [240, float("nan")]})

    def test_watchlist_order_and_selection_come_from_stored_ids(self):
        state = {"schema": 1, "pins": [{"repo_id": 2}, {"repo_id": 1}], "selected_repo_id": 1,
                 "repos": {"1": {"summary": {"full_name": "org/first"}}, "2": {"summary": {"full_name": "org/second"}}}}
        self.assertEqual(native.watchlist_view(state), {"pins": ["org/second", "org/first"], "selected": "org/first"})
        state["schema"] = 2
        with self.assertRaisesRegex(AssertionError, "Unknown"):
            native.watchlist_view(state)

    def test_json_pointer_preserves_escaped_repository_keys(self):
        self.assertEqual(native.json_pointer({"org/repo": {"~": [5]}}, "/org~1repo/~0/0"), 5)
        with self.assertRaises(ValueError):
            native.json_pointer({}, "org/repo")

    def test_profile_does_not_inherit_provider_or_trust_overrides(self):
        with tempfile.TemporaryDirectory() as folder, patch.dict(os.environ, {
                "HOME": "/unchanged-home", "OCTOSENSE_HOME": "/personal",
                "OPENAI_API_KEY": "private-test-only", "OCTOSENSE_HUB_ANCHOR": "untrusted",
                "OCTOSENSE_APP_DATA": "/personal-apps", "HTTPS_PROXY": "http://private-proxy"}):
            out = Path(folder)
            env = native.environment(out, out / "catalog")
            self.assertEqual(env["HOME"], "/unchanged-home")
            self.assertEqual(Path(env["OCTOSENSE_APP_DATA"]), out / "profile/apps")
            self.assertNotIn("OPENAI_API_KEY", env)
            self.assertNotIn("HTTPS_PROXY", env)
            self.assertNotIn("OCTOSENSE_HUB_ANCHOR", env)
            self.assertEqual(env["OCTOSENSE_HUB_CATALOG"], "github-v2")
            with self.assertRaises(FileExistsError):
                native.environment(out, out / "catalog")

    def test_state_reader_refuses_symlink_escape_and_ambiguous_state(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            root = base / fixture.APP_ID
            root.mkdir()
            outside = base / "other.json"
            outside.write_text("{}")
            path = root / "repo-radar-v1.json"
            path.symlink_to(outside)
            with self.assertRaisesRegex(AssertionError, "escapes"):
                native.component_state(base)
            path.unlink()
            path.write_text("{}")
            (root / "nested").mkdir()
            (root / "nested/repo-radar-v1.json").write_text("{}")
            with self.assertRaisesRegex(AssertionError, "Ambiguous"):
                native.component_state(base)

    def test_fixture_refuses_sealed_source_before_creating_keys(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            bundle = root / "bundle"
            bundle.mkdir()
            (bundle / "manifest.json").write_text(json.dumps({"id": fixture.APP_ID, "version": "0.1.0", "integrity": {"github": {"release": "proof"}}}))
            with self.assertRaisesRegex(ValueError, "sealed"):
                fixture.prepare(bundle, root / "hub", root / "out")
            self.assertFalse((root / "out").exists())

    def test_fixture_gate_failure_keeps_original_evidence_and_removes_keys(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            bundle = root / "bundle"
            bundle.mkdir()
            (bundle / "manifest.json").write_text(json.dumps({"id": fixture.APP_ID, "version": "0.1.0"}))
            (bundle / "main.splash").write_text("original source")
            hub = root / "hub"
            hub.write_text("not executed")
            original = fixture.inventory(bundle)
            out = root / "out"
            def simulated(args, **_):
                command = args[1]
                if command == "keygen":
                    Path(args[2]).write_text("synthetic key")
                    stdout = "a" * 64
                elif command == "certify":
                    stdout = "b" * 128
                elif command == "stamp":
                    (Path(args[2]) / "main.splash").write_text("changed private copy")
                    stdout = ""
                elif command == "publish":
                    return subprocess.CompletedProcess(args, 1, "[refused] screenshot missing", "")
                else:
                    stdout = ""
                return subprocess.CompletedProcess(args, 0, stdout, "")
            with patch.object(fixture.subprocess, "run", side_effect=simulated):
                with self.assertRaisesRegex(RuntimeError, "original failure retained"):
                    fixture.prepare(bundle, hub, out)
            self.assertEqual(fixture.inventory(bundle), original)
            self.assertFalse((out / "ephemeral-keys").exists())
            receipt = json.loads((out / "fixture.json").read_text())
            self.assertEqual(receipt["status"], "failed")
            self.assertTrue(receipt["private_keys_removed"])
            self.assertFalse(receipt["public_publishing_proof"])
            self.assertEqual(receipt["steps"][-1]["exit_code"], 1)
            self.assertIn("screenshot missing", (out / "publish-private-copy.log").read_text())
            with self.assertRaises(FileExistsError):
                fixture.prepare(bundle, hub, out)

    def test_fixture_inventory_rejects_symlink(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "source.splash").write_text("source")
            (root / "link.splash").symlink_to(root / "source.splash")
            with self.assertRaisesRegex(ValueError, "symlink"):
                fixture.inventory(root)


if __name__ == "__main__":
    unittest.main()
