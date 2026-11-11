import {
  isSessionFile,
  sessionDigest,
  sessionFile
} from "./chunk-NKU7LGRE.js";

// frontend/comfyui/ui/types/common.ts
function isRecord(value) {
  return typeof value === "object" && value !== null;
}

// frontend/comfyui/ui/output_saving/OutputArtifactClient.ts
var unavailableCodes = /* @__PURE__ */ new Set([
  "missing",
  "changed",
  "unsupported",
  "inaccessible",
  "invalid_reference",
  "budget",
  "cancelled",
  "ambiguous"
]);
var OutputArtifactClient = class {
  /** Bind an explicit current-origin transport; descriptor values never select a backend. */
  constructor(fetch) {
    this.fetch = fetch;
  }
  /** Confirm a live final's encoded identity before a host checkpoint can claim durability. */
  async receipt(file, signal) {
    const result = await this.#request("artifact-receipt", { file }, signal);
    if (result.available && (result.receipt.file.filename !== file.filename || result.receipt.file.subfolder !== file.subfolder.replaceAll("\\", "/") || result.receipt.file.storage !== file.type))
      throw new Error("Confirmed output differs from its requested descriptor.");
    return result;
  }
  /** Re-resolve only a recorded artifact; source/run ownership remains the session domain's concern. */
  async resolve(output, signal) {
    const { file, digest, bytes, width, height, media, mime, metadataEmbedded, promptId } = output;
    const result = await this.#request(
      "artifact-resolve",
      {
        expected: { file, digest, bytes, width, height, media, mime, metadataEmbedded, promptId }
      },
      signal
    );
    if (result.available) {
      const actual = result.receipt;
      if (actual.digest !== digest || actual.bytes !== bytes || actual.width !== width || actual.height !== height || actual.media !== media || actual.mime !== mime || actual.metadataEmbedded !== metadataEmbedded)
        throw new Error("Resolved output differs from its recorded identity.");
    }
    return result;
  }
  /** Bound response allocation and lifetime while keeping per-file refusal an ordinary typed result. */
  async #request(action, body, signal) {
    const controller = new AbortController();
    const abort = () => controller.abort();
    if (signal?.aborted) abort();
    signal?.addEventListener("abort", abort, { once: true });
    const timer = setTimeout(abort, 2e4);
    try {
      const response = await this.fetch("/sugarcubes/output-saving/" + action, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
        signal: controller.signal
      });
      const encoded = await boundedResponse(response, 16384);
      const value = JSON.parse(encoded);
      const result = decodeArtifactResult(value);
      if (!response.ok && result.available) throw new Error("Invalid output artifact response.");
      return result;
    } finally {
      clearTimeout(timer);
      signal?.removeEventListener("abort", abort);
    }
  }
};
function decodeArtifactResult(value) {
  if (!isRecord(value)) throw new Error("Invalid output artifact response.");
  if (value.available === false && keys(value, ["available", "code", "message"]) && typeof value.code === "string" && unavailableCodes.has(value.code) && typeof value.message === "string" && value.message.length <= 2048) {
    return Object.freeze({
      available: false,
      code: value.code,
      message: value.message
    });
  }
  if (value.available !== true || !keys(value, ["available", "receipt"]) || !isRecord(value.receipt))
    throw new Error("Invalid output artifact response.");
  const receipt = value.receipt;
  const { file, digest, bytes, width, height, media, mime, metadataEmbedded, animated } = receipt;
  if (!keys(receipt, [
    "file",
    "digest",
    "bytes",
    "width",
    "height",
    "media",
    "mime",
    "metadataEmbedded",
    "animated"
  ]) || !isSessionFile(file) || !sessionFile(file) || !managedFile(file) || typeof digest !== "string" || !sessionDigest(digest) || !integer(bytes, 8589934592) || !integer(width, 131072) || !integer(height, 131072) || media !== "image" && media !== "video" || !["image/png", "image/jpeg", "image/webp", "image/gif", "video/mp4", "video/webm"].includes(
    String(mime)
  ) || typeof metadataEmbedded !== "boolean" || typeof animated !== "boolean" || typeof mime !== "string" || mime.startsWith("video/") !== (media === "video") || media === "video" && animated || mime === "image/gif" && !animated || mime === "image/jpeg" && animated || !matchingMime(file.filename, mime))
    throw new Error("Invalid output artifact response.");
  return Object.freeze({
    available: true,
    receipt: Object.freeze({
      file: Object.freeze({ ...file }),
      digest,
      bytes,
      width,
      height,
      media,
      mime,
      metadataEmbedded,
      animated
    })
  });
}
function matchingMime(filename, mime) {
  const suffix = filename.split(".").at(-1)?.toLowerCase();
  return mime === "image/png" && (suffix === "png" || suffix === "apng") || mime === "image/jpeg" && (suffix === "jpg" || suffix === "jpeg") || mime === "image/webp" && suffix === "webp" || mime === "image/gif" && suffix === "gif" || mime === "video/mp4" && suffix === "mp4" || mime === "video/webm" && suffix === "webm";
}
function managedFile(file) {
  const prefix = "__sugarcubes_managed__/";
  if (!file.subfolder.startsWith(prefix)) return file.managedArtifact === null;
  const identity = file.subfolder.slice(prefix.length);
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(identity) && identity !== "00000000-0000-0000-0000-000000000000" && file.managedArtifact === identity;
}
function keys(value, names) {
  return Object.keys(value).length === names.length && names.every((name) => Object.hasOwn(value, name));
}
function integer(value, maximum) {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 1 && value <= maximum;
}
async function boundedResponse(response, maximum) {
  if (!response.body) throw new Error("Output artifact response is empty.");
  const reader = response.body.getReader();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let total = 0;
  let text = "";
  try {
    for (; ; ) {
      const next = await reader.read();
      if (next.done) break;
      total += next.value.byteLength;
      if (total > maximum) throw new Error("Output artifact response exceeds its byte limit.");
      text += decoder.decode(next.value, { stream: true });
    }
    return text + decoder.decode();
  } finally {
    try {
      await reader.cancel();
    } finally {
      reader.releaseLock();
    }
  }
}
export {
  OutputArtifactClient,
  decodeArtifactResult
};
