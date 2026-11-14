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
"""Prove release notes are cut from the changelog by version range, newest first."""

from __future__ import annotations

import base64
from pathlib import Path

from sugarcubes.backend.pack_install.pack_identity import parse_release_version
from sugarcubes.backend.release_notes import (
    NotesFailure,
    NotesUnavailable,
    ReleaseNotes,
    ReleaseNotesService,
    notes_in_range,
)
from sugarcubes.backend.release_notes.changelog import (
    MAX_NOTES_UNITS,
    MAX_RELEASES,
    MAX_TOTAL_NOTES_BYTES,
)
from sugarcubes.backend.self_update.http_client import HttpFailure, HttpFailureKind
from sugarcubes.backend.self_update.release_config import GitHubReleaseConfig

CHANGELOG = """## [0.16.1](https://github.test/compare/sugarcubes-v0.16.0...sugarcubes-v0.16.1) (2026-11-12)


### Bug Fixes

* **updates:** keep the line ([abc1234](https://github.test/commit/abc1234))

# [0.16.0](https://github.test/compare/sugarcubes-v0.15.3...sugarcubes-v0.16.0) (2026-11-10)


### Features

* **notes:** show what's new

## [0.15.3](https://github.test/compare/v0.15.2...v0.15.3) (2026-10-11)


### Bug Fixes

* **execution:** prepare noise

## 0.15.2 (2026-09-27)

* plain heading without a link
"""

CONFIG = GitHubReleaseConfig(
    api_root="https://api.github.test",
    repository="example/monorepo",
    tag_prefix="sugarcubes-v",
    asset_name="SugarCubes-{version}.zip",
    download_hosts=frozenset({"github.test"}),
)


def _versions(notes: ReleaseNotes | None) -> list[str]:
    assert notes is not None
    return [str(section.version) for section in notes.releases]


def test_a_range_keeps_every_version_after_since_newest_first() -> None:
    """Minor `#` and patch `##` sections both end at the next version heading."""

    notes = notes_in_range(
        CHANGELOG,
        since=parse_release_version("0.15.2"),
        through=parse_release_version("0.16.1"),
    )
    assert _versions(notes) == ["0.16.1", "0.16.0", "0.15.3"]
    assert notes is not None and notes.omitted == 0
    minor = notes.releases[1].notes
    assert minor.startswith("# [0.16.0]")
    assert "show what's new" in minor and "0.15.3" not in minor.split("\n", 1)[1]
    assert notes.releases[0].notes.endswith(
        "([abc1234](https://github.test/commit/abc1234))"
    )


def test_a_since_not_older_than_the_running_version_asks_for_it_alone() -> None:
    """The running version's own notes answer a same or newer since."""

    for since in ("0.16.1", "0.17.0"):
        notes = notes_in_range(
            CHANGELOG,
            since=parse_release_version(since),
            through=parse_release_version("0.16.1"),
        )
        assert _versions(notes) == ["0.16.1"]
    plain = notes_in_range(
        CHANGELOG,
        since=parse_release_version("0.15.2"),
        through=parse_release_version("0.15.2"),
    )
    assert plain is not None and plain.releases[0].notes.startswith("## 0.15.2")


def test_a_version_without_a_section_is_not_published() -> None:
    """No section for the running version means its notes are not published."""

    assert (
        notes_in_range(
            CHANGELOG,
            since=parse_release_version("0.1.0"),
            through=parse_release_version("0.17.0"),
        )
        is None
    )


def test_releases_beyond_the_bounds_are_counted_as_omitted() -> None:
    """The newest versions are kept; older ones are counted, never cut mid-text."""

    many = "\n".join(
        f"## [0.0.{patch}](x)\n\n* fix {patch}\n" for patch in range(80, 0, -1)
    )
    notes = notes_in_range(
        many,
        since=parse_release_version("0.0.0"),
        through=parse_release_version("0.0.80"),
    )
    assert notes is not None
    assert len(notes.releases) == MAX_RELEASES and notes.omitted == 80 - MAX_RELEASES
    assert str(notes.releases[0].version) == "0.0.80"
    block = ("x" * 99 + "\n") * 2500
    heavy = "".join(f"## [0.0.{patch}](x)\n\n{block}" for patch in range(4, 0, -1))
    bounded = notes_in_range(
        heavy,
        since=parse_release_version("0.0.0"),
        through=parse_release_version("0.0.4"),
    )
    assert _versions(bounded) == ["0.0.4", "0.0.3", "0.0.2"]
    assert bounded is not None and bounded.omitted == 1
    assert sum(len(s.notes.encode()) for s in bounded.releases) <= MAX_TOTAL_NOTES_BYTES


def test_an_oversized_section_is_cut_at_a_line_within_the_page_bound() -> None:
    """One version longer than the page admits keeps its whole leading lines."""

    lines = ("y" * 99 + "\n") * (MAX_NOTES_UNITS // 100 + 50)
    notes = notes_in_range(
        f"## [1.0.0](x)\n\n{lines}",
        since=parse_release_version("0.9.0"),
        through=parse_release_version("1.0.0"),
    )
    assert notes is not None
    text = notes.releases[0].notes
    assert len(text) <= MAX_NOTES_UNITS
    assert text.endswith("y" * 99) and text.startswith("## [1.0.0](x)")


class FakeContents:
    """Serve one contents API reply, or fail as the configured network does."""

    def __init__(
        self, reply: object = None, failure: HttpFailureKind | None = None
    ) -> None:
        """Remember the reply and every requested address."""

        self.reply = reply
        self.failure = failure
        self.urls: list[str] = []

    def get_json(
        self, url: str, *, allowed_hosts: frozenset[str], max_bytes: int
    ) -> object:
        """Answer one bounded read on the configured API host only."""

        assert allowed_hosts == frozenset({"api.github.test"})
        assert max_bytes > 0
        self.urls.append(url)
        if self.failure is not None:
            raise HttpFailure(self.failure, "test")
        return self.reply

    def download(self, *args: object, **kwargs: object) -> str:
        """Never download anything for release notes."""

        raise AssertionError("release notes never download")


def _service(http: FakeContents, running: str = "0.16.1") -> ReleaseNotesService:
    return ReleaseNotesService(config=CONFIG, http=http, running_version=running)


def _contents(text: str) -> dict[str, object]:
    encoded = base64.b64encode(text.encode("utf-8")).decode("ascii")
    # The contents API wraps base64 at sixty characters.
    wrapped = "\n".join(encoded[i : i + 60] for i in range(0, len(encoded), 60))
    return {"encoding": "base64", "content": wrapped}


def test_the_service_reads_the_changelog_at_the_running_release_tag() -> None:
    """One bounded contents API read at the running version's tag answers the range."""

    http = FakeContents(_contents(CHANGELOG))
    notes = _service(http).notes_since(parse_release_version("0.15.3"))
    assert isinstance(notes, ReleaseNotes)
    assert [str(s.version) for s in notes.releases] == ["0.16.1", "0.16.0"]
    assert http.urls == [
        "https://api.github.test/repos/example/monorepo/contents/Sugar/CHANGELOG.md"
        "?ref=sugarcubes-v0.16.1"
    ]


def test_a_missing_tag_is_not_published_and_other_failures_are_unavailable() -> None:
    """404 says nothing is published; offline, timeouts and bad replies may be retried."""

    since = parse_release_version("0.16.0")
    assert _service(FakeContents(failure=HttpFailureKind.NOT_FOUND)).notes_since(
        since
    ) == NotesUnavailable(NotesFailure.NOT_PUBLISHED)
    for kind in (
        HttpFailureKind.UNREACHABLE,
        HttpFailureKind.TIMED_OUT,
        HttpFailureKind.TOO_LARGE,
    ):
        assert _service(FakeContents(failure=kind)).notes_since(
            since
        ) == NotesUnavailable(NotesFailure.UNAVAILABLE)
    replies: tuple[object, ...] = (
        {"encoding": "none", "content": ""},
        {"encoding": "base64", "content": "@@"},
        [],
    )
    for reply in replies:
        assert _service(FakeContents(reply)).notes_since(since) == NotesUnavailable(
            NotesFailure.UNAVAILABLE
        )
    assert _service(FakeContents(_contents(CHANGELOG)), running="0.17.0").notes_since(
        since
    ) == NotesUnavailable(NotesFailure.NOT_PUBLISHED)
    assert _service(FakeContents(), running="dev").notes_since(
        since
    ) == NotesUnavailable(NotesFailure.NOT_PUBLISHED)


def test_the_changelog_path_names_the_shipped_component_folder() -> None:
    """The notes are read from the folder the release workflow writes them to."""

    root = Path(__file__).resolve().parents[3]
    assert (root / "CHANGELOG.md").is_file()
    assert root.name == "Sugar"
