//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Recognize the core inpaint encoder's spatial crop through an unchanged latent round trip. */
import { isRecord } from '../../types/common.js';
/** Admit only concrete compiled socket links, never literal array widget values. */
function readLink(value) {
    return Array.isArray(value) &&
        value.length === 2 &&
        typeof value[0] === 'string' &&
        typeof value[1] === 'number' &&
        Number.isSafeInteger(value[1]) &&
        value[1] >= 0
        ? { node: value[0], slot: value[1] }
        : null;
}
/** Preserve strict geometry checks outside a verified same-VAE encode/sample/decode chain. */
export function hasCompiledVaeCenterCrop(prompt, output, image) {
    if (!isRecord(prompt))
        return false;
    const visited = new Set();
    let edge = { node: output, slot: 0 };
    let decoderVae = null;
    while (edge && visited.size < 256) {
        if (visited.has(edge.node))
            return false;
        visited.add(edge.node);
        const node = prompt[edge.node];
        if (!isRecord(node) || !isRecord(node.inputs))
            return false;
        const values = node.inputs;
        if (!decoderVae) {
            if (node.class_type === 'SugarCubes.CubeOutput')
                edge = readLink(values.value);
            else if (node.class_type === 'SaveImage' || node.class_type === 'PreviewImage')
                edge = readLink(values.images);
            else if ((node.class_type === 'VAEDecode' || node.class_type === 'VAEDecodeTiled') &&
                edge.slot === 0) {
                decoderVae = readLink(values.vae);
                if (!decoderVae)
                    return false;
                edge = readLink(values.samples);
            }
            else
                return false;
            continue;
        }
        if (edge.slot === 0 &&
            [
                'KSampler',
                'KSamplerAdvanced',
                'SimpleSyrup.KSampler',
                'SimpleSyrup.KSamplerExtras',
            ].includes(String(node.class_type))) {
            edge = readLink(values.latent_image);
            continue;
        }
        if (node.class_type !== 'InpaintModelConditioning' || edge.slot !== 2)
            return false;
        const encoderVae = readLink(values.vae), source = readLink(values.pixels);
        if (!encoderVae ||
            encoderVae.node !== decoderVae.node ||
            encoderVae.slot !== decoderVae.slot ||
            !source ||
            source.slot !== 0)
            return false;
        const loader = prompt[source.node];
        if (!isRecord(loader) ||
            !isRecord(loader.inputs) ||
            loader.class_type !== image.classType ||
            image.files.length !== 1 ||
            loader.inputs[image.field] !== image.files[0])
            return false;
        return true;
    }
    return false;
}
