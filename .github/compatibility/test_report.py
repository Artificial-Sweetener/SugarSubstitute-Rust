#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    This program is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU Affero General Public License for more details.
#
#    You should have received a copy of the GNU Affero General Public License
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.
"""Exercise alert delivery, comparison failures, retries and recovery through the normal report entry points."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from report import (
    MARKER,
    observed_difference,
    presentation_differences,
    publish,
    summarize,
)
from releases import CORE, resolve
from members import changed_members


class FailureEvidence(unittest.TestCase):
    """Make missing UI landmarks actionable from the incident summary."""

    def test_missing_graph_member_locates_its_consumers(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            control, latest = root / "control.json", root / "candidate.json"
            control.write_text('{"graph":["_links"],"widget":["_links"]}')
            latest.write_text('{"graph":[],"widget":["_links"]}')
            expression = "typed host member ComfyGraph._links"
            changes = changed_members(
                control,
                latest,
                {
                    "graph.ts": [expression],
                    "widget.ts": ["typed host member ComfyWidget._links"],
                },
                {("graph.ts", expression): [12]},
            )
            self.assertEqual(
                changes,
                [
                    {
                        "member": "graph._links",
                        "control": True,
                        "candidate": False,
                        "consumers": [
                            {
                                "source": "graph.ts",
                                "lines": [12],
                                "expression": expression,
                            }
                        ],
                    }
                ],
            )

    def test_missing_element_names_both_expectation_and_selector(self) -> None:
        message = (
            "Locator: locator('#toolbar')\nExpected: 44px\nError: element(s) not found"
        )
        self.assertEqual(
            observed_difference(message),
            "Expected: 44px Observed: no element matched locator('#toolbar')",
        )


class IssueLifecycle(unittest.TestCase):
    """Drive the GitHub CLI boundary while observing the actual issue body and incident identity."""

    def test_create_repeat_change_and_recover(self) -> None:
        """Notify once per new incident state and close a recovered incident."""
        issues: list[dict[str, object]] = []
        events: list[str] = []

        def github(*args: str) -> str:
            if args[:2] == ("issue", "list"):
                return json.dumps(issues)
            if args[:2] == ("issue", "create"):
                issues.append(
                    {
                        "number": 17,
                        "author": {"login": "github-actions[bot]"},
                        "title": args[args.index("--title") + 1],
                        "body": Path(args[-1]).read_text(encoding="utf-8"),
                    }
                )
                events.append("create")
            elif args[0] == "api":
                issues[0]["body"] = Path(args[-1].removeprefix("body=@")).read_text(
                    encoding="utf-8"
                )
                events.append("edit")
            else:
                events.append(args[1])
            return ""

        first = (
            MARKER
            + "\n### candidate · ComfyUI `old` · frontend `1`\n**New divergence · toolbar**\nExpected: 44px\nReceived: 52px"
        )
        changed = first.replace("52px", "60px")
        with patch("report.gh", github):
            publish(first, True, "test/repo")
            publish(first, True, "test/repo")
            publish(first.replace("`old`", "`next`"), True, "test/repo")
            publish(changed, True, "test/repo")
            publish(
                MARKER + "\n### candidate\n2 of 2 checks passed.", False, "test/repo"
            )
        self.assertEqual(events, ["create", "edit", "edit", "comment", "edit", "close"])


class ReleaseDiscovery(unittest.TestCase):
    """Select distinct recent stable releases and independently updated host components."""

    def test_discovery_orders_published_releases_and_avoids_redundant_pairs(
        self,
    ) -> None:
        """A draft or prerelease must never displace an installed stable ComfyUI release."""
        releases = [
            {
                "tag_name": f"v0.{n}.0",
                "published_at": str(n),
                "draft": n == 5,
                "prerelease": n == 4,
            }
            for n in (2, 0, 5, 3, 1, 4)
        ]

        def lane(ref: str, name: str) -> dict[str, str]:
            return {
                "name": name,
                "core": "v0.3.0" if ref == "HEAD" else ref,
                "frontend": "1.2.3",
            }

        def response(url: str) -> object:
            return releases if url.startswith(CORE) else {"info": {"version": "1.2.4"}}

        with patch("releases.fetch", side_effect=response), patch(
            "releases.core_lane", side_effect=lane
        ):
            matrix = resolve()
        self.assertEqual(
            [entry["name"] for entry in matrix],
            ["control", "v0.3.0", "v0.2.0", "v0.1.0", "standalone-frontend"],
        )
        self.assertEqual(
            matrix[-1],
            {"name": "standalone-frontend", "core": "v0.3.0", "frontend": "1.2.4"},
        )


class Qualification(unittest.TestCase):
    """Exercise green, broken and incomplete comparisons using actual reporter-format files."""

    def test_scene_differences_require_matching_control_engines_and_passing_replay(
        self,
    ) -> None:
        """Preserve the expected native role, actual role and SugarCubes repair location."""
        key = ("workflow actions", "chromium")
        control = {
            key: {".actions": ["button"], "--accent": ["red"], ".transient": ["div"]},
            (key[0], "firefox"): {".actions": ["button"], "--accent": ["red"]},
        }
        candidate = {key: {".actions": ["div[role=group]"], "--accent": ["red"]}}
        owners = {".actions": ["frontend/toolbar.ts"]}
        self.assertEqual(
            presentation_differences(control, candidate, set(), owners), []
        )
        changes = presentation_differences(control, candidate, {key}, owners)
        self.assertEqual(
            changes,
            [
                {
                    "scenario": key[0],
                    "browser": key[1],
                    "selector": ".actions",
                    "expected": ["button"],
                    "actual": ["div[role=group]"],
                    "sugarSources": ["frontend/toolbar.ts"],
                }
            ],
        )

    def test_classifies_candidate_control_and_missing_results(self) -> None:
        """Never classify a missing control or runtime artifact as a passing comparison."""
        matrix = {
            "include": [
                {"name": name, "core": "abc", "frontend": "1.2.3"}
                for name in ("control", "new")
            ]
        }
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)

            def record(
                name: str, status: str, error: str = "Expected: 44px\nReceived: 52px"
            ) -> None:
                lane = root / name
                lane.mkdir(exist_ok=True)
                (lane / "versions.json").write_text(
                    json.dumps(matrix["include"][0]), encoding="utf-8"
                )
                (lane / "upstream.json").write_text(
                    '{"core":{},"frontend":{}}', encoding="utf-8"
                )
                test = {
                    "projectName": "chromium",
                    "results": [
                        {
                            "status": status,
                            "errors": [{"message": error}],
                        }
                    ],
                }
                spec = {
                    "title": "Substitute frame retains a 44px toolbar row on the live graph",
                    "file": "host.live.ts",
                    "line": 14,
                    "tests": [test],
                }
                (lane / "checks.json").write_text(
                    json.dumps({"suites": [{"specs": [spec]}]}), encoding="utf-8"
                )

            record("control", "passed")
            record("new", "passed")
            self.assertEqual(summarize(matrix, root, "test-url")[1:], (False, True))
            stage_file = root / "new/stages.json"
            stage_file.write_text(
                '{"install":"success","runtime":"failure","inventory":"skipped","drill":"skipped"}',
                encoding="utf-8",
            )
            body, failed, complete = summarize(matrix, root, "test-url")
            self.assertTrue(failed and not complete)
            self.assertIn("Host runner did not complete successfully", body)
            stage_file.unlink()
            record("new", "failed")
            body, failed, complete = summarize(matrix, root, "test-url")
            self.assertTrue(failed and complete)
            self.assertIn("New divergence", body)
            self.assertIn("SubstituteToolbarLayout.ts", body)
            record(
                "new", "timedOut", 'Test timeout exceeded while setting up "loaded".'
            )
            body, failed, complete = summarize(matrix, root, "test-url")
            self.assertTrue(failed and not complete)
            self.assertIn("Browser or host setup incomplete", body)
            self.assertNotIn("New divergence", body)
            record("new", "failed")
            record("control", "failed")
            body, failed, complete = summarize(matrix, root, "test-url")
            self.assertTrue(failed and not complete)
            self.assertIn("Also fails on the control", body)
            (root / "control/checks.json").unlink()
            body, failed, complete = summarize(matrix, root, "test-url")
            self.assertTrue(failed and not complete)
            self.assertIn("Control unavailable", body)

            record("control", "passed")
            record("new", "passed")
            for name, digest in (("control", "old"), ("new", "new")):
                (root / name / "upstream.json").write_text(
                    json.dumps(
                        {
                            "core": {"server.PromptServer.queue_updated": "self"},
                            "frontend": {},
                            "coreDefinitions": {
                                "server.PromptServer.queue_updated": {
                                    "file": "server.py",
                                    "line": 27,
                                    "sha256": digest,
                                    "sugarSources": [
                                        "sugarcubes/backend/queue_order_routes.py"
                                    ],
                                }
                            },
                        }
                    ),
                    encoding="utf-8",
                )
            body, failed, complete = summarize(matrix, root, "test-url")
            self.assertTrue(failed and complete)
            self.assertIn("without a directly assigned live assertion", body)
            review = json.loads(
                (root / "new/upstream-review.json").read_text(encoding="utf-8")
            )
            self.assertEqual(
                review["server.PromptServer.queue_updated"]["candidate"]["sha256"],
                "new",
            )
            (root / "new/coverage-index.json").write_text("truncated", encoding="utf-8")
            body, failed, complete = summarize(matrix, root, "test-url")
            self.assertTrue(failed and not complete)
            self.assertIn("Run incomplete", body)


if __name__ == "__main__":
    unittest.main()
