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
"""Prove release metadata, configuration and transport policy fail closed."""

from __future__ import annotations

import io
import json
import tomllib
import urllib.request
from http.client import HTTPMessage
from pathlib import Path

import pytest

from sugarcubes.backend.pack_install.pack_identity import parse_release_version
from sugarcubes.backend.pack_install.startup import StartupOutcome, StartupOutcomeKind
from sugarcubes.backend.self_update.http_client import (
    HttpFailure,
    HttpFailureKind,
    _AllowListRedirects,
    require_allowed_url,
)
from sugarcubes.backend.self_update.release_config import (
    RELEASE_SOURCE_OVERRIDE,
    ReleaseConfigError,
    configured_release_source_path,
    load_release_config,
)
from sugarcubes.backend.self_update.release_sources import (
    GitHubReleases,
    ReleaseSourceKind,
)
from sugarcubes.backend.self_update.update_messages import describe_notice
from tests.self_update.support.fake_release_web import (
    CONFIG,
    GITHUB_TAGS,
    FakeReleaseWeb,
    github_tags_page,
)

_REPOSITORY = Path(__file__).resolve().parents[3]


def test_shipped_configuration_names_the_pack_and_its_release_asset() -> None:
    """The updater and the release workflow agree on tag and asset names."""

    config = load_release_config()
    pyproject = tomllib.loads(
        (_REPOSITORY / "pyproject.toml").read_text(encoding="utf-8")
    )
    assert config.pack_id == pyproject["project"]["name"]
    assert config.github_release.tag_for("1.2.3") == "sugarcubes-v1.2.3"
    assert config.github_release.asset_for("1.2.3") == "SugarCubes-1.2.3.zip"
    assert config.registry.archive_host == "cdn.comfy.org"


@pytest.mark.parametrize(
    "change",
    [
        {"schema": 2},
        {
            "registry": {
                "apiRoot": "http://api.comfy.org",
                "archiveHost": "cdn.comfy.org",
            }
        },
        {
            "registry": {
                "apiRoot": "https://user@api.comfy.org",
                "archiveHost": "cdn.comfy.org",
            }
        },
    ],
)
def test_invalid_configuration_is_rejected(
    tmp_path: Path, change: dict[str, object]
) -> None:
    """A configuration that is not plain HTTPS or has an unknown schema never loads."""

    payload = json.loads(
        (_REPOSITORY / "sugarcubes/backend/self_update/release_source.json").read_text(
            encoding="utf-8"
        )
    )
    payload.update(change)
    path = tmp_path / "release_source.json"
    path.write_text(json.dumps(payload), encoding="utf-8")
    with pytest.raises(ReleaseConfigError):
        load_release_config(path)


@pytest.mark.parametrize(
    "url",
    [
        "http://cdn.comfy.org/a/node.zip",
        "https://cdn.comfy.org.evil.example/a/node.zip",
        "https://user:pw@cdn.comfy.org/a/node.zip",
        "https://cdn.comfy.org:8443/a/node.zip",
        "file:///C:/Windows/win.ini",
    ],
)
def test_only_plain_https_to_allowed_hosts_is_requested(url: str) -> None:
    """Scheme, host, port and credentials are all checked before a request."""

    with pytest.raises(HttpFailure) as raised:
        require_allowed_url(url, frozenset({"cdn.comfy.org"}))
    assert raised.value.kind is HttpFailureKind.UNTRUSTED


def test_redirect_to_an_unlisted_host_is_refused() -> None:
    """A redirect cannot carry a download off the allow-listed hosts."""

    handler = _AllowListRedirects(frozenset({"github.com"}))
    request = urllib.request.Request("https://github.com/a")
    with pytest.raises(HttpFailure):
        handler.redirect_request(
            request,
            io.BytesIO(),
            302,
            "Found",
            HTTPMessage(),
            "https://evil.example/a.zip",
        )


def test_github_latest_is_found_by_sugarcubes_tags_alone() -> None:
    """Only a published release under a SugarCubes tag counts, and nothing else is read."""

    web = FakeReleaseWeb()
    web.publish_github("1.1.0", {"__init__.py": ""})
    web.publish_github("9.0.0", {"__init__.py": ""}, draft=True)
    web.publish_github("8.0.0", {"__init__.py": ""}, prerelease=True)
    tags = web.json[GITHUB_TAGS]
    assert isinstance(tags, list)
    # A tag that names no version, and a tag whose release was never published.
    tags.extend(
        [
            {"ref": "refs/tags/sugarcubes-vnext"},
            {"ref": "refs/tags/sugarcubes-v7.0.0"},
        ]
    )
    offer = GitHubReleases(config=CONFIG.github_release, http=web).latest()
    assert offer is not None
    assert offer.version == parse_release_version("1.1.0")
    assert offer.source is ReleaseSourceKind.GITHUB_RELEASE
    assert offer.sha256 is not None
    # Releases of other components in the repository are never listed or fetched.
    assert web.requests[0] == GITHUB_TAGS
    assert all("/releases/tags/sugarcubes-v" in url for url in web.requests[1:])
    assert not any("/releases?" in url for url in web.requests)


def test_github_latest_reads_every_page_of_sugarcubes_tags() -> None:
    """The newest tag is found when SugarCubes has more tags than one page holds."""

    web = FakeReleaseWeb()
    web.publish_github("2.0.0", {"__init__.py": ""})
    web.json[GITHUB_TAGS] = [
        {"ref": f"refs/tags/sugarcubes-v1.0.{patch}"} for patch in range(100)
    ]
    web.json[github_tags_page(2)] = [{"ref": "refs/tags/sugarcubes-v2.0.0"}]
    offer = GitHubReleases(config=CONFIG.github_release, http=web).latest()
    assert offer is not None
    assert offer.version == parse_release_version("2.0.0")
    assert web.requests[:2] == [GITHUB_TAGS, github_tags_page(2)]


def test_github_latest_is_absent_without_a_published_sugarcubes_release() -> None:
    """A repository with no SugarCubes tag, or only unpublished ones, offers nothing."""

    web = FakeReleaseWeb()
    web.publish_empty_github()
    assert GitHubReleases(config=CONFIG.github_release, http=web).latest() is None
    web.json[GITHUB_TAGS] = [{"ref": "refs/tags/sugarcubes-v3.0.0"}]
    assert GitHubReleases(config=CONFIG.github_release, http=web).latest() is None


@pytest.mark.parametrize("kind", list(StartupOutcomeKind))
def test_report_sentences_use_plain_words(kind: StartupOutcomeKind) -> None:
    """Report causes never mention internals the user cannot act on."""

    for reason in ("did-not-start", "interrupted", "changed-files"):
        sentence = describe_notice(StartupOutcome(kind, "1.1.0", "1.0.0", reason))
        lowered = sentence.casefold()
        for word in (
            "lock",
            "lease",
            "profile",
            "instance",
            "trial",
            "candidate",
            "quarantine",
            "tracking",
            "digest",
            "registry",
            "\\",
            "/",
        ):
            assert word not in lowered, (kind, reason, word)


def test_test_setup_can_point_the_updater_at_a_loopback_release_server(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """An isolated test ComfyUI names its own release file; only loopback may skip TLS."""

    payload = json.loads(
        (_REPOSITORY / "sugarcubes/backend/self_update/release_source.json").read_text(
            encoding="utf-8"
        )
    )
    payload["registry"] = {
        "apiRoot": "http://127.0.0.1:8392",
        "archiveHost": "127.0.0.1",
    }
    path = tmp_path / "release_source.json"
    path.write_text(json.dumps(payload), encoding="utf-8")
    monkeypatch.setenv(RELEASE_SOURCE_OVERRIDE, str(path))
    assert configured_release_source_path() == path
    assert load_release_config(path).registry.api_root == "http://127.0.0.1:8392"
    monkeypatch.delenv(RELEASE_SOURCE_OVERRIDE)
    assert configured_release_source_path().name == "release_source.json"
    assert configured_release_source_path() != path
