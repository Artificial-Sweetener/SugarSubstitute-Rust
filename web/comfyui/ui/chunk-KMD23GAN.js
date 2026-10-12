import {
  defineComponent,
  h,
  watch
} from "./chunk-LFPAN2S3.js";

// frontend/comfyui/ui/core/CubeFallbackIconRenderer.ts
var FALLBACK_ICON_STYLE = Object.freeze({
  fontFamily: "Segoe UI",
  fontWeight: 700,
  inset: 2,
  renderSize: 96
});
var FALLBACK_ICON_FONT_SCALE = 62 / FALLBACK_ICON_STYLE.renderSize;
function fallbackStyle(model) {
  const source = model?.fallback ?? {};
  const renderSize = Number(source.renderSize);
  return {
    fontFamily: typeof source.fontFamily === "string" && source.fontFamily.trim() ? source.fontFamily.trim() : FALLBACK_ICON_STYLE.fontFamily,
    fontWeight: typeof source.fontWeight === "number" && Number.isFinite(source.fontWeight) ? source.fontWeight : FALLBACK_ICON_STYLE.fontWeight,
    inset: typeof source.inset === "number" && Number.isFinite(source.inset) ? source.inset : FALLBACK_ICON_STYLE.inset,
    renderSize: Number.isFinite(renderSize) && renderSize > 0 ? renderSize : FALLBACK_ICON_STYLE.renderSize
  };
}
function resolveFallbackIconFont(model, pixelSize) {
  const style = fallbackStyle(model);
  const size = Math.max(1, Math.round(pixelSize));
  return `${style.fontWeight} ${size}px "${style.fontFamily}", sans-serif`;
}
function measureFallbackText(ctx, model, initials, pixelSize) {
  ctx.font = resolveFallbackIconFont(model, pixelSize);
  if (typeof ctx.measureText !== "function") {
    return {
      width: 0,
      height: pixelSize,
      ascent: pixelSize * 0.78,
      descent: pixelSize * 0.22,
      precise: false
    };
  }
  const metrics = ctx.measureText(initials);
  const ascent = Number(metrics.actualBoundingBoxAscent);
  const descent = Number(metrics.actualBoundingBoxDescent);
  const left = Number(metrics.actualBoundingBoxLeft);
  const right = Number(metrics.actualBoundingBoxRight);
  const width = Number(metrics.width) || 0;
  if (Number.isFinite(ascent) && Number.isFinite(descent) && ascent + descent > 0) {
    const measuredWidth = Number.isFinite(left) && Number.isFinite(right) && left + right > 0 ? left + right : width;
    return {
      width: measuredWidth,
      height: ascent + descent,
      ascent,
      descent,
      x: Number.isFinite(left) ? -left : 0,
      y: -ascent,
      precise: true
    };
  }
  return {
    width,
    height: pixelSize,
    ascent: pixelSize * 0.78,
    descent: pixelSize * 0.22,
    x: 0,
    y: pixelSize * -0.78,
    precise: false
  };
}
function fallbackInitialsSourceLayout(ctx, initials, model = {}) {
  const style = fallbackStyle(model);
  const sourceSize = Math.max(1, style.renderSize);
  const inset = Math.max(0, Math.min(sourceSize / 2, style.inset));
  const textSize = Math.max(0, sourceSize - inset * 2);
  const fontSize = Math.max(10, sourceSize * FALLBACK_ICON_FONT_SCALE);
  const bounds = measureFallbackText(ctx, model, initials, fontSize);
  if (!textSize || bounds.width <= 0 || bounds.height <= 0) {
    return {
      bounds,
      fontSize,
      offsetX: inset,
      offsetY: inset,
      scale: 1,
      sourceSize
    };
  }
  const scale = Math.min(1, textSize / bounds.width, textSize / bounds.height);
  return {
    bounds,
    fontSize,
    offsetX: inset + (textSize - bounds.width * scale) / 2,
    offsetY: inset + (textSize - bounds.height * scale) / 2,
    scale,
    sourceSize
  };
}
function drawFallbackInitialsCanvas(ctx, model, x, y, size, options = {}) {
  const initials = typeof model?.initials === "string" && model.initials ? model.initials : "?";
  if (!ctx || typeof ctx.fillText !== "function" || typeof ctx.save !== "function" || typeof ctx.restore !== "function" || typeof ctx.translate !== "function" || typeof ctx.scale !== "function") {
    return;
  }
  ctx.save();
  try {
    const layout = fallbackInitialsSourceLayout(ctx, initials, model ?? {});
    const targetScale = size / layout.sourceSize;
    ctx.translate(x, y);
    ctx.scale(targetScale, targetScale);
    ctx.translate(layout.offsetX, layout.offsetY);
    ctx.scale(layout.scale, layout.scale);
    ctx.translate(-(layout.bounds.x ?? 0), -(layout.bounds.y ?? 0));
    ctx.font = resolveFallbackIconFont(model, layout.fontSize);
    ctx.fillStyle = typeof options.color === "string" ? options.color : "#ffffff";
    ctx.textAlign = "left";
    ctx.textBaseline = "alphabetic";
    ctx.fillText(initials, 0, 0);
  } finally {
    ctx.restore();
  }
}
function resolveDevicePixelRatio() {
  const ratio = typeof globalThis !== "undefined" ? Number(globalThis.devicePixelRatio || 1) : 1;
  return Number.isFinite(ratio) && ratio > 0 ? ratio : 1;
}
function drawFallbackInitialsElementCanvas(canvas, model) {
  if (!canvas || typeof canvas.getContext !== "function") {
    return;
  }
  const sourceSize = fallbackStyle(model).renderSize;
  const ratio = resolveDevicePixelRatio();
  canvas.width = Math.round(sourceSize * ratio);
  canvas.height = Math.round(sourceSize * ratio);
  canvas.style.width = "100%";
  canvas.style.height = "100%";
  let ctx = null;
  try {
    ctx = canvas.getContext("2d");
  } catch (_error) {
    return;
  }
  if (!ctx) {
    return;
  }
  if (typeof ctx.clearRect === "function") {
    ctx.clearRect(0, 0, canvas.width, canvas.height);
  }
  ctx.save();
  if (typeof ctx.scale === "function") {
    ctx.scale(ratio, ratio);
  }
  drawFallbackInitialsCanvas(ctx, model, 0, 0, sourceSize);
  ctx.restore();
}

// frontend/comfyui/ui/core/CubeIconToneProfile.ts
var NEUTRAL_SPREAD_LIMIT = 18;
var NEUTRAL_CONFIDENCE = 0.88;
var VISIBLE_ALPHA = 16;
function classifyCubeIconTone(pixels) {
  if (pixels.length % 4 !== 0)
    throw new TypeError("Icon pixels must contain complete RGBA samples.");
  let visible = 0;
  let neutral = 0;
  let luminance = 0;
  for (let index = 0; index < pixels.length; index += 4) {
    const red = pixels[index] ?? 0;
    const green = pixels[index + 1] ?? 0;
    const blue = pixels[index + 2] ?? 0;
    if ((pixels[index + 3] ?? 0) < VISIBLE_ALPHA) continue;
    visible += 1;
    if (Math.max(red, green, blue) - Math.min(red, green, blue) <= NEUTRAL_SPREAD_LIMIT)
      neutral += 1;
    luminance += red * 0.2126 + green * 0.7152 + blue * 0.0722;
  }
  if (visible === 0 || neutral / visible < NEUTRAL_CONFIDENCE) return "original";
  const mean = luminance / visible;
  return mean >= 170 ? "bright" : mean <= 85 ? "dark" : "original";
}
function invertNeutralCubeIconPixels(pixels) {
  if (pixels.length % 4 !== 0)
    throw new TypeError("Icon pixels must contain complete RGBA samples.");
  const adjusted = new Uint8ClampedArray(pixels);
  for (let index = 0; index < pixels.length; index += 4) {
    const red = pixels[index] ?? 0;
    const green = pixels[index + 1] ?? 0;
    const blue = pixels[index + 2] ?? 0;
    if ((pixels[index + 3] ?? 0) === 0 || Math.max(red, green, blue) - Math.min(red, green, blue) > NEUTRAL_SPREAD_LIMIT)
      continue;
    adjusted[index] = 255 - red;
    adjusted[index + 1] = 255 - green;
    adjusted[index + 2] = 255 - blue;
  }
  return adjusted;
}

// frontend/comfyui/ui/core/CubeIconAssetContrast.ts
var SAMPLE_SIZE = 32;
var RENDER_SIZE = 192;
var contrastImages = /* @__PURE__ */ new WeakMap();
function resolveCubeIconContrastImage(image) {
  const retained = contrastImages.get(image);
  if (retained) return retained;
  if (image.naturalWidth <= 0 || image.naturalHeight <= 0) return null;
  try {
    const sample = drawIcon(image.ownerDocument, image, SAMPLE_SIZE);
    if (!sample) return null;
    const tone = classifyCubeIconTone(
      sample.context.getImageData(0, 0, SAMPLE_SIZE, SAMPLE_SIZE).data
    );
    const variant = tone === "original" ? null : drawIcon(image.ownerDocument, image, RENDER_SIZE);
    if (tone !== "original" && !variant) return null;
    if (variant) {
      const pixels = variant.context.getImageData(0, 0, RENDER_SIZE, RENDER_SIZE);
      pixels.data.set(invertNeutralCubeIconPixels(pixels.data));
      variant.context.putImageData(pixels, 0, 0);
    }
    const result = { tone, canvas: variant?.canvas ?? null };
    contrastImages.set(image, result);
    return result;
  } catch (error) {
    console.warn("SugarCubes retained an icon whose contrast could not be sampled.", { error });
    const original = { tone: "original", canvas: null };
    contrastImages.set(image, original);
    return original;
  }
}
function prepareCubeIconContrast(root) {
  const image = root.querySelector("img");
  if (!image) return;
  const prepare = () => {
    if (!root.contains(image) || root.dataset.iconTone || image.naturalWidth <= 0 || image.naturalHeight <= 0)
      return;
    const variant = resolveCubeIconContrastImage(image);
    if (!variant) return;
    if (variant.canvas) {
      variant.canvas.className = "sugarcubes-cube-icon__contrast";
      variant.canvas.setAttribute("aria-hidden", "true");
      root.append(variant.canvas);
    }
    root.dataset.iconTone = variant.tone;
  };
  if (image.complete) prepare();
  else image.addEventListener("load", prepare, { once: true });
}
function drawIcon(document2, image, size) {
  const canvas = document2.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) return null;
  const scale = size / Math.max(image.naturalWidth, image.naturalHeight);
  const width = image.naturalWidth * scale;
  const height = image.naturalHeight * scale;
  context.drawImage(image, (size - width) / 2, (size - height) / 2, width, height);
  return { canvas, context };
}

// frontend/comfyui/ui/core/CubeIconResolver.ts
function createResolvedCubeIconElement(documentRef, model, className = "sugarcubes-cube-icon") {
  const doc = documentRef;
  const root = doc.createElement("span");
  root.className = className;
  if (model.kind === "asset" && model.url) {
    const image = doc.createElement("img");
    image.alt = "";
    image.decoding = "async";
    image.loading = "lazy";
    image.src = model.url;
    root.appendChild(image);
    prepareCubeIconContrast(root);
    return root;
  }
  if (model.kind === "initials" && model.initials) {
    const canvas = doc.createElement("canvas");
    canvas.setAttribute("aria-hidden", "true");
    canvas.className = "sugarcubes-cube-icon__fallback";
    drawFallbackInitialsElementCanvas(canvas, model);
    root.dataset.initials = model.initials;
    root.setAttribute("aria-hidden", "true");
    root.classList.add("is-initials");
    root.appendChild(canvas);
    return root;
  }
  root.classList.add("is-generic");
  root.textContent = "";
  return root;
}

// frontend/comfyui/ui/surface/stack/vue/CubeStackIcon.ts
var CubeStackIcon = defineComponent({
  name: "SugarCubesStackIcon",
  props: {
    icon: { type: Object, required: true },
    targetModel: { type: String, required: true }
  },
  setup(props) {
    let element = null;
    const render = () => {
      if (element) {
        const icon = createResolvedCubeIconElement(element.ownerDocument, props.icon);
        element.replaceChildren(icon);
      }
    };
    watch(() => JSON.stringify(props.icon), render);
    const retainElement = (value) => {
      const next = value instanceof HTMLElement ? value : null;
      if (element === next) return;
      element = next;
      render();
    };
    return () => h("span", { class: "sc-cube-stack-row__icon", "aria-hidden": "true" }, [
      h("span", { class: "sc-cube-stack-row__asset", ref: retainElement }),
      props.targetModel ? h("span", { class: "sc-cube-stack-row__model" }, props.targetModel) : null
    ]);
  }
});

export {
  CubeStackIcon
};
