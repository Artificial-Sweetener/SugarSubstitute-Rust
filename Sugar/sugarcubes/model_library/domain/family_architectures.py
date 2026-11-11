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
"""Bridge explicit provider ecosystems to core Comfy tensor architectures conservatively."""

FAMILY_ARCHITECTURES: dict[str, str] = {
    "SD1": "SD1.5",
    "SD2": "SD2",
    "SD3": "SD3",
    "SD35M": "SD3",
    "SDXL": "SDXL",
    "SDXLDistilled": "SDXL",
    "PlaygroundV2": "SDXL",
    "Flux1": "FLUX",
    "FluxKrea": "FLUX",
    "Flux1Kontext": "FLUX",
    "Flux2": "FLUX.2",
    "Chroma": "Chroma",
    "Anima": "Anima",
    "AuraFlow": "AuraFlow",
    "PixArtA": "PixArtAlpha",
    "PixArtE": "PixArtSigma",
    "Mochi": "Mochi",
    "HiDream": "HiDream",
    "Kolors": "Kolors",
    "HyDit1": "HunyuanDiT",
    "SCascade": "Stable_Cascade_C",
    "Qwen": "QwenImage",
    "Lumina": "Lumina2",
    "HyV1": "HunyuanVideo",
    "SVD": "SVD_img2vid",
    "CogVideoX": "CogVideo",
    "LTXV": "LTXV",
    "LTXV2": "LTXV2",
    "LTXV23": "LTXV2",
    "WanVideo1_3B_T2V": "WAN21_T2V",
    "WanVideo14B_T2V": "WAN21_T2V",
    "WanVideo14B_I2V_480p": "WAN21_I2V",
    "WanVideo14B_I2V_720p": "WAN21_I2V",
    "WanVideo22_TI2V_5B": "WAN22_TI2V",
    "WanVideo22_I2V_A14B": "WAN22_I2V",
    "WanVideo22_T2V_A14B": "WAN22_T2V",
    "Krea2": "Krea2",
}
