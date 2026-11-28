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
"""Turn real-host Playwright results into a reproducible upstream compatibility incident."""

from __future__ import annotations

import argparse
import base64
import difflib
import hashlib
import tomllib
import json
import os
import re
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterator

from members import changed_members

MARKER = "<!-- sugarcubes-upstream-compatibility -->"


@dataclass(frozen=True)
class Check:
    """Identify the affected user outcome across control and candidate reports."""

    name: str
    browser: str
    source: str
    line: int
    error: str
    attachments: tuple[str, ...]


def specs(suites: list[dict[str, Any]]) -> Iterator[dict[str, Any]]:
    """Include nested renderer, feature and state groups from Playwright."""
    for suite in suites:
        yield from suite.get("specs", [])
        yield from specs(suite.get("suites", []))


def checks(path: Path) -> dict[tuple[str, str], Check]:
    """Require a terminal result for each scheduled behavior and browser."""
    report = json.loads(path.read_text(encoding="utf-8"))
    if report.get("errors"):
        raise ValueError("Playwright reported a runner error")
    outcomes = {}
    for spec in specs(report["suites"]):
        for test in spec["tests"]:
            results = test["results"]
            if not results or results[-1]["status"] not in {
                "passed",
                "failed",
                "timedOut",
            }:
                raise ValueError(f"Incomplete check: {spec['title']}")
            error = ""
            if results[-1]["status"] != "passed":
                errors = results[-1].get("errors") or [{}]
                error = "\n\n".join(
                    str(item.get("message") or results[-1]["status"]) for item in errors
                )
                error = re.sub(r"\x1b\[[0-9;]*m", "", error)
            key = (spec["title"], test["projectName"])
            if key in outcomes:
                raise ValueError(f"Duplicate check identity: {key}")
            files = []
            for attachment in results[-1].get("attachments", []):
                if (
                    error
                    and attachment.get("name") == "measured-host-state"
                    and attachment.get("body")
                ):
                    measured = base64.b64decode(attachment["body"]).decode(
                        "utf-8", errors="replace"
                    )
                    error = error[:1000] + "\nMeasured host state:\n" + measured
                raw = attachment.get("path", "").replace("\\", "/")
                if "/artifacts/" in raw:
                    relative = Path("artifacts") / raw.split("/artifacts/", 1)[1]
                    target = (path.parent / relative).resolve()
                    if target.is_relative_to((path.parent / "artifacts").resolve()):
                        files.append(relative.as_posix())
            for actual in files:
                if actual.endswith("-actual.json"):
                    reference = actual.replace("-actual.json", "-expected.json")
                    if (
                        reference in files
                        and (path.parent / actual).is_file()
                        and (path.parent / reference).is_file()
                    ):
                        before = (
                            (path.parent / reference)
                            .read_text(encoding="utf-8")
                            .splitlines()
                        )
                        after = (
                            (path.parent / actual)
                            .read_text(encoding="utf-8")
                            .splitlines()
                        )
                        error = (
                            "\n".join(
                                difflib.unified_diff(
                                    before, after, "expected", "actual", n=2
                                )
                            )
                            + "\n"
                            + error
                        )
            outcomes[key] = Check(
                *key,
                "Sugar/tests/comfy_contract/integration/"
                + spec["file"].replace("\\", "/"),
                spec["line"],
                error,
                tuple(files),
            )
    if not outcomes:
        raise ValueError("No compatibility outcomes were recorded")
    return outcomes


def presentation(location: Path) -> dict[tuple[str, str], dict[str, list[str]]]:
    """Read observed native element roles and CSS properties by scenario and browser."""
    samples = {}
    for file in (location / "presentation").glob("*.json"):
        sample = json.loads(file.read_text(encoding="utf-8"))
        samples[sample["scenario"], sample["browser"]] = sample["observed"]
    return samples


def observed_difference(error: str) -> str:
    """Prefer a measured size, a changed field, or the host error over a generic assertion header."""
    lines = [line.strip() for line in error.splitlines() if line.strip()]
    for line in lines:
        if "Expected an image" in line or "NativeRendererCompatibilityError" in line:
            return line[:240]
    measurements = [
        line
        for line in lines
        if line.startswith(
            ("Expected:", "Received:", "Expected string:", "Received string:")
        )
    ]
    if measurements:
        if len(measurements) == 1 and "Error: element(s) not found" in lines:
            selector = next(
                (
                    line.removeprefix("Locator: ")
                    for line in lines
                    if line.startswith("Locator: ")
                ),
                "the reported selector",
            )
            measurements.append(f"Observed: no element matched {selector}")
        return " ".join(measurements[:2])[:240]
    changes = [
        line
        for line in lines
        if line.startswith(("+", "-"))
        and not line.startswith(("+++", "---", "- Expected", "+ Received"))
    ]
    return (
        " ".join(changes[:2])[:240]
        if changes
        else next(iter(lines), "Unknown failure")[:240]
    )


def presentation_differences(
    control: dict[tuple[str, str], dict[str, list[str]]],
    candidate: dict[tuple[str, str], dict[str, list[str]]],
    passed: set[tuple[str, str]],
    sources: dict[str, list[str]],
) -> list[dict[str, object]]:
    """Compare passing scenes, retaining only control observations shared by both engines."""
    changes = []
    for identity, expected in sorted(control.items()):
        scenario, browser = identity
        if identity not in passed or identity not in candidate:
            continue
        sibling = control.get(
            (scenario, "firefox" if browser == "chromium" else "chromium"), {}
        )
        for selector, values in expected.items():
            actual = candidate[identity].get(selector, [])
            shared = sorted(set(values) & set(sibling.get(selector, [])))
            if not shared or set(shared).issubset(actual):
                continue
            changes.append(
                {
                    "scenario": scenario,
                    "browser": browser,
                    "selector": selector,
                    "expected": shared,
                    "actual": actual,
                    "sugarSources": sources.get(selector, []),
                }
            )
    return changes


def summarize(
    matrix: dict[str, Any],
    evidence: Path,
    run_url: str,
    *,
    require_full_suite: bool = False,
    require_ci_stages: bool = False,
) -> tuple[str, bool, bool]:
    """Compare like-for-like scenarios, keeping infrastructure failures explicitly unknown."""
    contracts_file = (
        Path(__file__).resolve().parents[2]
        / "Sugar/tests/comfy_contract/support/host-contracts.toml"
    )
    contracts = tomllib.loads(contracts_file.read_text(encoding="utf-8"))["contract"]
    lines = [
        MARKER,
        "## SugarCubes upstream compatibility",
        "",
        f"[Run and failure artifacts]({run_url})",
        "",
    ]
    baseline: dict[tuple[str, str], Check] = {}
    overview = ["| Host | Passed | Failed |", "| --- | ---: | ---: |"]
    regressions: dict[str, tuple[set[str], str, str]] = {}
    failed = False
    complete = True
    control_sources: dict[str, Any] = {}
    control_coverage: dict[str, Any] = {}
    control_digest: dict[str, str] = {}
    control_presentation: dict[tuple[str, str], dict[str, list[str]]] = {}
    selectors: dict[str, list[str]] = {}
    inventory = json.loads(
        (contracts_file.parent / "host-dependencies.json").read_text(encoding="utf-8")
    )
    sites: dict[tuple[str, str], list[int]] = {}
    ledger = evidence / "control/touch-points.json"
    if ledger.is_file():
        try:
            for site in json.loads(ledger.read_text(encoding="utf-8")):
                sites.setdefault(
                    (site["path"], site["kind"] + " " + site["assumption"]), []
                ).append(site["line"])
        except (OSError, ValueError, TypeError, KeyError) as error:
            lines.extend([f"**Source location inventory unavailable.** {error}", ""])
            complete, failed = False, True
    for path, entries in inventory.items():
        for entry in entries:
            if entry.startswith("host selector or token "):
                selector = entry.removeprefix("host selector or token ")
            elif match := re.fullmatch(
                r"""DOM lookup (?:'([^']+)'|"([^"]+)")""", entry
            ):
                selector = match[1] or match[2]
            else:
                continue
            selectors.setdefault(selector, []).append(path)
    for lane in matrix["include"]:
        name = lane["name"]
        location = evidence / name
        lines.append(
            f"### {name} · ComfyUI `{lane['core']}` · frontend `{lane['frontend']}`"
        )
        lines.append("")
        lines.extend(
            [
                f"[ComfyUI source](https://github.com/Comfy-Org/ComfyUI/tree/{lane['core']}) · [Installed frontend](https://pypi.org/project/comfyui-frontend-package/{lane['frontend']}/)",
                "",
            ]
        )
        stages: dict[str, str] = {}
        stages_file = location / "stages.json"
        try:
            if stages_file.is_file() or require_ci_stages:
                stages = json.loads(stages_file.read_text(encoding="utf-8-sig"))
                if not isinstance(stages, dict):
                    raise ValueError("Qualification stage evidence is malformed")
                if (
                    stages.get("install") != "success"
                    or stages.get("drill") == "failure"
                    or (name == "control" and stages.get("inventory") != "success")
                ):
                    lines.extend(
                        [
                            f"**Qualification infrastructure requires attention.** Installation: {stages.get('install')}. Inventory: {stages.get('inventory')}. Failure drill: {stages.get('drill')}.",
                            "",
                        ]
                    )
                    complete, failed = False, True
            coverage_file = location / "coverage-index.json"
            coverage = None
            if coverage_file.exists() or require_full_suite:
                coverage = json.loads(coverage_file.read_text(encoding="utf-8"))
                if (
                    not isinstance(coverage, dict)
                    or not coverage
                    or any(
                        not isinstance(row, dict)
                        or not isinstance(row.get("reached"), list)
                        or not isinstance(row.get("assertions"), list)
                        for row in coverage.values()
                    )
                ):
                    raise ValueError(
                        "Runtime source measurements unavailable or malformed"
                    )
            actual = json.loads(
                (location / "versions.json").read_text(encoding="utf-8")
            )
            if any(actual[key] != lane[key] for key in ("core", "frontend")):
                raise ValueError(
                    "Installed core/frontend pair differs from the scheduled pair"
                )
            stamp = location / "pack-source.json"
            if stamp.exists() or require_full_suite:
                fingerprint = json.loads(stamp.read_text(encoding="utf-8"))
                if name == "control":
                    control_digest = fingerprint
                elif control_digest and fingerprint != control_digest:
                    raise ValueError(
                        "Installed pack or test sources differ from the control. Replay the same SugarCubes checkout."
                    )
            outcomes = checks(location / "checks.json")
            if stages and (
                stages.get("runtime") not in {"success", "failure"}
                or (
                    stages["runtime"] == "failure"
                    and not any(c.error for c in outcomes.values())
                )
            ):
                lines.extend(
                    [
                        "**Host runner did not complete successfully.** Inspect the launcher and teardown logs for this lane.",
                        "",
                    ]
                )
                complete, failed = False, True
            scene = presentation(location)
            if require_full_suite and not scene:
                raise ValueError("Native selector and CSS measurements unavailable")
            if require_full_suite:
                for contract in contracts:
                    browsers = {
                        c.browser
                        for c in outcomes.values()
                        if contract["test"] in c.name
                    }
                    if browsers != {"chromium", "firefox"}:
                        raise ValueError(
                            f"Missing real-host proof for {contract['id']}: collected browsers {sorted(browsers)}"
                        )
                for check in outcomes.values():
                    if (
                        sum(contract["test"] in check.name for contract in contracts)
                        != 1
                    ):
                        raise ValueError(
                            f"Missing or ambiguous repair contract for {check.name}"
                        )
            sources = json.loads(
                (location / "upstream.json").read_text(encoding="utf-8")
            )
            if name == "control":
                baseline = outcomes
                control_sources = sources
                control_presentation = scene
                if any(check.error for check in outcomes.values()):
                    complete = False
            elif baseline and outcomes.keys() != baseline.keys():
                raise ValueError("Control and candidate collected different checks")
        except (OSError, KeyError, ValueError) as error:
            lines.extend([f"**Run incomplete.** {error}", ""])
            overview.append(f"| {name} | incomplete | incomplete |")
            complete = False
            failed = True
            continue
        if sources.get("error"):
            lines.extend([f"**Source inspection incomplete.** {sources['error']}", ""])
            complete, failed = False, True
        if name != "control" and control_presentation:
            passed = {
                key
                for key, check in outcomes.items()
                if not check.error and key in baseline and not baseline[key].error
            }
            missing_scenes = passed.intersection(control_presentation).difference(scene)
            if missing_scenes:
                lines.extend(
                    [
                        f"**Missing native presentation measurements for {len(missing_scenes)} passing checks.**",
                        "",
                    ]
                )
                complete, failed = False, True
            differences = presentation_differences(
                control_presentation, scene, passed, selectors
            )
            if differences:
                observation_hash = hashlib.sha256(
                    json.dumps(differences, sort_keys=True, ensure_ascii=True).encode()
                ).hexdigest()
                lines.append(f"<!-- observations:{observation_hash} -->")
                (location / "presentation-changes.json").write_text(
                    json.dumps(differences, indent=2), encoding="utf-8"
                )
                lines.extend(
                    [
                        f"**{len(differences)} native selector or CSS changes in passing scenarios require review.** Every entry in `{name}/presentation-changes.json` names the scene, browser, SugarCubes consumers and expected and actual native state.",
                        "",
                    ]
                )
                for item in differences[:4]:
                    lines.append(
                        f"- {item['scenario']} ({item['browser']}), `{item['selector']}`: `{item['expected']}` → `{item['actual']}`."
                    )
                lines.append("")
                failed = True
        if coverage is not None:
            reachable = {path for path, row in coverage.items() if row["reached"]}
            direct = {path for path, row in coverage.items() if row["assertions"]}
            lines.extend(
                [
                    f"Runtime measurements reached {len(reachable)} of {len(coverage)} indexed source files. {len(direct)} have an explicitly assigned outcome assertion. `{name}/coverage-index.json` lists every file, scenario, and coverage gap.",
                    "",
                ]
            )
            if name == "control":
                control_coverage = coverage
            elif control_coverage:
                lost = {
                    path: facts["reached"]
                    for path, facts in control_coverage.items()
                    if facts["reached"] and path not in reachable
                }
                if lost:
                    passing = {
                        f"{check.browser}: {check.name}"
                        for key, check in outcomes.items()
                        if not check.error
                        and key in baseline
                        and not baseline[key].error
                    }
                    unverified_losses = {
                        path
                        for path, scenarios in lost.items()
                        if not control_coverage[path]["assertions"]
                        and passing.intersection(scenarios)
                    }
                    (location / "coverage-changes.json").write_text(
                        json.dumps(lost, indent=2), encoding="utf-8"
                    )
                    lines.extend(
                        [
                            f"{len(lost)} previously exercised files had no measured calls on this host. See `{name}/coverage-changes.json` for the control scenarios that reached each file.",
                            "",
                        ]
                    )
                    if unverified_losses:
                        failed = True
                        lines.extend(
                            [
                                f"**{len(unverified_losses)} unasserted SugarCubes sources stopped executing despite a passing replay.** Check the recorded control scenarios for a silently bypassed integration.",
                                "",
                            ]
                        )
        elif require_full_suite:
            lines.extend(
                [
                    "**Runtime source measurements unavailable.** Inspect the host log and browser runner status.",
                    "",
                ]
            )
            failed, complete = True, False
        if (
            name != "control"
            and control_sources
            and not sources.get("error")
            and not control_sources.get("error")
        ):
            missing = sorted(
                set(control_sources["frontend"]) - set(sources["frontend"])
            )
            changed = {
                k: (v, sources["core"].get(k))
                for k, v in control_sources["core"].items()
                if sources["core"].get(k) != v
            }
            review = {}
            unverified: set[str] = set()
            for path, before in control_sources.get("definitions", {}).items():
                after = sources.get("definitions", {}).get(path)
                if before == after:
                    continue
                owners = sorted(
                    {
                        owner
                        for symbol in before["anchors"].split(", ")
                        for owner in control_sources.get("consumers", {}).get(
                            symbol, []
                        )
                    }
                )
                review[path] = {
                    "control": before,
                    "candidate": after,
                    "sugarSources": owners,
                }
                unverified.update(
                    owner
                    for owner in owners
                    if not control_coverage.get(owner, {}).get("assertions")
                )
            for symbol, original in control_sources.get("coreDefinitions", {}).items():
                candidate = sources.get("coreDefinitions", {}).get(symbol)
                if candidate and original["sha256"] == candidate["sha256"]:
                    continue
                review[symbol] = {
                    "control": original,
                    "candidate": candidate,
                    "sugarSources": original["sugarSources"],
                }
                unverified.update(
                    owner
                    for owner in original["sugarSources"]
                    if not control_coverage.get(owner, {}).get("assertions")
                )
            if review:
                (location / "upstream-review.json").write_text(
                    json.dumps(review, indent=2), encoding="utf-8"
                )
                lines.extend(
                    [
                        f"**{len(review)} upstream source definitions containing SugarCubes dependencies changed.** See `{name}/upstream-review.json` for the files, hashes, native selectors, callables and SugarCubes consumers.",
                        "",
                    ]
                )
                if unverified:
                    lines.extend(
                        [
                            f"**Upstream changes require review in {len(unverified)} SugarCubes files without a directly assigned live assertion.** Inspect the listed source dependencies and replay the relevant scenarios before accepting these changes.",
                            "",
                        ]
                    )
                    failed = True
            if missing or changed:
                lines.extend(
                    [
                        "**Upstream implementation changes requiring review**",
                        "",
                        "Compare these upstream definitions with the SugarCubes call sites in the host dependency inventory.",
                        "",
                    ]
                )
                for symbol in missing:
                    owners = control_sources.get("consumers", {}).get(symbol, [])
                    lines.append(
                        f"- Frontend source no longer contains `{symbol}` (control: `{control_sources['frontend'][symbol][0]}`). "
                        + ", ".join(f"`{path}`" for path in owners[:3])
                    )
                for key, (before, after) in changed.items():
                    lines.append(
                        f"- Core `{key}`: `{before}` → `{after if after is not None else 'absent'}`."
                    )
                lines.append("")
                failed = True
        failures = [check for check in outcomes.values() if check.error]
        overview.append(f"| {name} | {len(outcomes)-len(failures)} | {len(failures)} |")
        lines.extend(
            [f"{len(outcomes) - len(failures)} of {len(outcomes)} checks passed.", ""]
        )
        member_report = {}
        for check in failures:
            previous = baseline.get((check.name, check.browser))
            if name == "control" or not previous or previous.error:
                continue
            observed = next(
                (
                    f
                    for f in check.attachments
                    if f.endswith("/native-members-actual.json")
                ),
                None,
            )
            if not observed:
                continue
            reference = (
                contracts_file.parent
                / "reference-images"
                / check.browser
                / "native-members.json"
            )
            try:
                differences = changed_members(
                    reference, location / observed, inventory, sites
                )
            except (OSError, ValueError) as error:
                lines.extend([f"**Native member evidence unavailable.** {error}", ""])
                complete, failed = False, True
                continue
            if differences:
                member_report[check.browser] = differences
                for entry in differences:
                    origin = ", ".join(
                        sorted({item["source"] for item in entry["consumers"]})[:3]
                    )
                    old = "present" if entry["control"] else "absent"
                    new = "present" if entry["candidate"] else "absent"
                    lines.append(
                        f"- Native member `{entry['member']}` ({check.browser}): {old} → {new}. Consumers: {origin or 'see host-dependencies.json'}."
                    )
        if member_report:
            (location / "native-member-changes.json").write_text(
                json.dumps(member_report, indent=2), encoding="utf-8"
            )
            lines.extend(
                [
                    "",
                    f"Every matched SugarCubes expression and available line number is in `{name}/native-member-changes.json`.",
                    "",
                ]
            )
        for check in failures:
            control = baseline.get((check.name, check.browser))
            classification = (
                "Control failure"
                if name == "control"
                else (
                    "New divergence"
                    if control and not control.error
                    else (
                        "Also fails on the control"
                        if control
                        else "Control unavailable"
                    )
                )
            )
            first_error = check.error.partition("\n\n")[0]
            if re.search(
                r'while setting up "(?:loaded|page|context|browser|backendMeasurement)"'
                r'|Tearing down "(?:context|browser)" exceeded|browserType\.launch:',
                first_error,
            ):
                classification = "Browser or host setup incomplete"
                complete = False
            previous_failure = ""
            if classification == "Also fails on the control" and control:
                previous_failure = observed_difference(control.error)
                if previous_failure != observed_difference(check.error):
                    classification = "Different failure on the candidate and control"
            contract = next((c for c in contracts if c["test"] in check.name), None)
            if classification == "New divergence":
                hosts, _, _ = regressions.setdefault(
                    check.name,
                    (
                        set(),
                        observed_difference(check.error),
                        f"{check.source}:{check.line}",
                    ),
                )
                hosts.add(f"{name} ({check.browser})")
            repair = (
                [
                    contract["assumption"],
                    "",
                    contract["repair"],
                    "",
                    "SugarCubes: "
                    + ", ".join(f"`{owner}`" for owner in contract["owners"]),
                    "",
                ]
                if contract
                else []
            )
            filter_expression = re.escape(check.name).replace("'", "''")
            lines.extend(
                [
                    f"**{classification} · {check.browser} · {check.name}**",
                    "",
                    *repair,
                    f"Reproduction and assertions: `{check.source}:{check.line}`",
                    "",
                    f"Single-check replay from Sugar with PowerShell: `python -m tools.compatibility.host --comfy $comfy --python $python --output $output -- --project {check.browser} --grep '{filter_expression}'`",
                    "",
                    *[f"- `{name}/{file}`" for file in check.attachments],
                    "",
                    "````text",
                    check.error[:2200].replace("````", "''''"),
                    "````",
                    "",
                ]
            )
            if classification == "Different failure on the candidate and control":
                lines.extend(
                    [
                        "Control observation:",
                        "",
                        "````text",
                        previous_failure.replace("````", "''''"),
                        "````",
                        "",
                    ]
                )
        if failures:
            failed = True
            lines.extend(
                [
                    "From Sugar/, with the reported ComfyUI checkout and its Python environment:",
                    "",
                    "```",
                    "python -m tools.compatibility.host --comfy <checkout> --python <host-python> --output <new-results-folder>",
                    "```",
                    "",
                ]
            )
    highlights = []
    if regressions:
        highlights.extend(["### Observed regressions", ""])
        for title, (hosts, example, source) in regressions.items():
            quoted = example.replace("`", "'")
            highlights.extend(
                [
                    f"- **{title}** — {', '.join(sorted(hosts))}. `{quoted}` (`{source}`)",
                    "",
                ]
            )
    lines[5:5] = [*overview, "", *highlights]
    return "\n".join(lines), failed, complete


def gh(*arguments: str) -> str:
    """Run the issue API in the reporting job only."""
    return subprocess.check_output(
        ["gh", *arguments], text=True, encoding="utf-8", timeout=60
    )


def publish(body: str, failed: bool, repo: str) -> None:
    """Update the one active incident, or resolve it after a fully green comparison."""
    title = "SugarCubes upstream compatibility needs attention"
    issues = json.loads(
        gh(
            "issue",
            "list",
            "--repo",
            repo,
            "--state",
            "open",
            "--search",
            f'"{title}" in:title',
            "--json",
            "number,title,body,author",
            "--limit",
            "100",
        )
    )
    current = next(
        (
            i
            for i in issues
            if i["title"] == title
            and i["body"].startswith(MARKER)
            and i["author"]["login"] in {"app/github-actions", "github-actions[bot]"}
        ),
        None,
    )
    if not current and not failed:
        return
    comparable = "\n".join(
        line.partition(" · ComfyUI ")[0] if line.startswith("### ") else line
        for line in body.splitlines()
        if line.startswith(
            (
                "### ",
                "**",
                "- Frontend source",
                "- Core",
                "Expected:",
                "Received:",
                "<!-- observations:",
                "Upstream release discovery failed",
            )
        )
    )
    signature = hashlib.sha256(comparable.encode()).hexdigest()
    marker = f"<!-- report:{signature} -->"
    unchanged = bool(current and marker in current["body"])
    clipped = body.encode("utf-8")[:61000].decode("utf-8", errors="ignore")
    if clipped != body:
        clipped += "\n\nFull comparison and every replay artifact are available from the run linked above."
    issue_body = clipped + "\n" + marker
    if current and current["body"] == issue_body and failed:
        return
    with tempfile.TemporaryDirectory() as temporary:
        file = Path(temporary) / "issue.md"
        file.write_text(issue_body, encoding="utf-8")
        if current:
            gh(
                "api",
                "--method",
                "PATCH",
                f"repos/{repo}/issues/{current['number']}",
                "-F",
                f"body=@{file}",
            )
            if failed and not unchanged:
                gh(
                    "issue",
                    "comment",
                    str(current["number"]),
                    "--repo",
                    repo,
                    "--body",
                    "The upstream comparison changed. The issue description contains the latest failures and replay evidence.",
                )
            elif not failed:
                gh("issue", "close", str(current["number"]), "--repo", repo)
        else:
            gh(
                "issue",
                "create",
                "--repo",
                repo,
                "--title",
                title,
                "--body-file",
                str(file),
            )


def main() -> None:
    """Save a job summary, publish the incident if authorized, and expose terminal state."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--matrix", help="Exact CI matrix, or omit to compare local lane directories"
    )
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--publish", action="store_true")
    parser.add_argument(
        "--focused",
        action="store_true",
        help="Compare an explicitly selected subset in a local replay",
    )
    options = parser.parse_args()
    repo = os.environ.get("GITHUB_REPOSITORY", "")
    ci = bool(os.environ.get("GITHUB_RUN_ID"))
    if ci and options.focused:
        parser.error("CI qualification requires the full suite")
    url = (
        f"https://github.com/{repo}/actions/runs/{os.environ['GITHUB_RUN_ID']}"
        if ci
        else options.evidence.resolve().as_uri()
    )
    if options.matrix is None:
        targets = [
            options.evidence / "control",
            *sorted(
                p
                for p in options.evidence.iterdir()
                if p.name != "control" and (p / "versions.json").is_file()
            ),
        ]
        if len(targets) < 2:
            parser.error(
                "Local comparison requires control/ and at least one candidate with versions.json"
            )
        pairs = []
        for target in targets:
            identity = json.loads(
                (target / "versions.json").read_text(encoding="utf-8")
            )
            pairs.append(
                {
                    "name": target.name,
                    "core": identity["core"],
                    "frontend": identity["frontend"],
                }
            )
        options.matrix = json.dumps({"include": pairs})
    if options.matrix:
        body, failed, complete = summarize(
            json.loads(options.matrix),
            options.evidence,
            url,
            require_full_suite=not options.focused,
            require_ci_stages=ci,
        )
    else:
        body = (
            f"{MARKER}\nUpstream release discovery failed.\n\n[Inspect the run]({url})."
        )
        failed, complete = True, False
    destination = (
        Path("compatibility-report.md")
        if ci
        else options.evidence / "compatibility-report.md"
    )
    destination.write_text(body, encoding="utf-8")
    if not ci:
        print(destination)
        if options.publish:
            parser.error("Issue publication requires the trusted CI reporting job")
        raise SystemExit(int(failed))
    with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a", encoding="utf-8") as output:
        output.write(
            body[:60000]
            + (
                "\n\nFull comparison in compatibility-report.md in the report artifact."
                if len(body) > 60000
                else ""
            )
        )
    if options.publish:
        publish(body, failed, repo)
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write(
            f"complete={str(complete).lower()}\nfailed={str(failed).lower()}\n"
        )


if __name__ == "__main__":
    main()
