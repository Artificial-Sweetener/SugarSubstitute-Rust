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
/** Extract bounded SugarScript metadata from a PNG without decoding image pixels. */
const PNG_SIGNATURE = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]);
const MAX_IMAGE_BYTES = 128 * 1024 * 1024;
const MAX_SOURCE_BYTES = 1_000_000;
const MAX_WORKFLOW_BYTES = 32 * 1024 * 1024;
const MAX_CHUNKS = 10_000;
/** Identify bounded, user-actionable failures at the image adapter boundary. */
export class PngSugarScriptArtifactError extends Error {
    code;
    /** Retain a stable code without exposing image contents. */
    constructor(code, message) {
        super(message);
        this.name = 'PngSugarScriptArtifactError';
        this.code = code;
    }
}
/** Read supported PNG text chunks and retain only artifact authority indicators. */
export async function readPngSugarScriptMetadata(input) {
    if (!Number.isSafeInteger(input.size) || input.size < PNG_SIGNATURE.length) {
        throw artifactError('image.invalid_size', 'Image size is invalid.');
    }
    if (input.size > MAX_IMAGE_BYTES) {
        throw artifactError('image.too_large', 'SugarScript image exceeds 128 MB.');
    }
    const bytes = new Uint8Array(await input.arrayBuffer());
    if (bytes.byteLength !== input.size || !hasPngSignature(bytes)) {
        throw artifactError('image.invalid_png', 'Image is not a valid PNG artifact.');
    }
    let offset = PNG_SIGNATURE.length;
    let chunks = 0;
    let sawHeader = false;
    let sawEnd = false;
    let sugarScript = null;
    let workflow = null;
    while (offset < bytes.byteLength) {
        chunks += 1;
        if (chunks > MAX_CHUNKS) {
            throw artifactError('image.too_many_chunks', 'PNG contains too many chunks.');
        }
        const chunk = readChunk(bytes, offset);
        offset = chunk.nextOffset;
        if (!sawHeader) {
            if (chunk.type !== 'IHDR' || chunk.data.byteLength !== 13) {
                throw artifactError('image.invalid_png', 'PNG header chunk is invalid.');
            }
            sawHeader = true;
        }
        const text = readTextChunk(chunk.type, chunk.data);
        if (text?.keyword === 'workflow') {
            if (workflow !== null) {
                throw artifactError('image.duplicate_workflow', 'PNG contains more than one workflow record.');
            }
            if (text.encodedLength > MAX_WORKFLOW_BYTES) {
                throw artifactError('image.workflow_too_large', 'PNG workflow metadata exceeds 32 MB.');
            }
            workflow = parseWorkflow(text.value);
        }
        if (text?.keyword === 'sugar_script') {
            if (sugarScript !== null) {
                throw artifactError('image.duplicate_sugarscript', 'PNG contains more than one SugarScript record.');
            }
            if (text.encodedLength > MAX_SOURCE_BYTES) {
                throw artifactError('image.source_too_large', 'SugarScript source exceeds 1 MB.');
            }
            sugarScript = text.value;
        }
        if (chunk.type === 'IEND') {
            sawEnd = true;
            break;
        }
    }
    if (!sawEnd || offset !== bytes.byteLength) {
        throw artifactError('image.invalid_png', 'PNG chunk stream is incomplete or trailing.');
    }
    return { sugarScript, workflow };
}
/** Extract a script-only artifact and refuse to bypass workflow authority. */
export async function readScriptOnlySugarScript(input) {
    const metadata = await readPngSugarScriptMetadata(input);
    if (metadata.workflow !== null) {
        throw artifactError('image.workflow_authority_present', 'PNG contains workflow metadata; import it through the workflow authority path.');
    }
    if (metadata.sugarScript === null) {
        throw artifactError('image.sugarscript_missing', 'PNG contains no SugarScript metadata.');
    }
    return metadata.sugarScript;
}
/** Parse one workflow JSON object before it reaches either host or backend mutation. */
function parseWorkflow(value) {
    let parsed;
    try {
        parsed = JSON.parse(value);
    }
    catch {
        throw artifactError('image.invalid_workflow', 'PNG workflow metadata is not valid JSON.');
    }
    if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
        throw artifactError('image.invalid_workflow', 'PNG workflow metadata must be an object.');
    }
    return parsed;
}
/** Read one length-delimited chunk while rejecting integer and buffer overrun. */
function readChunk(bytes, offset) {
    if (bytes.byteLength - offset < 12) {
        throw artifactError('image.invalid_png', 'PNG chunk header is truncated.');
    }
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const length = view.getUint32(offset, false);
    const dataOffset = offset + 8;
    const nextOffset = dataOffset + length + 4;
    if (!Number.isSafeInteger(nextOffset) || nextOffset > bytes.byteLength) {
        throw artifactError('image.invalid_png', 'PNG chunk data is truncated.');
    }
    return {
        type: decodeAscii(bytes.subarray(offset + 4, dataOffset)),
        data: bytes.subarray(dataOffset, dataOffset + length),
        nextOffset,
    };
}
/** Decode supported uncompressed PNG text records without executing codecs. */
function readTextChunk(type, data) {
    if (type === 'tEXt')
        return readPlainText(data);
    if (type === 'iTXt')
        return readInternationalText(data);
    if (type === 'zTXt') {
        const separator = data.indexOf(0);
        const keyword = separator < 0 ? '' : decodeLatin1(data.subarray(0, separator));
        if (keyword === 'sugar_script' || keyword === 'workflow') {
            throw artifactError('image.compressed_metadata_unsupported', `Compressed PNG metadata '${keyword}' is unsupported.`);
        }
    }
    return null;
}
/** Decode a PNG tEXt record using its ISO-8859-1 byte contract. */
function readPlainText(data) {
    const separator = data.indexOf(0);
    if (separator < 1)
        return null;
    const valueBytes = data.subarray(separator + 1);
    return {
        keyword: decodeLatin1(data.subarray(0, separator)),
        value: decodeLatin1(valueBytes),
        encodedLength: valueBytes.byteLength,
    };
}
/** Decode an uncompressed PNG iTXt record and reject target compression explicitly. */
function readInternationalText(data) {
    const keywordEnd = data.indexOf(0);
    if (keywordEnd < 1 || keywordEnd + 2 >= data.byteLength)
        return null;
    const keyword = decodeLatin1(data.subarray(0, keywordEnd));
    const compressionFlag = data[keywordEnd + 1];
    const compressionMethod = data[keywordEnd + 2];
    let cursor = keywordEnd + 3;
    const languageEnd = data.indexOf(0, cursor);
    if (languageEnd < 0)
        return null;
    cursor = languageEnd + 1;
    const translatedEnd = data.indexOf(0, cursor);
    if (translatedEnd < 0)
        return null;
    cursor = translatedEnd + 1;
    if (compressionFlag !== 0 || compressionMethod !== 0) {
        if (keyword === 'sugar_script' || keyword === 'workflow') {
            throw artifactError('image.compressed_metadata_unsupported', `Compressed PNG metadata '${keyword}' is unsupported.`);
        }
        return null;
    }
    const valueBytes = data.subarray(cursor);
    let value;
    try {
        value = new TextDecoder('utf-8', { fatal: true }).decode(valueBytes);
    }
    catch {
        throw artifactError('image.invalid_utf8', `PNG metadata '${keyword}' is not valid UTF-8.`);
    }
    return { keyword, value, encodedLength: valueBytes.byteLength };
}
/** Compare the fixed PNG signature without accepting prefix-only data. */
function hasPngSignature(bytes) {
    return PNG_SIGNATURE.every((value, index) => bytes[index] === value);
}
/** Decode four-byte chunk types and PNG Latin-1 text without locale behavior. */
function decodeAscii(bytes) {
    return String.fromCharCode(...bytes);
}
/** Preserve each ISO-8859-1 code point exactly. */
function decodeLatin1(bytes) {
    let value = '';
    for (const byte of bytes)
        value += String.fromCharCode(byte);
    return value;
}
/** Construct a stable adapter error. */
function artifactError(code, message) {
    return new PngSugarScriptArtifactError(code, message);
}
