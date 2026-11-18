// frontend/comfyui/ui/model_library/ModelPickerChoices.ts
function filterModelChoices(items, query, folder) {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  return items.filter(
    (item) => (folder === "" || item.folder === folder || item.folder.startsWith(`${folder}/`)) && terms.every(
      (term) => [item.title, item.version, item.value, item.baseModel, ...item.trainedWords].join(" ").toLocaleLowerCase().includes(term)
    )
  );
}
function modelChoiceFolders(items, folder) {
  const prefix = folder ? `${folder}/` : "";
  const children = /* @__PURE__ */ new Set();
  for (const item of items) {
    if (!item.folder.startsWith(prefix)) continue;
    const child = item.folder.slice(prefix.length).split("/")[0];
    if (child) children.add(`${prefix}${child}`);
  }
  return [...children].sort((left, right) => left.localeCompare(right));
}

export {
  filterModelChoices,
  modelChoiceFolders
};
