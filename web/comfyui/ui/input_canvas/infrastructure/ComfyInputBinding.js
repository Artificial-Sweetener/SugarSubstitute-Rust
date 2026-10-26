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
/** Adapt native node persistence and widget commits to the shared mask binding use case. */
import { isSerializedWidget } from '../../graph/WidgetValueSerialization.js';
import { inputLabel } from '../domain/InputLoader.js';
import { inputLoaderSettings } from '../domain/InputLoaderSettings.js';
import { InputBindingController } from '../application/InputBindingController.js';
import { readInputBinding, withInputBinding } from './InputBindingMetadata.js';
import { observeInputLoaderWidgets } from './ComfyInputWidgetObserver.js';
/** Bind exact-node native history and serialization without taking ownership of editor pixel state. */
export function createComfyInputBinding(node, loader, ports) {
    const files = sourceFiles(node, loader.field);
    const initial = {
        version: 2,
        field: loader.field,
        files,
        entries: (files.length ? files : [null]).map((file, index) => ({
            id: crypto.randomUUID(),
            label: inputLabel(loader.kind) + ' ' + String(index + 1),
            source: { kind: 'file', file },
        })),
        selected: null,
    };
    const state = { ...initial, selected: initial.entries[0]?.id ?? null };
    return new InputBindingController(loader, {
        read: () => readInputBinding(node.properties, loader.kind, loader.field),
        load: async () => {
            if (node.inputs?.some((input) => input.name === loader.field && input.link !== null && input.link !== undefined))
                throw new Error('Disconnect the linked source field before selecting a canvas source.');
            const saved = readInputBinding(node.properties, loader.kind, loader.field) ??
                (loader.kind === 'mask' ? await ports.legacy.read(node.properties) : null);
            if (saved && saved.field !== loader.field)
                throw new Error('The saved source loader definition changed.');
            for (const entry of saved?.entries ?? [])
                if (entry.source.kind === 'file' &&
                    entry.source.file &&
                    !(await ports.verifyFile(entry.source.file)))
                    throw new Error('The selected input source file is unavailable.');
            return saved;
        },
        write: (value) => {
            const widget = node.widgets?.find((widget) => widget.name === loader.field);
            const canvas = value.entries.some((entry) => entry.source.kind === 'canvas');
            const files = canvas
                ? []
                : value.entries.flatMap((entry) => entry.source.kind === 'file' && entry.source.file ? [entry.source.file] : []);
            const normalized = { ...value, files };
            const nativeValue = loader.ordered ? files : (files[0] ?? '');
            const choices = widget?.options?.values;
            if (Array.isArray(choices))
                for (const file of files)
                    if (ports.verifiedFile(file) && !choices.includes(file))
                        choices.push(file);
            if (JSON.stringify(readInputBinding(node.properties, loader.kind, loader.field)) ===
                JSON.stringify(normalized) &&
                JSON.stringify(widget?.value) === JSON.stringify(nativeValue))
                return;
            const graph = node.graph;
            graph?.beforeChange?.();
            try {
                node.properties = withInputBinding(node.properties, loader.kind, normalized);
                if (widget) {
                    widget.value = nativeValue;
                }
            }
            finally {
                graph?.afterChange?.();
            }
        },
        files: () => sourceFiles(node, loader.field),
        settings: () => inputLoaderSettings(loader, Object.fromEntries((node.widgets ?? []).flatMap((widget) => widget.name &&
            isSerializedWidget(widget) &&
            !node.inputs?.some((input) => input.link !== null &&
                input.link !== undefined &&
                (input.widget?.name ?? input.name) === widget.name)
            ? [[widget.name, widget.value]]
            : []))),
        subscribe: (changed) => observeInputLoaderWidgets(node, loader, changed, ports.report),
    }, state, ports.host, ports.files, () => crypto.randomUUID(), ports.report, (sources) => ports.refreshPreviews(sources));
}
/** Validate dynamic native file values before they enter binding policy. */
function sourceFiles(node, field) {
    const value = node.widgets?.find((widget) => widget.name === field)?.value;
    if (value === undefined || value === null || value === '')
        return [];
    if (typeof value === 'string')
        return [value];
    if (!Array.isArray(value) || value.some((file) => typeof file !== 'string'))
        throw new Error('A source loader must choose a file or an ordered file list.');
    return value.filter((file) => typeof file === 'string' && Boolean(file));
}
