// integrations/shared/ContractRecord.ts
function isRecord(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

// integrations/shared/SessionOutputValidation.generated.ts
function isSessionScene(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "groupId") && typeof value["groupId"] === "string" && value["groupId"].length <= 36 && Object.hasOwn(value, "sceneId") && typeof value["sceneId"] === "string" && value["sceneId"].length <= 36 && Object.hasOwn(value, "order") && typeof value["order"] === "number" && Number.isSafeInteger(value["order"]) && value["order"] >= 0 && value["order"] <= 8191 && Object.hasOwn(value, "count") && typeof value["count"] === "number" && Number.isSafeInteger(value["count"]) && value["count"] >= 1 && value["count"] <= 8192 && Object.hasOwn(value, "title") && typeof value["title"] === "string" && value["title"].length <= 4096;
}
function isSessionFile(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "storage") && (value["storage"] === "output" || value["storage"] === "input" || value["storage"] === "temp") && Object.hasOwn(value, "filename") && typeof value["filename"] === "string" && value["filename"].length <= 255 && Object.hasOwn(value, "subfolder") && typeof value["subfolder"] === "string" && value["subfolder"].length <= 4096 && Object.hasOwn(value, "managedArtifact") && (typeof value["managedArtifact"] === "string" && value["managedArtifact"].length <= 256 || value["managedArtifact"] === null);
}
function isSessionOutput(value) {
  return isRecord(value) && Object.keys(value).length === 20 && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 36 && Object.hasOwn(value, "entryId") && typeof value["entryId"] === "string" && value["entryId"].length <= 36 && Object.hasOwn(value, "backend") && typeof value["backend"] === "string" && value["backend"].length <= 36 && Object.hasOwn(value, "promptId") && (typeof value["promptId"] === "string" && value["promptId"].length <= 256 || value["promptId"] === null) && Object.hasOwn(value, "runId") && typeof value["runId"] === "string" && value["runId"].length <= 36 && Object.hasOwn(value, "runOrder") && typeof value["runOrder"] === "number" && Number.isSafeInteger(value["runOrder"]) && value["runOrder"] >= 0 && value["runOrder"] <= 9007199254740991 && Object.hasOwn(value, "sourceId") && typeof value["sourceId"] === "string" && value["sourceId"].length <= 36 && Object.hasOwn(value, "scene") && (isSessionScene(value["scene"]) || value["scene"] === null) && Object.hasOwn(value, "listIndex") && typeof value["listIndex"] === "number" && Number.isSafeInteger(value["listIndex"]) && value["listIndex"] >= 0 && value["listIndex"] <= 8191 && Object.hasOwn(value, "batchIndex") && typeof value["batchIndex"] === "number" && Number.isSafeInteger(value["batchIndex"]) && value["batchIndex"] >= 0 && value["batchIndex"] <= 8191 && Object.hasOwn(value, "imageIndex") && typeof value["imageIndex"] === "number" && Number.isSafeInteger(value["imageIndex"]) && value["imageIndex"] >= 0 && value["imageIndex"] <= 8191 && Object.hasOwn(value, "file") && isSessionFile(value["file"]) && Object.hasOwn(value, "assetDigest") && (typeof value["assetDigest"] === "string" && value["assetDigest"].length <= 64 || value["assetDigest"] === null) && Object.hasOwn(value, "digest") && typeof value["digest"] === "string" && value["digest"].length <= 64 && Object.hasOwn(value, "bytes") && typeof value["bytes"] === "number" && Number.isSafeInteger(value["bytes"]) && value["bytes"] >= 1 && value["bytes"] <= 8589934592 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isSafeInteger(value["width"]) && value["width"] >= 1 && value["width"] <= 131072 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isSafeInteger(value["height"]) && value["height"] >= 1 && value["height"] <= 131072 && Object.hasOwn(value, "media") && (value["media"] === "image" || value["media"] === "video") && Object.hasOwn(value, "mime") && typeof value["mime"] === "string" && value["mime"].length <= 128 && Object.hasOwn(value, "metadataEmbedded") && typeof value["metadataEmbedded"] === "boolean";
}

// integrations/shared/SessionPrimitives.ts
function utf8Bytes(value) {
  let bytes = 0;
  for (const character of value) {
    const point = character.codePointAt(0) ?? 0;
    if (point >= 55296 && point <= 57343) return Infinity;
    bytes += point < 128 ? 1 : point < 2048 ? 2 : point < 65536 ? 3 : 4;
  }
  return bytes;
}
function sessionUuid(value) {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(value);
}
function sessionDigest(value) {
  return /^[0-9a-f]{64}$/.test(value);
}
function component(value) {
  return value.length > 0 && value !== "." && value !== ".." && !/[<>:"/\\|?*\u0000-\u001f\u007f]/.test(value) && !/[. ]$/.test(value) && !/^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(value) && Number.isFinite(utf8Bytes(value));
}
function sessionFile(value) {
  return component(value.filename) && (value.subfolder === "" || value.subfolder.split("/").every(component)) && (value.managedArtifact === null || value.managedArtifact.length > 0 && component(value.managedArtifact));
}

export {
  isRecord,
  isSessionFile,
  isSessionOutput,
  sessionUuid,
  sessionDigest,
  sessionFile
};
