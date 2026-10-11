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

    def scroll_case(self):
        ui = self.instrument()
        ui.call = Mock()
        ui.check_logs = Mock()
        surface = [{"i": "list", "r": [0, 0, 200, 300], "window_id": 1}]
        installed = surface + [{"t": "Installed", "r": [10, 10, 100, 20], "window_id": 1}]
        step = {"selector": {"id": "list"}, "until": {"text": "Installed"},
                "max_scrolls": 1, "dy": -500}
        return ui, surface, installed, step

    def test_delayed_install_is_observed_after_last_scroll_without_extra_input(self):
        ui, surface, installed, step = self.scroll_case()
        step["timeout"] = 90
        ui.rows = Mock(side_effect=[surface, surface, surface, installed])
        with patch.object(native.time, "sleep"):
            native.scroll_until(ui, step, ui.actions)
        self.assertEqual(ui.call.call_count, 1)
        self.assertEqual(ui.rows.call_count, 4)
        self.assertEqual([a["op"] for a in ui.actions], ["scroll", "scroll_wait"])
        self.assertEqual(ui.actions[-1]["timeout"], 90)
        self.assertFalse(ui.actions[-1]["additional_input"])

    def test_scroll_without_timeout_keeps_immediate_budget_failure(self):
        ui, surface, installed, step = self.scroll_case()
        ui.rows = Mock(side_effect=[surface, surface, installed])
        with patch.object(native.time, "sleep"):
            with self.assertRaisesRegex(AssertionError, "bounded scrolling"):
                native.scroll_until(ui, step, ui.actions)
        self.assertEqual(ui.call.call_count, 1)
        self.assertEqual(ui.rows.call_count, 2)
        self.assertEqual([a["op"] for a in ui.actions], ["scroll"])

    def test_delayed_install_timeout_still_fails_without_extra_input(self):
        ui, surface, _, step = self.scroll_case()
        step["timeout"] = 90
        ui.rows = Mock(return_value=surface)
        with patch.object(native.time, "sleep"), patch.object(native.time, "monotonic", side_effect=[0, 0, 91]):
            with self.assertRaisesRegex(TimeoutError, "did not settle"):
                native.scroll_until(ui, step, ui.actions)
        self.assertEqual(ui.call.call_count, 1)
        self.assertEqual(ui.rows.call_count, 3)

    def test_unbounded_scroll_observation_timeout_is_refused_before_input(self):
        for timeout in (0, -1, 121, float("nan"), float("inf"), True, "90"):
            with self.subTest(timeout=timeout):
                ui, _, _, step = self.scroll_case()
                step["timeout"] = timeout
                with self.assertRaisesRegex(AssertionError, "observation timeout"):
                    native.scroll_until(ui, step, ui.actions)
                ui.call.assert_not_called()

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

    def store_rows(self, name="Repo Radar"):
        return [
            {"i": "search", "ty": "TextInput", "t": "Repo Radar", "val": "Repo Radar", "r": [100, 30, 700, 40], "w": 0},
            {"i": "list", "ty": "PortalList", "r": [80, 100, 800, 500], "w": 0},
            {"i": "row", "ty": "View", "r": [100, 190, 760, 90], "w": 0},
            {"i": "name", "ty": "Label", "t": name, "r": [240, 205, 300, 24], "w": 0},
            {"i": "action", "ty": "Button", "t": "Get", "r": [700, 210, 80, 44], "w": 0},
        ]

    def test_store_filter_settles_before_one_identity_bound_click(self):
        ui = self.instrument()
        ui.check_logs = Mock()
        pending = self.store_rows() + [{"i": "row", "ty": "View", "r": [100, 290, 760, 90], "w": 0}]
        ui.rows = Mock(side_effect=[pending, self.store_rows("Another app"), self.store_rows()])
        ui.call = Mock(side_effect=[{"w": [{"i": 0, "sz": [1000, 700]}]}, {}])
        with patch.object(native.time, "sleep"):
            ui.click_store_result("Repo Radar")
        self.assertEqual(ui.rows.call_count, 3)
        self.assertEqual([c.args[0] for c in ui.call.call_args_list], ["s", "click"])
        self.assertEqual(ui.call.call_args_list[-1].kwargs, {"x": 740.0, "y": 232.0, "wait": 1, "w": 0})
        self.assertEqual(ui.actions[0]["observations"], {"ambiguous": 1, "missing_or_unsettled": 1, "matched": 1})
        self.assertFalse(ui.actions[0]["additional_input"])

    def test_permanently_ambiguous_store_results_fail_without_input(self):
        ui = self.instrument()
        ui.check_logs = Mock()
        ui.call = Mock()
        ui.rows = Mock(return_value=self.store_rows() + [
            {"i": "row", "ty": "View", "r": [100, 290, 760, 90], "w": 0}])
        with patch.object(native.time, "sleep"), patch.object(native.time, "monotonic", side_effect=[0, 0, 31]):
            with self.assertRaisesRegex(TimeoutError, "did not settle"):
                ui.click_store_result("Repo Radar")
        ui.call.assert_not_called()
        self.assertEqual(ui.actions[0]["observations"]["ambiguous"], 1)

    def test_search_text_alone_cannot_authorize_another_apps_get_button(self):
        ui = self.instrument()
        ui.check_logs = Mock()
        ui.call = Mock()
        ui.rows = Mock(return_value=self.store_rows("Another app"))
        with patch.object(native.time, "sleep"), patch.object(native.time, "monotonic", side_effect=[0, 0, 31]):
            with self.assertRaises(TimeoutError):
                ui.click_store_result("Repo Radar")
        ui.call.assert_not_called()

    def test_store_get_must_be_inside_the_same_named_result_and_window(self):
        for change in ({"r": [700, 310, 80, 44]}, {"w": 1}):
            with self.subTest(change=change):
                rows = self.store_rows()
                rows[-1].update(change)
                self.assertIsNone(native.store_result(rows, "Repo Radar"))

    def test_store_wait_does_not_hide_other_assertions_or_unbounded_timeout(self):
        ui = self.instrument()
        ui.check_logs = Mock()
        ui.call = Mock()
        ui.rows = Mock(return_value=[])
        with patch.object(native, "store_result", side_effect=AssertionError("Invalid minimum widget size")):
            with self.assertRaisesRegex(AssertionError, "Invalid minimum"):
                ui.click_store_result("Repo Radar")
        for timeout in (0, -1, 121, float("inf"), float("nan"), True, "30"):
            with self.subTest(timeout=timeout):
                with self.assertRaisesRegex(AssertionError, "store result timeout"):
                    ui.click_store_result("Repo Radar", timeout)
        ui.call.assert_not_called()

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
