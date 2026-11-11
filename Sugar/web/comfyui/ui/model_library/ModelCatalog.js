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
/** Validate the shared model-library catalog before presenting backend values. */
import { isRecord } from '../types/common.js';
import { decodeModelTraits } from './ModelTraits.js';
import { decodeModelRelationships } from './ModelRelationships.js';
/** Reject unknown schemas and invalid image addresses at the browser boundary. */
export function decodeModelCatalog(value) {
    if (!isRecord(value) ||
        value.schemaVersion !== 1 ||
        !Array.isArray(value.models) ||
        !Array.isArray(value.fieldContracts))
        throw new Error('Unsupported model catalog response.');
    if (typeof value.revision !== 'number' ||
        !Number.isSafeInteger(value.revision) ||
        value.revision < 0)
        throw new Error('Invalid model catalog revision.');
    const scan = isRecord(value.scan) ? value.scan : {};
    return {
        revision: value.revision,
        relationships: decodeModelRelationships(value.relationships),
        items: value.models.map(decodeItem),
        scanning: scan.status === 'queued' || scan.status === 'running',
        fieldContracts: value.fieldContracts.map((contract) => {
            if (!isRecord(contract))
                throw new Error('Invalid model field contract.');
            return {
                classType: text(contract.classType),
                inputName: text(contract.inputName),
                kind: text(contract.kind),
            };
        }),
    };
}
/** Preserve literal text and exact Comfy choices instead of coercing dynamic data. */
function decodeItem(value) {
    if (!isRecord(value) || !Array.isArray(value.thumbnails) || !Array.isArray(value.trainedWords))
        throw new Error('Invalid model catalog item.');
    return {
        ...(value.detectorAsset === undefined ? {} : { detectorAsset: text(value.detectorAsset) }),
        id: text(value.id),
        kind: text(value.kind),
        value: text(value.value),
        rootId: text(value.rootId),
        relativePath: text(value.relativePath),
        folder: text(value.folder),
        title: text(value.title),
        version: text(value.version),
        baseModel: text(value.baseModel),
        versionCount: decodeVersionCount(value.versionCount),
        architecture: decodeArchitecture(value.architecture),
        traits: decodeModelTraits(value.traits),
        provider: text(value.provider),
        providerUrl: text(value.providerUrl),
        providerModelId: value.providerModelId === undefined ? '' : text(value.providerModelId),
        providerVersionId: value.providerVersionId === undefined ? '' : text(value.providerVersionId),
        providerLinks: decodeProviderLinks(value.providerLinks),
        description: text(value.description),
        trainedWords: value.trainedWords.map(text),
        tags: value.tags === undefined ? [] : decodeTags(value.tags),
        thumbnails: value.thumbnails.map(decodeThumbnail),
    };
}
/** Validate additive tensor evidence while preserving older public catalog payloads. */
function decodeArchitecture(value) {
    if (value === undefined || value === null)
        return null;
    if (!isRecord(value) || value.source !== 'comfy_tensor_shapes')
        throw new Error('Invalid model architecture evidence.');
    const base = text(value.base);
    const comfyClass = text(value.comfyClass);
    if (!base.trim() || !comfyClass.trim())
        throw new Error('Invalid model architecture label.');
    return { base, comfyClass, source: 'comfy_tensor_shapes' };
}
/** Preserve unknown history while rejecting malformed prepared version counts. */
function decodeVersionCount(value) {
    if (value === undefined || value === null)
        return null;
    if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0)
        throw new Error('Invalid model version count.');
    return value;
}
/** Accept older catalogs while validating additive family-discovery metadata. */
function decodeTags(value) {
    if (!Array.isArray(value))
        throw new Error('Invalid model tags.');
    return value.map(text);
}
/** Accept the additive public schema while rejecting malformed association arrays. */
function decodeProviderLinks(value) {
    if (value === undefined)
        return [];
    if (!Array.isArray(value) || value.length > 16)
        throw new Error('Invalid model provider links.');
    return value.map((item) => {
        if (!isRecord(item))
            throw new Error('Invalid model provider link.');
        return {
            providerId: text(item.providerId),
            providerName: text(item.providerName),
            modelId: text(item.modelId),
            versionId: text(item.versionId),
            modelPageUrl: text(item.modelPageUrl),
        };
    });
}
/** Admit only the library's immutable image endpoint and known tile geometry. */
function decodeThumbnail(value) {
    if (!isRecord(value) || (value.role !== 'standard' && value.role !== 'banner'))
        throw new Error('Invalid model thumbnail reference.');
    const assetId = text(value.assetId);
    const width = value.width;
    const height = value.height;
    if (!/^[a-f0-9]{64}$/.test(assetId) ||
        typeof width !== 'number' ||
        typeof height !== 'number' ||
        !Number.isSafeInteger(width) ||
        !Number.isSafeInteger(height) ||
        width <= 0 ||
        height <= 0)
        throw new Error('Invalid model thumbnail dimensions or identity.');
    const url = `/sugarcubes/v1/models/assets/${assetId}`;
    if (value.url !== url)
        throw new Error('Invalid model thumbnail URL.');
    return { assetId, role: value.role, width, height, url };
}
/** Narrow untrusted labels without treating their contents as markup. */
function text(value) {
    if (typeof value !== 'string')
        throw new Error('Expected model catalog text.');
    return value;
}
/** Pick a prepared image large enough for its destination without decoding originals. */
export function chooseModelThumbnail(item, role, size) {
    const candidates = item.thumbnails
        .filter((image) => image.role === role)
        .sort((left, right) => Math.max(left.width, left.height) - Math.max(right.width, right.height));
    return (candidates.find((image) => Math.max(image.width, image.height) >= size) ??
        candidates.at(-1) ??
        null);
}
