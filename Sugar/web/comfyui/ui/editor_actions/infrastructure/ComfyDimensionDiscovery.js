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
/** Capture Comfy's compiled graph through its existing preparation owner for explicit spatial discovery. */
import { isRecord } from '../../types/common.js';
/** Separate Comfy compilation and immutable graph admission from native sizing and field mutation. */
export class ComfyDimensionDiscovery {
    app;
    preparePrompt;
    client;
    /** Reuse the generation pipeline's relation and proximity preparation without submitting a generation. */
    constructor(app, preparePrompt, client) {
        this.app = app;
        this.preparePrompt = preparePrompt;
        this.client = client;
    }
    /** Compile only on explicit discovery, then transfer a detached prompt and exact owning field addresses. */
    async prepare(binding, cancellation) {
        if (binding.current?.() === false || binding.width.disabled() || binding.height.disabled())
            throw new Error('The dimension fields are unavailable.');
        const width = binding.width.targets?.() ?? [], height = binding.height.targets?.() ?? [];
        if (!width.length || !height.length)
            throw new Error('These dimension fields have no runnable workflow target.');
        if (!isRecord(this.app) || typeof this.app.graphToPrompt !== 'function')
            throw new Error('Comfy workflow compilation is unavailable.');
        const payload = await this.app.graphToPrompt();
        const prepared = await this.preparePrompt(payload);
        if (!isRecord(prepared) || !isRecord(prepared.output))
            throw new Error('Comfy did not return a compiled workflow.');
        if (binding.current?.() === false || cancellation?.cancelled())
            throw new Error('Scale discovery cancelled.');
        return await this.client.prepare(structuredClone(prepared.output), width, height, cancellation);
    }
}
