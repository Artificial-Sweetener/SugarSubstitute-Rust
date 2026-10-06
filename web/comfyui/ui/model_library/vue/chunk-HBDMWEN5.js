// frontend/comfyui/ui/model_library/ModelCatalog.ts
function chooseModelThumbnail(item, role, size) {
  const candidates = item.thumbnails.filter((image) => image.role === role).sort((left, right) => Math.max(left.width, left.height) - Math.max(right.width, right.height));
  return candidates.find((image) => Math.max(image.width, image.height) >= size) ?? candidates.at(-1) ?? null;
}

export {
  chooseModelThumbnail
};
