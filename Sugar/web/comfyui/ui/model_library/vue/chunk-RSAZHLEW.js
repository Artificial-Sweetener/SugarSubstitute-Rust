// frontend/comfyui/ui/model_library/ModelPickerChoices.ts
function filterModelChoices(items, query, folder) {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  return items.filter(
    (item) => (folder === "" || item.folder === folder || item.folder.startsWith(`${folder}/`)) && terms.every(
      (term) => [item.title, item.version, item.value, item.baseModel, ...item.trainedWords].join(" ").toLocaleLowerCase().includes(term)
    )
  );
}
function compareNames(left, right) {
  const folded = left.toLowerCase();
  const other = right.toLowerCase();
  if (folded !== other) return folded < other ? -1 : 1;
  if (left === right) return 0;
  return left < right ? -1 : 1;
}
function modelChoiceFolders(items, folder) {
  const prefix = folder ? `${folder}/` : "";
  const children = /* @__PURE__ */ new Set();
  for (const item of items) {
    if (!item.folder.startsWith(prefix)) continue;
    const child = item.folder.slice(prefix.length).split("/")[0];
    if (child) children.add(`${prefix}${child}`);
  }
  return [...children].sort(compareNames);
}

export {
  filterModelChoices,
  modelChoiceFolders
};
