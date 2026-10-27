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
"""Require pinned, licensed, hash-verified icon sources and deployment notices."""

from __future__ import annotations

import hashlib
import re
import tomllib
from collections.abc import Mapping
from pathlib import Path
from xml.etree import ElementTree

ROOT = Path(__file__).resolve().parents[3]


def test_substitute_frame_artwork_has_pinned_licensed_deployment_bytes() -> None:
    """Verify every frame icon against pinned, licensed upstream and deployment bytes."""
    manifest = tomllib.loads((ROOT / "third_party/manifest.toml").read_text("utf-8"))
    expected = {
        "SugarSubstitute frame presentation": "GPL-3.0-or-later",
        "Substitute panel-control Fluent artwork": "MIT",
        "QFluent override pin artwork": "GPL-3.0-or-later",
        "QFluent application settings artwork": "GPL-3.0-or-later",
        "QFluent generation play artwork": "GPL-3.0-or-later",
        "QFluent generation queue artwork": "GPL-3.0-or-later",
        "Substitute generation segment artwork": "MIT",
    }
    recorded: set[Path] = set()
    for component in manifest["component"]:
        if component["name"] not in expected:
            continue
        assert component["license"] == expected[component["name"]]
        assert re.fullmatch(r"[0-9a-f]{40}", component["revision"])
        license_path = Path(component["license_file"])
        assert (ROOT / license_path).read_bytes() == (
            ROOT / "web" / license_path
        ).read_bytes()
        for asset in component["asset"]:
            authored = Path(asset["vendored_file"])
            assert authored.parts[:3] == ("frontend", "static", "orb")
            assert authored not in recorded
            recorded.add(authored)
            contents = (ROOT / authored).read_bytes()
            assert hashlib.sha256(contents).hexdigest() == asset["sha256"]
            assert (ROOT / asset["runtime_file"]).read_bytes() == contents
            if authored.suffix == ".svg":
                tree = ElementTree.fromstring(contents)
                assert tree.tag == "{http://www.w3.org/2000/svg}svg"
                assert not any(
                    element.tag.endswith("script") for element in tree.iter()
                )
    assert recorded == {
        path.relative_to(ROOT) for path in (ROOT / "frontend/static/orb").iterdir()
    }


def test_every_vendored_icon_has_exact_provenance_and_deployment_bytes() -> None:
    """Reject unrecorded assets, stale derivatives, unsafe SVGs, and missing notices."""
    manifest = tomllib.loads((ROOT / "third_party/manifest.toml").read_text("utf-8"))
    components: object = manifest.get("component")
    assert isinstance(components, list) and components
    recorded: set[str] = set()
    for component in components:
        assert isinstance(component, dict)
        if "asset" not in component:
            continue
        source = component["source"]
        if source == "https://gitlab.gnome.org/GNOME/adwaita-icon-theme":
            adwaita_paths = verify_adwaita_icon_provenance(component)
            assert recorded.isdisjoint(adwaita_paths)
            recorded.update(adwaita_paths)
            continue
        if source in {
            "https://github.com/Artificial-Sweetener/SugarSubstitute",
            "https://github.com/civitai/civitai",
            "https://github.com/OpenModelDB/open-model-database",
            "https://github.com/microsoft/fluentui",
            "https://github.com/microsoft/polyfills",
            "https://github.com/zhiyiYo/PyQt-Fluent-Widgets",
            "https://github.com/yuanzhixiang/openphoto",
        }:
            continue
        assert source in {
            "https://github.com/microsoft/fluentui-system-icons",
            "https://github.com/microsoft/fluentui-emoji",
        }
        assert component["license"] == "MIT"
        assert re.fullmatch(r"[0-9a-f]{40}", component["revision"])
        assert component["modifications"] == "None; SVG files copied verbatim."
        license_path = Path(component["license_file"])
        license_bytes = (ROOT / license_path).read_bytes()
        expected_copyright = (
            b"Copyright (c) 2020 Microsoft Corporation"
            if source.endswith("/fluentui-system-icons")
            else b"Copyright (c) Microsoft Corporation"
        )
        assert expected_copyright in license_bytes
        assert b"permission notice shall be included" in license_bytes
        assert (ROOT / "web" / license_path).read_bytes() == license_bytes
        assets: object = component["asset"]
        assert isinstance(assets, list) and assets
        for asset in assets:
            assert isinstance(asset, dict)
            source_path = asset["source_path"]
            authored_path = asset["vendored_file"]
            runtime_path = asset["runtime_file"]
            assert isinstance(source_path, str) and source_path.startswith("assets/")
            if source.endswith("/fluentui-system-icons"):
                assert source_path.endswith("_20_regular.svg")
            else:
                assert source_path in {
                    "assets/Game die/High Contrast/game_die_high_contrast.svg",
                    "assets/Locked/High Contrast/locked_high_contrast.svg",
                }
            assert isinstance(authored_path, str)
            assert authored_path.startswith("frontend/static/icons/")
            assert authored_path not in recorded
            recorded.add(authored_path)
            assert runtime_path == authored_path.replace("frontend/static/", "web/", 1)
            contents = (ROOT / authored_path).read_bytes()
            assert hashlib.sha256(contents).hexdigest() == asset["sha256"]
            assert (ROOT / runtime_path).read_bytes() == contents
            svg = ElementTree.fromstring(contents)
            assert svg.tag == "{http://www.w3.org/2000/svg}svg"
            for element in svg.iter():
                assert element.tag in {svg.tag, "{http://www.w3.org/2000/svg}path"}
                assert not any(key.lower().startswith("on") for key in element.attrib)
                assert not any("href" in key.lower() for key in element.attrib)
    actual = {
        path.relative_to(ROOT).as_posix()
        for path in (ROOT / "frontend/static/icons").rglob("*")
        if path.is_file()
    }
    assert recorded == actual


def verify_adwaita_icon_provenance(component: Mapping[str, object]) -> set[str]:
    """Verify pinned GTK3 artwork, passive SVGs, and complete deployed LGPL attribution."""
    assert component["name"] == "GTK3 Adwaita symbolic icons for GIMP"
    assert component["revision"] == "b496edba45cb664e92ac1333f40de515e518bec8"
    assert component["version"] == "3.38.0"
    assert component["license"] == "LGPL-3.0-only"
    license_path = Path("third_party/licenses/adwaita.LICENSE.txt")
    assert component["license_file"] == license_path.as_posix()
    license_bytes = (ROOT / license_path).read_bytes()
    assert b"GNU LESSER GENERAL PUBLIC LICENSE" in license_bytes
    assert b"GNU GENERAL PUBLIC LICENSE" in license_bytes
    assert b"When attributing the artwork" in license_bytes
    assert (ROOT / "web" / license_path).read_bytes() == license_bytes
    notice = (ROOT / "third_party/NOTICE.md").read_text("utf-8")
    assert "GNOME Project" in notice and "Adwaita" in notice
    expected = {
        "color-select-symbolic.svg": (
            "Adwaita/scalable/actions/color-select-symbolic.svg",
            "6efea1a6fa89236ff3781146e99eaf62efcf351ccf6f26ccc0d98107d3996310",
        ),
        "document-edit-symbolic.svg": (
            "Adwaita/scalable/actions/document-edit-symbolic.svg",
            "65d9f54782726558c3d5162a0bd9c588d413bae0df81df5f395350a6561aa3ee",
        ),
        "document-open-symbolic.svg": (
            "Adwaita/scalable/actions/document-open-symbolic.svg",
            "273d7267d4d23f023fa0405acae361544cad828dcce36de2338454804966daef",
        ),
        "document-save-symbolic.svg": (
            "Adwaita/scalable/actions/document-save-symbolic.svg",
            "abd68c3279cfd129c77a4676d7d87ef55143a919c69ebb6f951b7a4540bfc1a9",
        ),
        "edit-clear-all-symbolic.svg": (
            "Adwaita/scalable/actions/edit-clear-all-symbolic.svg",
            "7d42f19e2ce3646158d8f71280ef5f5e55a276ef53ae99c8ae7ba6e7a2bb3437",
        ),
        "edit-redo-symbolic.svg": (
            "Adwaita/scalable/actions/edit-undo-symbolic-rtl.svg",
            "54848579d52c1d09a8dade3ee3441e718544835a7d07455100e898d96767845a",
        ),
        "edit-undo-symbolic.svg": (
            "Adwaita/scalable/actions/edit-redo-symbolic-rtl.svg",
            "0ca274f687c42ed8d7e520f04efac2e26c605763449ef762f3153db760ef79f1",
        ),
        "folder-new-symbolic.svg": (
            "Adwaita/scalable/actions/folder-new-symbolic.svg",
            "632d0220ecb4cdbf4b5441da879ae4daca491e5d0b782dd6ed6e0c84e39890fb",
        ),
        "folder-open-symbolic.svg": (
            "Adwaita/scalable/status/folder-open-symbolic.svg",
            "12274f3a2e46b28d93f76b3390a32fa7f25891fb43d388a0802debc80948721b",
        ),
        "image-x-generic-symbolic.svg": (
            "Adwaita/scalable/mimetypes/image-x-generic-symbolic.svg",
            "406b237c7b1ab841e052d079d571470ad78637a4b01538d5d636115b820ef768",
        ),
        "list-add-symbolic.svg": (
            "Adwaita/scalable/actions/list-add-symbolic.svg",
            "0a17ecd8b2b8ce295e353bba28578ddcbf9dd1d283008eec2a404c5608d2da05",
        ),
        "list-remove-symbolic.svg": (
            "Adwaita/scalable/actions/list-remove-symbolic.svg",
            "170ab5cdca0284fd4f96da1c8ce624ed558f352de76fbed6ae7f0de4baa82940",
        ),
        "preferences-system-symbolic.svg": (
            "Adwaita/scalable/categories/preferences-system-symbolic.svg",
            "58adf1ecd4c7e419b22c2318ca545ccec2f9cb0a64a965e2c75bdd239b40b511",
        ),
        "system-run-symbolic.svg": (
            "Adwaita/scalable/actions/system-run-symbolic.svg",
            "a8d093b9c8db36fcb276c7cd80580dde2ead3b708d02bdf79472c65f9164d457",
        ),
        "view-refresh-symbolic.svg": (
            "Adwaita/scalable/actions/view-refresh-symbolic.svg",
            "6a71927ca3c8ff5442a7447c8358db9c72154e75add17c76de2f6a8507a82e8f",
        ),
        "window-close-symbolic.svg": (
            "Adwaita/scalable/ui/window-close-symbolic.svg",
            "88d97b39c5ea9ce305c9047e36e804883c0b7b439ef6880ddb9a6f0a78105e0c",
        ),
        "zoom-fit-best-symbolic.svg": (
            "Adwaita/scalable/actions/zoom-fit-best-symbolic.svg",
            "5492ef5d6baf353db401085dcce09b32fc6fba597d6a5de2dcaffd4fe49f7a33",
        ),
        "zoom-in-symbolic.svg": (
            "Adwaita/scalable/actions/zoom-in-symbolic.svg",
            "ac98c28d61d9b751a425cdd264811317172596e9f8030ddf6915f33a0d45efa6",
        ),
        "zoom-out-symbolic.svg": (
            "Adwaita/scalable/actions/zoom-out-symbolic.svg",
            "ea8c33dfb3c3dbfb1bba90c57d4a86d45a196cd384f3a477d2202392af797d25",
        ),
    }
    assets = component["asset"]
    assert isinstance(assets, list) and len(assets) == len(expected)
    recorded: set[str] = set()
    for asset in assets:
        assert isinstance(asset, dict)
        authored = asset["vendored_file"]
        assert isinstance(authored, str)
        name = Path(authored).name
        assert name in expected
        source_path, digest = expected[name]
        assert authored == f"frontend/static/icons/node/gimp/{name}"
        assert authored not in recorded
        recorded.add(authored)
        assert asset["source_path"] == source_path
        assert asset["runtime_file"] == f"web/icons/node/gimp/{name}"
        contents = (ROOT / authored).read_bytes()
        assert hashlib.sha256(contents).hexdigest() == asset["sha256"] == digest
        assert (ROOT / asset["runtime_file"]).read_bytes() == contents
        svg = ElementTree.fromstring(contents)
        assert svg.tag == "{http://www.w3.org/2000/svg}svg"
        widths = {
            "color-select-symbolic.svg": "16.006",
            "zoom-fit-best-symbolic.svg": "15.982",
            "zoom-in-symbolic.svg": "15.982",
            "zoom-out-symbolic.svg": "15.982",
        }
        assert svg.attrib["width"] == widths.get(name, "16")
        assert svg.attrib["height"] == "16"
        for element in svg.iter():
            assert element.tag in {
                "{http://www.w3.org/2000/svg}svg",
                "{http://www.w3.org/2000/svg}g",
                "{http://www.w3.org/2000/svg}path",
            }
            assert not any(
                key.lower().startswith("on") or "href" in key.lower()
                for key in element.attrib
            )
    assert {Path(path).name for path in recorded} == expected.keys()
    return recorded


def test_photopro_fonts_have_pinned_licensed_deployment_bytes() -> None:
    """Verify PhotoPro font provenance and deployed bytes independently of icons."""
    manifest = tomllib.loads((ROOT / "third_party/manifest.toml").read_text("utf-8"))
    components: object = manifest["component"]
    assert isinstance(components, list)
    matches = [
        component
        for component in components
        if isinstance(component, dict)
        and component.get("name") == "Source Sans 3 PhotoPro fonts"
    ]
    assert len(matches) == 1
    component = matches[0]
    assert component["source"] == "https://github.com/yuanzhixiang/openphoto"
    assert re.fullmatch(r"[0-9a-f]{40}", component["revision"])
    assert component["license"] == "OFL-1.1"
    license_path = Path(component["license_file"])
    license_bytes = (ROOT / license_path).read_bytes()
    assert b"SIL OPEN FONT LICENSE" in license_bytes
    assert (ROOT / "web" / license_path).read_bytes() == license_bytes
    assets: object = component["asset"]
    assert isinstance(assets, list) and len(assets) == 2
    recorded: set[Path] = set()
    for asset in assets:
        assert isinstance(asset, dict)
        source_path = asset["source_path"]
        authored_path = Path(asset["vendored_file"])
        runtime_path = Path(asset["runtime_file"])
        assert source_path == f"crates/op-ui/assets/fonts/{authored_path.name}"
        assert authored_path.parent == Path("frontend/static/fonts")
        assert runtime_path == Path("web/fonts") / authored_path.name
        assert authored_path not in recorded
        recorded.add(authored_path)
        contents = (ROOT / authored_path).read_bytes()
        assert hashlib.sha256(contents).hexdigest() == asset["sha256"]
        assert (ROOT / runtime_path).read_bytes() == contents
    assert recorded == {
        path.relative_to(ROOT)
        for path in (ROOT / "frontend/static/fonts").glob("SourceSans3-*.ttf")
    }


def test_detector_illustrations_preserve_reference_provenance() -> None:
    """Keep all seventeen copied detector assets licensed, recorded and byte-identical."""
    manifest = tomllib.loads((ROOT / "third_party/manifest.toml").read_text("utf-8"))
    components: object = manifest["component"]
    assert isinstance(components, list)
    detectors = [
        item
        for item in components
        if isinstance(item, dict)
        and item.get("name") == "SugarSubstitute detector illustrations"
    ]
    assert len(detectors) == 1
    component = detectors[0]
    assert component["license"] == "GPL-3.0-or-later"
    assert component["revision"] == "9e27255cd65e5d8d99543a693127083996e14924"
    license_path = Path(component["license_file"])
    license_bytes = (ROOT / license_path).read_bytes()
    assert b"GNU GENERAL PUBLIC LICENSE" in license_bytes
    assert (ROOT / "web" / license_path).read_bytes() == license_bytes
    assets: object = component["asset"]
    assert isinstance(assets, list) and len(assets) == 17
    recorded: set[str] = set()
    for asset in assets:
        assert isinstance(asset, dict)
        authored = asset["vendored_file"]
        assert isinstance(authored, str)
        assert authored.startswith(
            "sugarcubes/model_library/infrastructure/resources/detectors/"
        )
        assert asset["runtime_file"] == authored
        assert (
            asset["source_path"]
            == "substitute/infrastructure/model_thumbnails/resources/ultralytics/"
            + Path(authored).name
        )
        assert (
            hashlib.sha256((ROOT / authored).read_bytes()).hexdigest()
            == asset["sha256"]
        )
        recorded.add(authored)
    assert recorded == {
        path.relative_to(ROOT).as_posix()
        for path in (
            ROOT / "sugarcubes/model_library/infrastructure/resources/detectors"
        ).glob("*.png")
    }


def test_adapted_model_catalog_and_suggestions_have_pinned_source_provenance() -> None:
    """Record source revision, file fingerprints, adapted owners, and the distributed license."""
    manifest = tomllib.loads((ROOT / "third_party/manifest.toml").read_text("utf-8"))
    components: object = manifest["component"]
    assert isinstance(components, list)
    matching = [
        component
        for component in components
        if isinstance(component, dict)
        and component.get("name") == "SugarSubstitute model catalog and suggestions"
    ]
    assert len(matching) == 1
    component = matching[0]
    assert (
        component["source"] == "https://github.com/Artificial-Sweetener/SugarSubstitute"
    )
    assert component["license"] == "GPL-3.0-or-later"
    assert re.fullmatch(r"[0-9a-f]{40}", component["revision"])
    license_path = Path(component["license_file"])
    assert b"GNU GENERAL PUBLIC LICENSE" in (ROOT / license_path).read_bytes()
    assert (ROOT / "web" / license_path).read_bytes() == (
        ROOT / license_path
    ).read_bytes()
    implementations: object = component["implementation"]
    assert isinstance(implementations, list) and len(implementations) == 37
    expected_freshness = {
        "substitute/application/civitai/cache_service.py": "f9a77e7e08022e6a0cabcdb0fb8460f0f70c4ddcb137cb0be9bd89ab2dbdca1c",
        "substitute/presentation/settings/civitai_page.py": "10c451e01ae71025d484b88053e4929214650b65b8b00b0e3b7ec12ed6a1e8af",
        "substitute/application/model_metadata/model_catalog_provider.py": "0bc71743631813d0b97297cf6018973a1b987b2639605452b10701c0d4ed16e9",
        "substitute/infrastructure/persistence/sqlite_model_thumbnail_asset_store.py": "8f8e0f79e415c1bbcb4345617d1ee195cf33466e5df44768718f3e72c2ead1bb",
        "substitute/presentation/widgets/model_picker/model_picker_geometry.py": "6a2d62b1cb8821d0725e0e5fb7bbf2bdee3a1daf793d70185ff340b7f2567915",
        "substitute/infrastructure/persistence/model_metadata_repository.py": "2acbc78868642d7cba70ffc1b6bf203d6a09967e8ae7c5c6746d81571517ae48",
        "substitute/infrastructure/persistence/sqlite_model_metadata_store.py": "ae2c68f59b02b69580e28ee1a15718e9325d30facdadf1616d8938dc1c9ddc79",
        "substitute/application/model_metadata/metadata_enrichment.py": "5d222762e9a3d420f28d06bf101ba8db9562fee6a22c8fc123e7a18849cc2076",
    }
    assert {
        item["source_path"]: item["source_sha256"]
        for item in implementations
        if isinstance(item, dict) and item.get("source_path") in expected_freshness
    } == expected_freshness
    assert {
        "source_path": "sugarsubstitute_shared/model_acquisition/service.py",
        "source_sha256": "24585dff1d44e2d3f8eaed184c5855d4120f03e54ce56df2e3f82e1772128ce1",
        "adapted_paths": [
            "sugarcubes/model_library/infrastructure/artifact_destinations.py",
            "sugarcubes/model_library/infrastructure/artifact_files.py",
            "sugarcubes/model_library/infrastructure/artifact_downloads.py",
        ],
    } in implementations
    for implementation in implementations:
        assert isinstance(implementation, dict)
        assert implementation["source_path"].startswith(
            ("substitute/", "sugarsubstitute_shared/")
        )
        assert re.fullmatch(r"[0-9a-f]{64}", implementation["source_sha256"])
        paths: object = implementation["adapted_paths"]
        assert isinstance(paths, list) and paths
        for path in paths:
            assert isinstance(path, str) and path.startswith(
                ("sugarcubes/model_library/", "frontend/comfyui/ui/model_library/")
            )
            assert (ROOT / path).is_file()
    assert "SugarSubstitute Model Catalog and Suggestions" in (
        ROOT / "third_party/NOTICE.md"
    ).read_text("utf-8")


def test_browser_distribution_preserves_manifest_notice_and_upstream_notice() -> None:
    """Keep licensing available alongside the assets consumed by Comfy's browser."""
    for relative in (
        "third_party/manifest.toml",
        "third_party/NOTICE.md",
        "third_party/licenses/fluentui-system-icons.NOTICE.txt",
        "third_party/licenses/fluentui-emoji.LICENSE.txt",
    ):
        assert (ROOT / "web" / relative).read_bytes() == (ROOT / relative).read_bytes()
    notice = (ROOT / "third_party/NOTICE.md").read_text("utf-8")
    assert "Microsoft Fluent UI System Icons" in notice
    assert "Microsoft Fluent UI Emoji" in notice
    assert "MIT" in notice
    assert "Copyright (c) 2020 Microsoft Corporation" in notice


def test_discovery_provider_badges_have_exact_licensed_source_and_runtime_bytes() -> (
    None
):
    """Require reviewed provider identification assets and distributed attribution."""
    manifest = tomllib.loads((ROOT / "third_party/manifest.toml").read_text("utf-8"))
    components: object = manifest["component"]
    assert isinstance(components, list)
    expected = {
        "CivitAI Badge": (
            "Apache-2.0",
            "https://github.com/civitai/civitai",
            "e695de5aae91508df8a4465a86200d5cfa9c05f5",
            "b96eba49967d02ef5f6e59e7742cc8dcc6e22c8563e157a9c2686fa460ab3174",
        ),
        "OpenModelDB Favicon": (
            "GPL-3.0-only",
            "https://github.com/OpenModelDB/open-model-database",
            "782aac088bd83dd3fc438a42eb0c3868dc559110",
            "efe53844c597e16e2019aa2166ece248af277273e1c7444a56da4d60b363370a",
        ),
    }
    recorded: set[str] = set()
    for name, (license_id, source, revision, digest) in expected.items():
        matches = [
            item
            for item in components
            if isinstance(item, dict) and item.get("name") == name
        ]
        assert len(matches) == 1
        component = matches[0]
        assert isinstance(component, dict)
        assert (component["license"], component["source"], component["revision"]) == (
            license_id,
            source,
            revision,
        )
        license_path = Path(component["license_file"])
        assert (ROOT / "web" / license_path).read_bytes() == (
            ROOT / license_path
        ).read_bytes()
        assets: object = component["asset"]
        assert isinstance(assets, list) and len(assets) == 1
        asset = assets[0]
        assert isinstance(asset, dict)
        authored: object = asset["vendored_file"]
        assert isinstance(authored, str) and authored.startswith(
            "frontend/static/model-providers/"
        )
        assert asset["sha256"] == digest
        assert hashlib.sha256((ROOT / authored).read_bytes()).hexdigest() == digest
        assert asset["runtime_file"] == authored.replace("frontend/static/", "web/", 1)
        assert (ROOT / asset["runtime_file"]).read_bytes() == (
            ROOT / authored
        ).read_bytes()
        recorded.add(authored)
    assert recorded == {
        path.relative_to(ROOT).as_posix()
        for path in (ROOT / "frontend/static/model-providers").iterdir()
        if path.is_file()
    }


def test_official_fluent_menu_runtime_has_pinned_licensed_deployment() -> None:
    """Require the official menu bundle's pinned dependency, license and exact deployed bytes."""
    import json

    manifest = tomllib.loads((ROOT / "third_party/manifest.toml").read_text("utf-8"))
    components: object = manifest["component"]
    assert isinstance(components, list)
    matches = [
        item
        for item in components
        if isinstance(item, dict)
        and item.get("name") == "Microsoft Fluent Web Components menu runtime"
    ]
    assert len(matches) == 1
    component = matches[0]
    assert component["license"] == "MIT"
    assert component["source"] == "https://github.com/microsoft/fluentui"
    assert re.fullmatch(r"[0-9a-f]{40}", component["revision"])
    package = json.loads((ROOT / "package.json").read_text("utf-8"))
    assert package["dependencies"]["@fluentui/web-components"] == component["version"]
    lock = json.loads((ROOT / "package-lock.json").read_text("utf-8"))
    assert (
        lock["packages"]["node_modules/@fluentui/web-components"]["version"]
        == component["version"]
    )
    license_path = Path(component["license_file"])
    assert b"Microsoft Corporation" in (ROOT / license_path).read_bytes()
    assert (ROOT / "web" / license_path).read_bytes() == (
        ROOT / license_path
    ).read_bytes()
    assert component["runtime_files"] == ["web/comfyui/ui/menus/MenuControlRuntime.js"]
    assert (
        hashlib.sha256((ROOT / component["runtime_files"][0]).read_bytes()).hexdigest()
        == component["runtime_sha256"]
    )
