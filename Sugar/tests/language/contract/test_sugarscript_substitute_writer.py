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
"""Lock the script text written for a project to the format SugarSubstitute persisted."""

from __future__ import annotations

import pytest

from sugarcubes.language import SugarScriptLanguageService
from sugarcubes.language.substitute_writer import (
    ScriptConnection,
    ScriptCube,
    ScriptField,
    ScriptFieldReference,
    ScriptNode,
    ScriptOverride,
    SubstituteScript,
    write_substitute_script,
)
from sugarcubes.language.syntax import (
    ConnectStatement,
    DisableStatement,
    SetStatement,
    UseStatement,
)


def _field(key: str, value: object, **options: object) -> ScriptField:
    """Describe one field whose script name is its key."""

    return ScriptField(key, key, value, **options)  # type: ignore[arg-type]


def test_cubes_values_hashes_and_connections_are_written_in_persisted_order() -> None:
    """Write `use` lines, one assignment block per Cube, then the connections."""

    script = SubstituteScript(
        cubes=(
            ScriptCube(
                "Text to Image",
                "Acme/Base/Text to Image.cube",
                "1.2.0",
                nodes=(
                    ScriptNode(
                        "KSampler",
                        "KSampler",
                        (_field("seed", 42), _field("steps", 20), _field("cfg", 7.0)),
                    ),
                    ScriptNode(
                        "Models",
                        "Models",
                        (
                            _field(
                                "ckpt_name", "demo.safetensors", model_sha256="ab" * 32
                            ),
                        ),
                    ),
                    # The negative prompt is authored first; positive prompts precede it.
                    ScriptNode(
                        "negative_prompt",
                        "negative prompt",
                        (_field("value", "blurry"),),
                    ),
                    ScriptNode(
                        "positive_prompt",
                        "positive prompt",
                        (
                            _field(
                                "value",
                                "a red cube <lora:ink:1.00>",
                                lora_sha256=(("ink", "cd" * 32), (" ", "ef" * 32)),
                            ),
                        ),
                    ),
                ),
            ),
            ScriptCube(
                "Upscale",
                "Acme/Base/Upscale.cube",
                "1.0.0",
                nodes=(
                    ScriptNode(
                        "Upscale",
                        "Upscale",
                        (_field("method", "bilinear"), _field("scale_factor", 1.5)),
                    ),
                ),
            ),
        ),
        connections=(
            ScriptConnection("Text to Image", "output.image", "Upscale", "input.image"),
        ),
    )

    assert write_substitute_script(script) == (
        'use "Acme/Base/Text to Image.cube"@1.2.0 as "Text to Image"\n'
        'use "Acme/Base/Upscale.cube"@1.0.0 as Upscale\n'
        "\n"
        'set "Text to Image".KSampler.seed = 42\n'
        'set "Text to Image".KSampler.steps = 20\n'
        'set "Text to Image".KSampler.cfg = 7.0\n'
        'set "Text to Image".Models.ckpt_name = "demo.safetensors"\n'
        f"# sha256 {'AB' * 32}\n"
        'set "Text to Image"."positive prompt".value = "a red cube <lora:ink:1.00>"\n'
        f'# lora_sha256 {{"name":"ink","sha256":"{"CD" * 32}"}}\n'
        'set "Text to Image"."negative prompt".value = "blurry"\n'
        "\n"
        'set Upscale.Upscale.method = "bilinear"\n'
        "set Upscale.Upscale.scale_factor = 1.5\n"
        "\n"
        'connect "Text to Image".output.image to Upscale.input.image\n'
    )


def test_a_bypassed_cube_persists_every_physical_line_behind_the_marker() -> None:
    """Mark the declaration, the activation and every line of a multi-line value."""

    script = SubstituteScript(
        cubes=(
            ScriptCube(
                "Text to Image",
                "Acme/Base/Text to Image.cube",
                "1.2.0",
                nodes=(ScriptNode("KSampler", "KSampler", (_field("steps", 20),)),),
            ),
            ScriptCube(
                "Detailer",
                "Acme/Base/Detailer.cube",
                "2.0.0",
                bypassed=True,
                nodes=(
                    ScriptNode(
                        "lora", "Load LoRA", (_field("strength", 1.0),), enabled=False
                    ),
                    ScriptNode("KSampler", "KSampler", (_field("denoise", 0.4),)),
                    ScriptNode(
                        "positive_prompt",
                        "positive prompt",
                        (_field("value", "line one\n\nline three"),),
                    ),
                ),
            ),
        )
    )

    assert write_substitute_script(script) == (
        'use "Acme/Base/Text to Image.cube"@1.2.0 as "Text to Image"\n'
        '# bypass use "Acme/Base/Detailer.cube"@2.0.0 as Detailer\n'
        "\n"
        '# bypass disable Detailer."Load LoRA"\n'
        "\n"
        'set "Text to Image".KSampler.steps = 20\n'
        "\n"
        "# bypass set Detailer.KSampler.denoise = 0.4\n"
        '# bypass set Detailer."positive prompt".value = """line one\n'
        "# bypass\n"
        '# bypass line three"""\n'
    )


def test_overrides_seed_modes_and_output_policy_precede_the_assignments() -> None:
    """Write each kind of workflow-wide state in its own section, in persisted order."""

    seed = _field("seed", 1, owned_by_override=True)
    script = SubstituteScript(
        cubes=(
            ScriptCube(
                "T2I",
                "Acme/Base/Text to Image.cube",
                "1.2.0",
                nodes=(
                    ScriptNode(
                        "KSampler",
                        "KSampler",
                        (seed, _field("steps", 30), _field("cfg", 7.0)),
                    ),
                ),
            ),
            ScriptCube(
                "Up",
                "Acme/Base/Upscale.cube",
                None,
                save_outputs=False,
                nodes=(ScriptNode("Upscale", "Upscale", (_field("scale_factor", 2),)),),
            ),
        ),
        overrides=(
            ScriptOverride("sampler_name", "sampler_name", "euler"),
            ScriptOverride("seed", "seed", 123456),
            ScriptOverride("steps", "steps", 30, full=False),
        ),
        override_selections=(("seed", True),),
        fixed_seed_fields=(ScriptFieldReference("T2I", "KSampler", "seed"),),
        fixed_seed_overrides=("seed",),
    )

    assert write_substitute_script(script) == (
        'use "Acme/Base/Text to Image.cube"@1.2.0 as T2I\n'
        'use "Acme/Base/Upscale.cube" as Up\n'
        "\n"
        '# cube_output_persistence {"alias":"Up","saved":false}\n'
        "\n"
        'set *.*.sampler_name = "euler"\n'
        "set *.*.seed = 123456\n"
        "\n"
        '# global_override_value {"key":"steps","mode":"global","value":30}\n'
        "\n"
        '# global_override_selection {"key":"seed","selected":true}\n'
        "\n"
        '# seed_control {"alias":"T2I","field":"seed","mode":"fixed","node":"KSampler"}\n'
        "\n"
        '# global_override_seed_control {"key":"seed","mode":"fixed"}\n'
        "\n"
        "set T2I.KSampler.steps = 30\n"
        "set T2I.KSampler.cfg = 7.0\n"
        "\n"
        "set Up.Upscale.scale_factor = 2\n"
    )


def test_links_activation_and_editor_state_use_their_persisted_forms() -> None:
    """Write whole-node and field links as references and activation by its difference."""

    script = SubstituteScript(
        cubes=(
            ScriptCube("A", "Acme/A.cube", "1.0.0"),
            ScriptCube(
                "B",
                "Acme/B.cube",
                "1.0.0",
                nodes=(
                    ScriptNode(
                        "prompt",
                        "Prompt",
                        (_field("value", "ignored by the link"),),
                        linked_to=("A", "Prompt"),
                        revealed=True,
                    ),
                    ScriptNode(
                        "sampler",
                        "Sampler",
                        (
                            _field(
                                "scheduler",
                                "karras",
                                reference=ScriptFieldReference(
                                    "A", "Sampler", "scheduler"
                                ),
                            ),
                            _field("links", {"node": "x"}),
                        ),
                        enabled=True,
                    ),
                    ScriptNode("extra", "Extra", (_field("on", True),), mode=4),
                    ScriptNode(
                        "woken", "Woken", (_field("on", True),), mode=4, enabled=True
                    ),
                ),
            ),
        )
    )

    assert write_substitute_script(script) == (
        'use "Acme/A.cube"@1.0.0 as A\n'
        'use "Acme/B.cube"@1.0.0 as B\n'
        "\n"
        '# node_revealed {"alias":"B","node":"prompt"}\n'
        "\n"
        '# node_enabled {"alias":"B","enabled":true,"node":"sampler"}\n'
        "\n"
        "enable B.Woken\n"
        "\n"
        "set B.Prompt = A.Prompt\n"
        'set B.Prompt.value = "ignored by the link"\n'
        "set B.Sampler.scheduler = A.Sampler.scheduler\n"
        "set B.Woken.on = True\n"
    )


def test_duplicate_aliases_and_unknown_connection_ends_are_refused() -> None:
    """Never write a script the reader could not attribute to its Cubes."""

    cube = ScriptCube("A", "Acme/A.cube", "1.0.0")
    with pytest.raises(ValueError, match="same alias"):
        write_substitute_script(SubstituteScript(cubes=(cube, cube)))
    with pytest.raises(ValueError, match="unknown Cube"):
        write_substitute_script(
            SubstituteScript(
                cubes=(cube,),
                connections=(ScriptConnection("A", "output.x", "B", "input.y"),),
            )
        )


def test_written_text_is_read_back_statement_for_statement_by_this_pack() -> None:
    """Every statement written, bypassed ones included, parses without a diagnostic."""

    script = SubstituteScript(
        cubes=(
            ScriptCube(
                "One",
                "Acme/One.cube",
                "1.0.0",
                nodes=(
                    ScriptNode(
                        "n", "Node", (_field("text", "a\nb"), _field("on", False))
                    ),
                ),
            ),
            ScriptCube(
                "Two",
                "Acme/Two.cube",
                "1.0.0",
                bypassed=True,
                nodes=(
                    ScriptNode(
                        "n",
                        "Node",
                        (_field("text", "c\n\nd", model_sha256="12" * 32),),
                    ),
                    ScriptNode("m", "Muted", (_field("x", 1),), enabled=False),
                ),
            ),
        ),
        connections=(ScriptConnection("One", "output.image", "Two", "input.image"),),
    )

    result = SugarScriptLanguageService().parse(write_substitute_script(script))

    assert result.diagnostics == ()
    kinds = [type(statement) for statement in result.document.statements]
    assert kinds == [
        UseStatement,
        UseStatement,
        DisableStatement,
        SetStatement,
        SetStatement,
        SetStatement,
        ConnectStatement,
    ]
    assert [statement.bypassed for statement in result.document.statements] == [
        False,
        True,
        True,
        False,
        False,
        True,
        False,
    ]
    assert [comment.extension_name for comment in result.document.comments] == [
        "sha256"
    ]
