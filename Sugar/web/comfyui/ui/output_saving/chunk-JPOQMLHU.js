// integrations/shared/CanvasImagePolicy.ts
var MAX_CANVAS_IMAGE_BYTES = 64 * 1024 * 1024;
var MAX_CANVAS_IMAGE_PIXELS = 64 * 1024 * 1024;
function canvasPngDimensions(bytes) {
  const signature = [137, 80, 78, 71, 13, 10, 26, 10];
  if (bytes.length < 33 || bytes.length > MAX_CANVAS_IMAGE_BYTES || signature.some((byte, index) => bytes[index] !== byte) || bytes[12] !== 73 || bytes[13] !== 72 || bytes[14] !== 68 || bytes[15] !== 82)
    throw new Error("Canvas insertion requires a valid bounded PNG image.");
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const width = view.getUint32(16, false), height = view.getUint32(20, false);
  if (width < 1 || height < 1 || width > 3e4 || height > 3e4 || width * height > MAX_CANVAS_IMAGE_PIXELS)
    throw new Error("This image exceeds the canvas insertion pixel budget.");
  return Object.freeze({ width, height });
}

// frontend/comfyui/ui/output_saving/OutputImageDimensions.ts
function outputImageTransferDimensions(value) {
  return Number.isSafeInteger(value.width) && Number.isSafeInteger(value.height) && value.width >= 1 && value.height >= 1 && value.width <= 3e4 && value.height <= 3e4 && value.width * value.height <= MAX_CANVAS_IMAGE_PIXELS;
}
function outputImageDimensions(bytes, mime) {
  if (!bytes.length || bytes.length > MAX_CANVAS_IMAGE_BYTES)
    throw new Error("Output image exceeds its byte limit.");
  if (mime === "image/png") return canvasPngDimensions(bytes);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let width = 0, height = 0;
  if (mime === "image/jpeg") {
    if (bytes[0] !== 255 || bytes[1] !== 216) throw new Error("Output JPEG header is invalid.");
    let position = 2;
    for (let segments = 0; segments < 1e4 && position + 4 < bytes.length; segments++) {
      if (bytes[position++] !== 255) throw new Error("Output JPEG marker is invalid.");
      while (bytes[position] === 255) position++;
      const marker = bytes[position++];
      if (marker === void 0 || marker === 218 || marker === 217) break;
      if (marker === 1 || marker >= 208 && marker <= 216) continue;
      if (position + 2 > bytes.length) break;
      const length = view.getUint16(position, false);
      if (length < 2 || position + length > bytes.length) break;
      if ([192, 193, 194, 195, 197, 198, 199, 201, 202, 203, 205, 206, 207].includes(
        marker
      )) {
        if (length < 8) break;
        height = view.getUint16(position + 3, false);
        width = view.getUint16(position + 5, false);
        break;
      }
      position += length;
    }
  } else if (mime === "image/webp") {
    const text = (start, length) => String.fromCharCode(...bytes.subarray(start, start + length));
    if (bytes.length < 25 || text(0, 4) !== "RIFF" || text(8, 4) !== "WEBP" || view.getUint32(4, true) + 8 !== bytes.length)
      throw new Error("Output WebP header is invalid.");
    const kind = text(12, 4);
    if (kind === "VP8X" && bytes.length >= 30) {
      width = 1 + ((bytes[24] ?? 0) | (bytes[25] ?? 0) << 8 | (bytes[26] ?? 0) << 16);
      height = 1 + ((bytes[27] ?? 0) | (bytes[28] ?? 0) << 8 | (bytes[29] ?? 0) << 16);
    } else if (kind === "VP8 " && bytes.length >= 30 && text(23, 3) === "\x9D*") {
      width = view.getUint16(26, true) & 16383;
      height = view.getUint16(28, true) & 16383;
    } else if (kind === "VP8L" && bytes[20] === 47) {
      const bits = view.getUint32(21, true);
      width = (bits & 16383) + 1;
      height = (bits >>> 14 & 16383) + 1;
    }
  }
  if (!outputImageTransferDimensions({ width, height }))
    throw new Error("Output image exceeds its decoded pixel limit or has invalid dimensions.");
  return Object.freeze({ width, height });
}

export {
  MAX_CANVAS_IMAGE_BYTES,
  outputImageTransferDimensions,
  outputImageDimensions
};
