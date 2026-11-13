import {
  isRecord,
  isSessionOutput,
  sessionDigest,
  sessionFile,
  sessionUuid
} from "./chunk-NKU7LGRE.js";
import {
  MAX_CANVAS_IMAGE_BYTES,
  outputImageDimensions,
  outputImageTransferDimensions
} from "./chunk-JPOQMLHU.js";

// integrations/shared/SessionValidation.generated.ts
function isSessionBinding(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "entryId") && typeof value["entryId"] === "string" && value["entryId"].length <= 36 && Object.hasOwn(value, "runtimeSession") && typeof value["runtimeSession"] === "string" && value["runtimeSession"].length <= 256 && Object.hasOwn(value, "generation") && typeof value["generation"] === "number" && Number.isSafeInteger(value["generation"]) && value["generation"] >= 0 && value["generation"] <= 9007199254740991 && Object.hasOwn(value, "graphRevision") && typeof value["graphRevision"] === "number" && Number.isSafeInteger(value["graphRevision"]) && value["graphRevision"] >= 0 && value["graphRevision"] <= 9007199254740991 && Object.hasOwn(value, "sourceRevision") && typeof value["sourceRevision"] === "number" && Number.isSafeInteger(value["sourceRevision"]) && value["sourceRevision"] >= 0 && value["sourceRevision"] <= 9007199254740991 && Object.hasOwn(value, "navigationRevision") && typeof value["navigationRevision"] === "number" && Number.isSafeInteger(value["navigationRevision"]) && value["navigationRevision"] >= 0 && value["navigationRevision"] <= 9007199254740991;
}

// integrations/shared/SessionSnapshot.ts
function sessionOutput(value) {
  return [value.id, value.entryId, value.backend, value.runId, value.sourceId].every(sessionUuid) && sessionDigest(value.digest) && (value.assetDigest === null || sessionDigest(value.assetDigest)) && (value.promptId === null || value.promptId.length > 0) && sessionFile(value.file) && (value.file.storage !== "temp" || value.assetDigest === value.digest) && (value.assetDigest === null || value.assetDigest === value.digest) && (value.scene === null || [value.scene.groupId, value.scene.sceneId].every(sessionUuid) && value.scene.order < value.scene.count);
}

// frontend/comfyui/ui/output_saving/BrowserRetainedOutputTransfer.ts
function retainedOutputCapabilities(source) {
  const bounded = source.output.bytes > 0 && source.output.bytes <= MAX_CANVAS_IMAGE_BYTES;
  const image = source.output.media === "image" && ["image/png", "image/jpeg", "image/webp"].includes(source.output.mime) && outputImageTransferDimensions(source.output);
  return Object.freeze({
    copy: bounded && image,
    drag: bounded && image,
    download: bounded,
    reveal: false,
    photoshop: false
  });
}
var BrowserRetainedOutputTransfer = class {
  /** Bind the current frontend origin, HTTP boundary and interoperable SHA-256 implementation. */
  constructor(origin, fetch, digest = sha256) {
    this.origin = origin;
    this.fetch = fetch;
    this.digest = digest;
  }
  #pending = /* @__PURE__ */ new Set();
  #disposed = false;
  /** Read one exact original for supported image clipboard and drag operations. */
  async image(file, source, signal) {
    if (!retainedOutputCapabilities(source).copy)
      throw new Error("This retained output exceeds the supported image transfer bounds.");
    const url = this.#url(file, source);
    const mime = source.output.mime;
    if (mime !== "image/png" && mime !== "image/jpeg" && mime !== "image/webp")
      throw new Error("This retained output is not a supported clipboard image.");
    return this.#request(signal, async (controller) => {
      const response = await this.fetch(url, { signal: controller.signal });
      this.#headers(response, source);
      if (!response.body) throw new Error("The retained output response is empty.");
      const reader = response.body.getReader();
      const data = new Uint8Array(source.output.bytes);
      let offset = 0;
      try {
        for (; ; ) {
          if (controller.signal.aborted) throw new DOMException("Cancelled", "AbortError");
          const next = await reader.read();
          if (next.done) break;
          if (offset + next.value.byteLength > data.byteLength)
            throw new Error("The retained output exceeded its confirmed length.");
          data.set(next.value, offset);
          offset += next.value.byteLength;
        }
      } finally {
        try {
          await reader.cancel();
        } finally {
          reader.releaseLock();
        }
      }
      if (offset !== data.byteLength || await this.digest(data) !== source.output.digest)
        throw new Error("The retained output differs from its confirmed identity.");
      if (controller.signal.aborted || this.#disposed)
        throw new DOMException("Cancelled", "AbortError");
      const dimensions = outputImageDimensions(data, mime);
      if (dimensions.width !== source.output.width || dimensions.height !== source.output.height)
        throw new Error("The retained output geometry differs from its confirmed identity.");
      return {
        blob: new Blob([data], { type: mime }),
        filename: source.output.file.filename,
        mimeType: mime,
        dimensions,
        url
      };
    });
  }
  /** Validate exact native metadata before delegating a bounded original download to the browser. */
  async download(file, source, signal) {
    if (!retainedOutputCapabilities(source).download)
      throw new Error("This retained output exceeds the supported direct download limit.");
    const url = this.#url(file, source);
    return this.#request(signal, async (controller) => {
      const response = await this.fetch(url, { method: "HEAD", signal: controller.signal });
      this.#headers(response, source);
      return url;
    });
  }
  /** Revoke pending reads and refuse all later publication after the owning canvas is retired. */
  dispose() {
    this.#disposed = true;
    for (const controller of this.#pending) controller.abort();
  }
  /** Validate native URL correlation; the host separately checks the actual capability and current custody. */
  #url(file, source) {
    if (!isSessionOutput(source.output) || !sessionOutput(source.output) || !isSessionBinding(source.binding) || source.output.assetDigest !== source.output.digest || source.binding.entryId !== source.output.entryId || file.filename !== source.output.file.filename || file.subfolder !== source.output.file.subfolder || file.type !== source.output.file.storage)
      throw new Error("The retained output source is invalid.");
    const url = new URL(source.url, this.origin);
    const expected = new URL(this.origin);
    const path = /^\/__sugarsubstitute\/session-media\/v1\/[0-9a-f]{32}\/([0-9a-f-]{36})$/.exec(
      url.pathname
    );
    if (url.origin !== expected.origin || !["http:", "https:"].includes(url.protocol) || url.username || url.password || url.hash || url.href.length > 4096 || path?.[1] !== source.output.id || [...url.searchParams.keys()].length !== 1 || !url.searchParams.has("binding"))
      throw new Error("The retained output URL is outside its current host scope.");
    let binding;
    try {
      binding = JSON.parse(url.searchParams.get("binding") ?? "");
    } catch {
      throw new Error("The retained output binding is invalid.");
    }
    if (!isSessionBinding(binding) || ![
      "entryId",
      "runtimeSession",
      "generation",
      "graphRevision",
      "sourceRevision",
      "navigationRevision"
    ].every((key) => binding[key] === source.binding[key]))
      throw new Error("The retained output binding differs from its current source.");
    return url.href;
  }
  /** Refuse partial or mismatched original metadata before any byte or clipboard publication. */
  #headers(response, source) {
    if (!response.ok || response.status !== 200 || response.headers.get("Content-Type") !== source.output.mime || response.headers.get("Content-Length") !== String(source.output.bytes) || response.headers.has("Content-Range"))
      throw new Error("The retained output response does not match its confirmed original.");
  }
  /** Admit two active bounded reads; cancellation retains each operation through its observed terminal outcome. */
  async #request(signal, operation) {
    if (this.#disposed || signal?.aborted) throw new DOMException("Cancelled", "AbortError");
    if (this.#pending.size >= 2) throw new Error("Retained output transfers are busy.");
    const controller = new AbortController();
    const abort = () => controller.abort();
    signal?.addEventListener("abort", abort, { once: true });
    this.#pending.add(controller);
    const timeout = setTimeout(abort, 2e4);
    try {
      return await operation(controller);
    } finally {
      clearTimeout(timeout);
      signal?.removeEventListener("abort", abort);
      this.#pending.delete(controller);
    }
  }
};
async function sha256(bytes) {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)].map((value) => value.toString(16).padStart(2, "0")).join("");
}
export {
  BrowserRetainedOutputTransfer,
  retainedOutputCapabilities
};
