import {
  MAX_CANVAS_IMAGE_BYTES,
  outputImageTransferDimensions
} from "./chunk-JPOQMLHU.js";

// frontend/comfyui/ui/output_saving/BrowserImageRendition.ts
var BrowserImageRendition = class {
  /** Bind the current document without constructing native widgets or storing image pixels. */
  constructor(document) {
    this.document = document;
  }
  #active = 0;
  /** Encode caller-verified bounded headers; cancellation still observes bitmap/codec terminal return. */
  async encode(blob, mime, quality, dimensions, signal) {
    if (signal?.aborted) throw new DOMException("Cancelled", "AbortError");
    if (this.#active >= 2) throw new Error("Output image conversion is busy.");
    if (!["image/png", "image/jpeg", "image/webp"].includes(blob.type) || !blob.size || blob.size > MAX_CANVAS_IMAGE_BYTES || !outputImageTransferDimensions(dimensions) || !Number.isFinite(quality) || quality < 0.01 || quality > 1)
      throw new Error("Output image conversion exceeds its bounds.");
    const canvas = this.document.createElement("canvas");
    this.#active++;
    let bitmap;
    try {
      bitmap = await createImageBitmap(blob);
      if (signal?.aborted) throw new DOMException("Cancelled", "AbortError");
      if (!(bitmap.width === dimensions.width && bitmap.height === dimensions.height || bitmap.width === dimensions.height && bitmap.height === dimensions.width) || !outputImageTransferDimensions(bitmap))
        throw new Error("Decoded output geometry differs from its confirmed header.");
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const context = canvas.getContext("2d");
      if (!context) throw new Error("Output image encoding is unavailable.");
      context.drawImage(bitmap, 0, 0);
      const encoded = await new Promise(
        (resolve, reject) => canvas.toBlob(
          (value) => value ? resolve(value) : reject(new Error("Output image encoding failed.")),
          mime,
          quality
        )
      );
      if (signal?.aborted) throw new DOMException("Cancelled", "AbortError");
      if (encoded.type !== mime || !encoded.size || encoded.size > MAX_CANVAS_IMAGE_BYTES)
        throw new Error("Encoded output rendition is invalid.");
      return encoded;
    } finally {
      bitmap?.close();
      canvas.width = canvas.height = 0;
      this.#active--;
    }
  }
};
export {
  BrowserImageRendition
};
