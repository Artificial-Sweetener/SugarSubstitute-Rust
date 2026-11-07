// integrations/shared/SessionPrimitives.ts
var SESSION_DOCUMENT_BYTES = 33554432;
function utf8Bytes(value) {
  let bytes = 0;
  for (const character of value) {
    const point = character.codePointAt(0) ?? 0;
    if (point >= 55296 && point <= 57343) return Infinity;
    bytes += point < 128 ? 1 : point < 2048 ? 2 : point < 65536 ? 3 : 4;
  }
  return bytes;
}
function sessionJsonScalars(value) {
  if (typeof value === "string") return Number.isFinite(utf8Bytes(value));
  if (typeof value === "number") return Number.isFinite(value);
  if (Array.isArray(value)) return value.every((item) => sessionJsonScalars(item));
  if (value && typeof value === "object")
    return Object.entries(value).every(
      ([key, item]) => Number.isFinite(utf8Bytes(key)) && sessionJsonScalars(item)
    );
  return value === null || typeof value === "boolean";
}
function sessionUuid(value) {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(value);
}
function sessionDigest(value) {
  return /^[0-9a-f]{64}$/.test(value);
}
function sessionProfile(value) {
  return /^[A-Za-z0-9_-]{1,64}$/.test(value);
}
function sessionScope(value) {
  return /^[0-9a-f]{32}$/.test(value.installation) && sessionProfile(value.profile) && [value.workspace, value.backend].every(sessionUuid);
}
function component(value) {
  return value.length > 0 && value !== "." && value !== ".." && !/[<>:"/\\|?*\u0000-\u001f\u007f]/.test(value) && !/[. ]$/.test(value) && !/^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(value) && Number.isFinite(utf8Bytes(value));
}
function sessionFile(value) {
  return component(value.filename) && (value.subfolder === "" || value.subfolder.split("/").every(component)) && (value.managedArtifact === null || value.managedArtifact.length > 0 && component(value.managedArtifact));
}
function sessionGraph(encoded) {
  if (utf8Bytes(encoded) > SESSION_DOCUMENT_BYTES) return false;
  let graph;
  try {
    graph = JSON.parse(encoded);
  } catch {
    return false;
  }
  if (!graph || typeof graph !== "object" || Array.isArray(graph)) return false;
  const pending = [{ value: graph, depth: 0 }];
  while (pending.length) {
    const item = pending.pop();
    if (!item || item.depth > 64) return false;
    const value = item.value;
    if (typeof value === "number" && !Number.isFinite(value)) return false;
    if (typeof value === "string" && !Number.isFinite(utf8Bytes(value))) return false;
    if (value && typeof value === "object") {
      for (const [key, child] of Object.entries(value)) {
        if (!Number.isFinite(utf8Bytes(key))) return false;
        pending.push({ value: child, depth: item.depth + 1 });
      }
    }
  }
  return true;
}

// integrations/shared/ContractRecord.ts
function isRecord(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

// integrations/shared/SessionOutputValidation.generated.ts
function isSessionScope(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "installation") && typeof value["installation"] === "string" && value["installation"].length <= 32 && Object.hasOwn(value, "profile") && typeof value["profile"] === "string" && value["profile"].length <= 64 && Object.hasOwn(value, "workspace") && typeof value["workspace"] === "string" && value["workspace"].length <= 36 && Object.hasOwn(value, "backend") && typeof value["backend"] === "string" && value["backend"].length <= 36;
}
function isSessionSource(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 36 && Object.hasOwn(value, "nodeId") && typeof value["nodeId"] === "string" && value["nodeId"].length <= 256 && Object.hasOwn(value, "executionPath") && typeof value["executionPath"] === "string" && value["executionPath"].length <= 4096 && Object.hasOwn(value, "label") && typeof value["label"] === "string" && value["label"].length <= 4096 && Object.hasOwn(value, "graphRevision") && typeof value["graphRevision"] === "number" && Number.isSafeInteger(value["graphRevision"]) && value["graphRevision"] >= 0 && value["graphRevision"] <= 9007199254740991;
}
function isSessionScene(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "groupId") && typeof value["groupId"] === "string" && value["groupId"].length <= 36 && Object.hasOwn(value, "sceneId") && typeof value["sceneId"] === "string" && value["sceneId"].length <= 36 && Object.hasOwn(value, "order") && typeof value["order"] === "number" && Number.isSafeInteger(value["order"]) && value["order"] >= 0 && value["order"] <= 8191 && Object.hasOwn(value, "count") && typeof value["count"] === "number" && Number.isSafeInteger(value["count"]) && value["count"] >= 1 && value["count"] <= 8192 && Object.hasOwn(value, "title") && typeof value["title"] === "string" && value["title"].length <= 4096;
}
function isSessionFile(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "storage") && (value["storage"] === "output" || value["storage"] === "input" || value["storage"] === "temp") && Object.hasOwn(value, "filename") && typeof value["filename"] === "string" && value["filename"].length <= 255 && Object.hasOwn(value, "subfolder") && typeof value["subfolder"] === "string" && value["subfolder"].length <= 4096 && Object.hasOwn(value, "managedArtifact") && (typeof value["managedArtifact"] === "string" && value["managedArtifact"].length <= 256 || value["managedArtifact"] === null);
}
function isSessionAsset(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "digest") && typeof value["digest"] === "string" && value["digest"].length <= 64 && Object.hasOwn(value, "bytes") && typeof value["bytes"] === "number" && Number.isSafeInteger(value["bytes"]) && value["bytes"] >= 1 && value["bytes"] <= 8589934592 && Object.hasOwn(value, "extension") && (value["extension"] === "png" || value["extension"] === "jpg" || value["extension"] === "webp" || value["extension"] === "gif" || value["extension"] === "mp4" || value["extension"] === "webm") && Object.hasOwn(value, "media") && (value["media"] === "image" || value["media"] === "video");
}
function isSessionOutput(value) {
  return isRecord(value) && Object.keys(value).length === 20 && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 36 && Object.hasOwn(value, "entryId") && typeof value["entryId"] === "string" && value["entryId"].length <= 36 && Object.hasOwn(value, "backend") && typeof value["backend"] === "string" && value["backend"].length <= 36 && Object.hasOwn(value, "promptId") && (typeof value["promptId"] === "string" && value["promptId"].length <= 256 || value["promptId"] === null) && Object.hasOwn(value, "runId") && typeof value["runId"] === "string" && value["runId"].length <= 36 && Object.hasOwn(value, "runOrder") && typeof value["runOrder"] === "number" && Number.isSafeInteger(value["runOrder"]) && value["runOrder"] >= 0 && value["runOrder"] <= 9007199254740991 && Object.hasOwn(value, "sourceId") && typeof value["sourceId"] === "string" && value["sourceId"].length <= 36 && Object.hasOwn(value, "scene") && (isSessionScene(value["scene"]) || value["scene"] === null) && Object.hasOwn(value, "listIndex") && typeof value["listIndex"] === "number" && Number.isSafeInteger(value["listIndex"]) && value["listIndex"] >= 0 && value["listIndex"] <= 8191 && Object.hasOwn(value, "batchIndex") && typeof value["batchIndex"] === "number" && Number.isSafeInteger(value["batchIndex"]) && value["batchIndex"] >= 0 && value["batchIndex"] <= 8191 && Object.hasOwn(value, "imageIndex") && typeof value["imageIndex"] === "number" && Number.isSafeInteger(value["imageIndex"]) && value["imageIndex"] >= 0 && value["imageIndex"] <= 8191 && Object.hasOwn(value, "file") && isSessionFile(value["file"]) && Object.hasOwn(value, "assetDigest") && (typeof value["assetDigest"] === "string" && value["assetDigest"].length <= 64 || value["assetDigest"] === null) && Object.hasOwn(value, "digest") && typeof value["digest"] === "string" && value["digest"].length <= 64 && Object.hasOwn(value, "bytes") && typeof value["bytes"] === "number" && Number.isSafeInteger(value["bytes"]) && value["bytes"] >= 1 && value["bytes"] <= 8589934592 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isSafeInteger(value["width"]) && value["width"] >= 1 && value["width"] <= 131072 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isSafeInteger(value["height"]) && value["height"] >= 1 && value["height"] <= 131072 && Object.hasOwn(value, "media") && (value["media"] === "image" || value["media"] === "video") && Object.hasOwn(value, "mime") && typeof value["mime"] === "string" && value["mime"].length <= 128 && Object.hasOwn(value, "metadataEmbedded") && typeof value["metadataEmbedded"] === "boolean";
}
function isSessionSubmission(value) {
  return isRecord(value) && Object.keys(value).length === 8 && Object.hasOwn(value, "entryId") && typeof value["entryId"] === "string" && value["entryId"].length <= 36 && Object.hasOwn(value, "backend") && typeof value["backend"] === "string" && value["backend"].length <= 36 && Object.hasOwn(value, "promptId") && typeof value["promptId"] === "string" && value["promptId"].length <= 256 && Object.hasOwn(value, "runId") && typeof value["runId"] === "string" && value["runId"].length <= 36 && Object.hasOwn(value, "runOrder") && typeof value["runOrder"] === "number" && Number.isSafeInteger(value["runOrder"]) && value["runOrder"] >= 0 && value["runOrder"] <= 9007199254740991 && Object.hasOwn(value, "graphRevision") && typeof value["graphRevision"] === "number" && Number.isSafeInteger(value["graphRevision"]) && value["graphRevision"] >= 0 && value["graphRevision"] <= 9007199254740991 && Object.hasOwn(value, "scene") && (isSessionScene(value["scene"]) || value["scene"] === null) && Object.hasOwn(value, "sources") && Array.isArray(value["sources"]) && value["sources"].length >= 1 && value["sources"].length <= 8192 && value["sources"].every((item) => isSessionSource(item));
}

// integrations/shared/SessionViewValidation.generated.ts
function isSessionSelection(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "outputId") && typeof value["outputId"] === "string" && value["outputId"].length <= 36 && Object.hasOwn(value, "sourceId") && typeof value["sourceId"] === "string" && value["sourceId"].length <= 36 && Object.hasOwn(value, "sceneId") && (typeof value["sceneId"] === "string" && value["sceneId"].length <= 36 || value["sceneId"] === null) && Object.hasOwn(value, "batchIndex") && typeof value["batchIndex"] === "number" && Number.isSafeInteger(value["batchIndex"]) && value["batchIndex"] >= 0 && value["batchIndex"] <= 8191 && Object.hasOwn(value, "listIndex") && typeof value["listIndex"] === "number" && Number.isSafeInteger(value["listIndex"]) && value["listIndex"] >= 0 && value["listIndex"] <= 8191;
}
function isSessionCamera(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "outputId") && typeof value["outputId"] === "string" && value["outputId"].length <= 36 && Object.hasOwn(value, "inspection") && isSessionInspection(value["inspection"]);
}
function isSessionComparison(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "enabled") && typeof value["enabled"] === "boolean" && Object.hasOwn(value, "selection") && (isSessionSelection(value["selection"]) || value["selection"] === null) && Object.hasOwn(value, "orientation") && (value["orientation"] === "horizontal" || value["orientation"] === "vertical") && Object.hasOwn(value, "split") && typeof value["split"] === "number" && Number.isFinite(value["split"]) && value["split"] >= 0 && value["split"] <= 1;
}
function isSessionView(value) {
  return isRecord(value) && Object.keys(value).length === 13 && Object.hasOwn(value, "canvasEnabled") && typeof value["canvasEnabled"] === "boolean" && Object.hasOwn(value, "masonryEnabled") && typeof value["masonryEnabled"] === "boolean" && Object.hasOwn(value, "editorWidth") && typeof value["editorWidth"] === "number" && Number.isSafeInteger(value["editorWidth"]) && value["editorWidth"] >= 0 && value["editorWidth"] <= 16384 && Object.hasOwn(value, "route") && (value["route"] === "input" || value["route"] === "output") && Object.hasOwn(value, "follow") && (value["follow"] === "auto" || value["follow"] === "manual") && Object.hasOwn(value, "mode") && (value["mode"] === "image" || value["mode"] === "overview") && Object.hasOwn(value, "linked") && typeof value["linked"] === "boolean" && Object.hasOwn(value, "selection") && (isSessionSelection(value["selection"]) || value["selection"] === null) && Object.hasOwn(value, "comparison") && isSessionComparison(value["comparison"]) && Object.hasOwn(value, "cameras") && Array.isArray(value["cameras"]) && value["cameras"].length <= 8192 && value["cameras"].every((item) => isSessionCamera(item)) && Object.hasOwn(value, "navigation") && isSessionNavigation(value["navigation"]) && Object.hasOwn(value, "sourcePreview") && typeof value["sourcePreview"] === "boolean" && Object.hasOwn(value, "sharedInspection") && (isSessionInspection(value["sharedInspection"]) || value["sharedInspection"] === null);
}
function isSessionGraphView(value) {
  return isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "x") && typeof value["x"] === "number" && Number.isFinite(value["x"]) && value["x"] >= -1e9 && value["x"] <= 1e9 && Object.hasOwn(value, "y") && typeof value["y"] === "number" && Number.isFinite(value["y"]) && value["y"] >= -1e9 && value["y"] <= 1e9 && Object.hasOwn(value, "scale") && typeof value["scale"] === "number" && Number.isFinite(value["scale"]) && value["scale"] >= 1e-6 && value["scale"] <= 1e6;
}
function isSessionNavigation(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "sceneOverview") && typeof value["sceneOverview"] === "boolean" && Object.hasOwn(value, "sceneId") && (typeof value["sceneId"] === "string" && value["sceneId"].length <= 36 || value["sceneId"] === null) && Object.hasOwn(value, "sourceId") && (typeof value["sourceId"] === "string" && value["sourceId"].length <= 36 || value["sourceId"] === null) && Object.hasOwn(value, "batchIndex") && typeof value["batchIndex"] === "number" && Number.isSafeInteger(value["batchIndex"]) && value["batchIndex"] >= 0 && value["batchIndex"] <= 8191 && Object.hasOwn(value, "batchOverview") && typeof value["batchOverview"] === "boolean";
}
function isSessionInspection(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "mode") && (value["mode"] === "fit" || value["mode"] === "native" || value["mode"] === "custom") && Object.hasOwn(value, "centerX") && typeof value["centerX"] === "number" && Number.isFinite(value["centerX"]) && value["centerX"] >= -1e6 && value["centerX"] <= 1e6 && Object.hasOwn(value, "centerY") && typeof value["centerY"] === "number" && Number.isFinite(value["centerY"]) && value["centerY"] >= -1e6 && value["centerY"] <= 1e6 && Object.hasOwn(value, "spanX") && typeof value["spanX"] === "number" && Number.isFinite(value["spanX"]) && value["spanX"] >= 1e-12 && value["spanX"] <= 1e12 && Object.hasOwn(value, "spanY") && typeof value["spanY"] === "number" && Number.isFinite(value["spanY"]) && value["spanY"] >= 1e-12 && value["spanY"] <= 1e12;
}

// integrations/shared/SessionSnapshotValidation.generated.ts
function isSessionDocument(value) {
  return isRecord(value) && Object.keys(value).length === 9 && Object.hasOwn(value, "graphJson") && typeof value["graphJson"] === "string" && value["graphJson"].length <= 33554432 && Object.hasOwn(value, "graphRevision") && typeof value["graphRevision"] === "number" && Number.isSafeInteger(value["graphRevision"]) && value["graphRevision"] >= 0 && value["graphRevision"] <= 9007199254740991 && Object.hasOwn(value, "path") && (typeof value["path"] === "string" && value["path"].length <= 4096 || value["path"] === null) && Object.hasOwn(value, "name") && typeof value["name"] === "string" && value["name"].length <= 4096 && Object.hasOwn(value, "temporary") && typeof value["temporary"] === "boolean" && Object.hasOwn(value, "dirty") && typeof value["dirty"] === "boolean" && Object.hasOwn(value, "sourceIdentity") && (typeof value["sourceIdentity"] === "string" && value["sourceIdentity"].length <= 4096 || value["sourceIdentity"] === null) && Object.hasOwn(value, "view") && isSessionGraphView(value["view"]) && Object.hasOwn(value, "baselineGraphJson") && (typeof value["baselineGraphJson"] === "string" && value["baselineGraphJson"].length <= 33554432 || value["baselineGraphJson"] === null);
}
function isSessionInput(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "documentId") && typeof value["documentId"] === "string" && value["documentId"].length <= 256 && Object.hasOwn(value, "generation") && typeof value["generation"] === "number" && Number.isSafeInteger(value["generation"]) && value["generation"] >= 0 && value["generation"] <= 9007199254740991 && Object.hasOwn(value, "signature") && typeof value["signature"] === "string" && value["signature"].length <= 64 && Object.hasOwn(value, "surface") && (typeof value["surface"] === "string" && value["surface"].length <= 256 || value["surface"] === null) && Object.hasOwn(value, "mask") && (typeof value["mask"] === "string" && value["mask"].length <= 256 || value["mask"] === null);
}
function isSessionEntry(value) {
  return isRecord(value) && Object.keys(value).length === 9 && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 36 && Object.hasOwn(value, "document") && isSessionDocument(value["document"]) && Object.hasOwn(value, "input") && (isSessionInput(value["input"]) || value["input"] === null) && Object.hasOwn(value, "view") && isSessionView(value["view"]) && Object.hasOwn(value, "sourceRevision") && typeof value["sourceRevision"] === "number" && Number.isSafeInteger(value["sourceRevision"]) && value["sourceRevision"] >= 0 && value["sourceRevision"] <= 9007199254740991 && Object.hasOwn(value, "navigationRevision") && typeof value["navigationRevision"] === "number" && Number.isSafeInteger(value["navigationRevision"]) && value["navigationRevision"] >= 0 && value["navigationRevision"] <= 9007199254740991 && Object.hasOwn(value, "sources") && Array.isArray(value["sources"]) && value["sources"].length <= 8192 && value["sources"].every((item) => isSessionSource(item)) && Object.hasOwn(value, "outputs") && Array.isArray(value["outputs"]) && value["outputs"].length <= 8192 && value["outputs"].every((item) => isSessionOutput(item)) && Object.hasOwn(value, "submissions") && Array.isArray(value["submissions"]) && value["submissions"].length <= 8192 && value["submissions"].every((item) => isSessionSubmission(item));
}
function isSessionRect(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "x") && typeof value["x"] === "number" && Number.isFinite(value["x"]) && value["x"] >= -1e6 && value["x"] <= 1e6 && Object.hasOwn(value, "y") && typeof value["y"] === "number" && Number.isFinite(value["y"]) && value["y"] >= -1e6 && value["y"] <= 1e6 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isFinite(value["width"]) && value["width"] >= 1 && value["width"] <= 1e6 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isFinite(value["height"]) && value["height"] >= 1 && value["height"] <= 1e6;
}
function isSessionPlacement(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "bounds") && isSessionRect(value["bounds"]) && Object.hasOwn(value, "maximized") && typeof value["maximized"] === "boolean" && Object.hasOwn(value, "monitor") && typeof value["monitor"] === "string" && value["monitor"].length <= 256 && Object.hasOwn(value, "workArea") && isSessionRect(value["workArea"]) && Object.hasOwn(value, "dpi") && typeof value["dpi"] === "number" && Number.isFinite(value["dpi"]) && value["dpi"] >= 24 && value["dpi"] <= 960;
}
function isSessionCheckpoint(value) {
  return isRecord(value) && Object.keys(value).length === 14 && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 36 && Object.hasOwn(value, "revision") && typeof value["revision"] === "number" && Number.isSafeInteger(value["revision"]) && value["revision"] >= 0 && value["revision"] <= 9007199254740991 && Object.hasOwn(value, "previousId") && (typeof value["previousId"] === "string" && value["previousId"].length <= 36 || value["previousId"] === null) && Object.hasOwn(value, "capturedAt") && typeof value["capturedAt"] === "number" && Number.isSafeInteger(value["capturedAt"]) && value["capturedAt"] >= 0 && value["capturedAt"] <= 9007199254740991 && Object.hasOwn(value, "scope") && isSessionScope(value["scope"]) && Object.hasOwn(value, "producer") && typeof value["producer"] === "string" && value["producer"].length <= 256 && Object.hasOwn(value, "frontend") && typeof value["frontend"] === "string" && value["frontend"].length <= 256 && Object.hasOwn(value, "sugarcubes") && typeof value["sugarcubes"] === "string" && value["sugarcubes"].length <= 256 && Object.hasOwn(value, "entries") && Array.isArray(value["entries"]) && value["entries"].length <= 256 && value["entries"].every((item) => isSessionEntry(item)) && Object.hasOwn(value, "activeEntry") && (typeof value["activeEntry"] === "string" && value["activeEntry"].length <= 36 || value["activeEntry"] === null) && Object.hasOwn(value, "route") && (value["route"] === "workflow" || value["route"] === "settings") && Object.hasOwn(value, "placement") && (isSessionPlacement(value["placement"]) || value["placement"] === null) && Object.hasOwn(value, "assets") && Array.isArray(value["assets"]) && value["assets"].length <= 65536 && value["assets"].every((item) => isSessionAsset(item));
}

// integrations/shared/SessionValidation.generated.ts
function isSessionBinding(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "entryId") && typeof value["entryId"] === "string" && value["entryId"].length <= 36 && Object.hasOwn(value, "runtimeSession") && typeof value["runtimeSession"] === "string" && value["runtimeSession"].length <= 256 && Object.hasOwn(value, "generation") && typeof value["generation"] === "number" && Number.isSafeInteger(value["generation"]) && value["generation"] >= 0 && value["generation"] <= 9007199254740991 && Object.hasOwn(value, "graphRevision") && typeof value["graphRevision"] === "number" && Number.isSafeInteger(value["graphRevision"]) && value["graphRevision"] >= 0 && value["graphRevision"] <= 9007199254740991 && Object.hasOwn(value, "sourceRevision") && typeof value["sourceRevision"] === "number" && Number.isSafeInteger(value["sourceRevision"]) && value["sourceRevision"] >= 0 && value["sourceRevision"] <= 9007199254740991 && Object.hasOwn(value, "navigationRevision") && typeof value["navigationRevision"] === "number" && Number.isSafeInteger(value["navigationRevision"]) && value["navigationRevision"] >= 0 && value["navigationRevision"] <= 9007199254740991;
}
function isSessionFence(value) {
  return isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "webview") && typeof value["webview"] === "string" && value["webview"].length <= 36 && Object.hasOwn(value, "revision") && typeof value["revision"] === "number" && Number.isSafeInteger(value["revision"]) && value["revision"] >= 0 && value["revision"] <= 9007199254740991 && Object.hasOwn(value, "bindings") && Array.isArray(value["bindings"]) && value["bindings"].length <= 256 && value["bindings"].every((item) => isSessionBinding(item));
}
function isSessionCapabilities(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "capture") && typeof value["capture"] === "boolean" && Object.hasOwn(value, "materialize") && typeof value["materialize"] === "boolean" && Object.hasOwn(value, "chunkTransfer") && typeof value["chunkTransfer"] === "boolean" && Object.hasOwn(value, "artifactResolution") && typeof value["artifactResolution"] === "boolean";
}
function isSessionTransfer(value) {
  return isRecord(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 36 && Object.hasOwn(value, "checkpointId") && typeof value["checkpointId"] === "string" && value["checkpointId"].length <= 36 && Object.hasOwn(value, "revision") && typeof value["revision"] === "number" && Number.isSafeInteger(value["revision"]) && value["revision"] >= 0 && value["revision"] <= 9007199254740991 && Object.hasOwn(value, "bytes") && typeof value["bytes"] === "number" && Number.isSafeInteger(value["bytes"]) && value["bytes"] >= 1 && value["bytes"] <= 134217728 && Object.hasOwn(value, "chunks") && typeof value["chunks"] === "number" && Number.isSafeInteger(value["chunks"]) && value["chunks"] >= 1 && value["chunks"] <= 8192 && Object.hasOwn(value, "digest") && typeof value["digest"] === "string" && value["digest"].length <= 64 && Object.hasOwn(value, "direction") && (value["direction"] === "capture" || value["direction"] === "materialize");
}
function isSessionFailure(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "code") && (value["code"] === "unsupported" || value["code"] === "invalid" || value["code"] === "too-large" || value["code"] === "stale" || value["code"] === "cancelled" || value["code"] === "untrusted-origin" || value["code"] === "unavailable" || value["code"] === "identity-mismatch" || value["code"] === "quota" || value["code"] === "storage" || value["code"] === "migration-required" || value["code"] === "opted-out" || value["code"] === "not-committed") && Object.hasOwn(value, "retryable") && typeof value["retryable"] === "boolean" && Object.hasOwn(value, "entryId") && (typeof value["entryId"] === "string" && value["entryId"].length <= 36 || value["entryId"] === null) && Object.hasOwn(value, "outputId") && (typeof value["outputId"] === "string" && value["outputId"].length <= 36 || value["outputId"] === null);
}
function isSessionOperation(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "kind") && value["kind"] === "ready" && Object.hasOwn(value, "capabilities") && isSessionCapabilities(value["capabilities"]) && Object.hasOwn(value, "committed") && typeof value["committed"] === "boolean" && Object.hasOwn(value, "workflowPersistence") && typeof value["workflowPersistence"] === "boolean" && Object.hasOwn(value, "fence") && isSessionFence(value["fence"]) || isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "kind") && value["kind"] === "capture" && Object.hasOwn(value, "fence") && isSessionFence(value["fence"]) && Object.hasOwn(value, "checkpointId") && typeof value["checkpointId"] === "string" && value["checkpointId"].length <= 36 && Object.hasOwn(value, "revision") && typeof value["revision"] === "number" && Number.isSafeInteger(value["revision"]) && value["revision"] >= 0 && value["revision"] <= 9007199254740991 || isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "kind") && value["kind"] === "materialize" && Object.hasOwn(value, "fence") && isSessionFence(value["fence"]) && Object.hasOwn(value, "transferId") && typeof value["transferId"] === "string" && value["transferId"].length <= 36 || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "begin" && Object.hasOwn(value, "transfer") && isSessionTransfer(value["transfer"]) || isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "kind") && value["kind"] === "chunk" && Object.hasOwn(value, "transferId") && typeof value["transferId"] === "string" && value["transferId"].length <= 36 && Object.hasOwn(value, "index") && typeof value["index"] === "number" && Number.isSafeInteger(value["index"]) && value["index"] >= 0 && value["index"] <= 8191 && Object.hasOwn(value, "text") && typeof value["text"] === "string" && value["text"].length <= 65536 || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "commit" && Object.hasOwn(value, "transferId") && typeof value["transferId"] === "string" && value["transferId"].length <= 36 || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "cancel" && Object.hasOwn(value, "targetRequestId") && typeof value["targetRequestId"] === "string" && value["targetRequestId"].length <= 36 || isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "kind") && value["kind"] === "acknowledged" && Object.hasOwn(value, "checkpointId") && typeof value["checkpointId"] === "string" && value["checkpointId"].length <= 36 && Object.hasOwn(value, "revision") && typeof value["revision"] === "number" && Number.isSafeInteger(value["revision"]) && value["revision"] >= 0 && value["revision"] <= 9007199254740991 && Object.hasOwn(value, "phase") && (value["phase"] === "captured" || value["phase"] === "materialized" || value["phase"] === "durable" || value["phase"] === "restored" || value["phase"] === "cancelled") && Object.hasOwn(value, "bindings") && Array.isArray(value["bindings"]) && value["bindings"].length <= 256 && value["bindings"].every((item) => isSessionBinding(item)) || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "error" && Object.hasOwn(value, "failure") && isSessionFailure(value["failure"]) || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "resolve" && Object.hasOwn(value, "output") && isSessionOutput(value["output"]) || isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "kind") && value["kind"] === "resolved" && Object.hasOwn(value, "outputId") && typeof value["outputId"] === "string" && value["outputId"].length <= 36 && Object.hasOwn(value, "digest") && typeof value["digest"] === "string" && value["digest"].length <= 64 && Object.hasOwn(value, "bytes") && typeof value["bytes"] === "number" && Number.isSafeInteger(value["bytes"]) && value["bytes"] >= 1 && value["bytes"] <= 8589934592 && Object.hasOwn(value, "file") && isSessionFile(value["file"]);
}
function isSessionMessage(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.desktop-session" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "string" && value["requestId"].length <= 36 && Object.hasOwn(value, "scope") && isSessionScope(value["scope"]) && Object.hasOwn(value, "origin") && typeof value["origin"] === "string" && value["origin"].length <= 2048 && Object.hasOwn(value, "operation") && isSessionOperation(value["operation"]);
}

// integrations/shared/SessionDocuments.ts
function sessionDocumentError(value) {
  if (!isSessionDocument(value) || !sessionJsonScalars(value)) return "invalid";
  if (utf8Bytes(value.graphJson) + utf8Bytes(value.baselineGraphJson ?? "") > SESSION_DOCUMENT_BYTES)
    return "too-large";
  if (!value.name || value.path !== null && !value.path || value.temporary && value.path !== null || !sessionGraph(value.graphJson) || value.baselineGraphJson !== null && !sessionGraph(value.baselineGraphJson))
    return "invalid";
  return utf8Bytes(JSON.stringify(value)) > SESSION_DOCUMENT_BYTES ? "too-large" : null;
}

// integrations/shared/SessionPlacement.ts
function sessionPlacementError(value) {
  return isSessionPlacement(value) && sessionJsonScalars(value) && value.monitor.length > 0 && !/[\u0000-\u001f\u007f-\u009f]/.test(value.monitor) ? null : "invalid";
}

// integrations/shared/SessionScenes.ts
function sessionScenes(entry2) {
  const slots = /* @__PURE__ */ new Map();
  const facts = /* @__PURE__ */ new Map();
  const counts = /* @__PURE__ */ new Map();
  for (const item of [...entry2.submissions, ...entry2.outputs]) {
    const scene = item.scene;
    if (scene === null) continue;
    const slot = `${scene.groupId}/${String(scene.order)}`;
    const identity = `${scene.groupId}/${scene.sceneId}`;
    const description = JSON.stringify([scene.order, scene.count, scene.title]);
    if (slots.has(slot) && slots.get(slot) !== scene.sceneId || facts.has(identity) && facts.get(identity) !== description || counts.has(scene.groupId) && counts.get(scene.groupId) !== scene.count)
      return false;
    slots.set(slot, scene.sceneId);
    facts.set(identity, description);
    counts.set(scene.groupId, scene.count);
  }
  return true;
}

// integrations/shared/SessionNavigation.ts
function sessionNavigation(entry2) {
  const navigation = entry2.view.navigation;
  if (navigation.sceneOverview && navigation.sceneId !== null) return false;
  if (navigation.sourceId !== null && !entry2.sources.some((source) => source.id === navigation.sourceId))
    return false;
  if (navigation.sceneId !== null && !entry2.submissions.some((item) => item.scene?.sceneId === navigation.sceneId) && !entry2.outputs.some((item) => item.scene?.sceneId === navigation.sceneId))
    return false;
  if (navigation.batchOverview) return true;
  const outputs = entry2.outputs.filter(
    (item) => (navigation.sourceId === null || item.sourceId === navigation.sourceId) && (navigation.sceneOverview || (item.scene?.sceneId ?? null) === navigation.sceneId)
  );
  return outputs.length === 0 ? navigation.batchIndex === 0 : outputs.some((item) => item.batchIndex === navigation.batchIndex);
}

// integrations/shared/SessionSubmissions.ts
function sessionSubmissions(entry2, scope) {
  const prompts = /* @__PURE__ */ new Set();
  const runs = /* @__PURE__ */ new Set();
  for (const submission of entry2.submissions) {
    const key = `${submission.runId}/${submission.scene?.sceneId ?? ""}`;
    if (submission.entryId !== entry2.id || submission.backend !== scope.backend || !sessionUuid(submission.runId) || !submission.promptId || prompts.has(submission.promptId) || runs.has(key) || submission.graphRevision > entry2.document.graphRevision || submission.scene !== null && (!sessionUuid(submission.scene.groupId) || !sessionUuid(submission.scene.sceneId) || submission.scene.order >= submission.scene.count))
      return false;
    prompts.add(submission.promptId);
    runs.add(key);
    const sources = /* @__PURE__ */ new Set();
    for (const source of submission.sources) {
      if (!sessionUuid(source.id) || sources.has(source.id) || !source.nodeId || !source.executionPath || source.graphRevision !== submission.graphRevision)
        return false;
      sources.add(source.id);
    }
  }
  return entry2.outputs.every(
    (output) => output.promptId === null || entry2.submissions.some(
      (submission) => submission.promptId === output.promptId && submission.runId === output.runId && submission.runOrder === output.runOrder && (submission.scene === null ? output.scene === null : output.scene !== null && submission.scene.groupId === output.scene.groupId && submission.scene.sceneId === output.scene.sceneId && submission.scene.order === output.scene.order && submission.scene.count === output.scene.count && submission.scene.title === output.scene.title) && submission.sources.some((source) => source.id === output.sourceId)
    )
  );
}

// integrations/shared/SessionSnapshot.ts
function selection(value, outputs) {
  if (value === null) return true;
  const output = outputs.get(value.outputId);
  return !!output && output.sourceId === value.sourceId && (output.scene?.sceneId ?? null) === value.sceneId && output.batchIndex === value.batchIndex && output.listIndex === value.listIndex;
}
function sessionOutput(value) {
  return [value.id, value.entryId, value.backend, value.runId, value.sourceId].every(sessionUuid) && sessionDigest(value.digest) && (value.assetDigest === null || sessionDigest(value.assetDigest)) && (value.promptId === null || value.promptId.length > 0) && sessionFile(value.file) && (value.file.storage !== "temp" || value.assetDigest === value.digest) && (value.assetDigest === null || value.assetDigest === value.digest) && (value.scene === null || [value.scene.groupId, value.scene.sceneId].every(sessionUuid) && value.scene.order < value.scene.count);
}
function entry(value, checkpoint) {
  if (!sessionSubmissions(value, checkpoint.scope) || !sessionScenes(value) || !sessionNavigation(value) || !sessionUuid(value.id) || sessionDocumentError(value.document) !== null || utf8Bytes(JSON.stringify(value.view)) > 1048576 || value.input !== null && (!value.input.documentId || !sessionDigest(value.input.signature)))
    return false;
  const sources = /* @__PURE__ */ new Set();
  for (const source of value.sources) {
    if (!sessionUuid(source.id) || sources.has(source.id) || !source.nodeId || !source.executionPath || source.graphRevision > value.document.graphRevision)
      return false;
    sources.add(source.id);
  }
  const outputs = /* @__PURE__ */ new Map();
  const positions = /* @__PURE__ */ new Set();
  for (const output of value.outputs) {
    const position = JSON.stringify([
      output.runId,
      output.scene?.sceneId ?? null,
      output.sourceId,
      output.listIndex,
      output.batchIndex,
      output.imageIndex
    ]);
    if (!sessionOutput(output) || output.entryId !== value.id || output.backend !== checkpoint.scope.backend || !sources.has(output.sourceId) || outputs.has(output.id) || positions.has(position))
      return false;
    outputs.set(output.id, output);
    positions.add(position);
  }
  if (!selection(value.view.selection, outputs) || !selection(value.view.comparison.selection, outputs) || value.view.comparison.enabled && value.view.comparison.selection === null)
    return false;
  const cameras = /* @__PURE__ */ new Set();
  for (const camera of value.view.cameras) {
    if (!outputs.has(camera.outputId) || cameras.has(camera.outputId)) return false;
    cameras.add(camera.outputId);
  }
  return true;
}
function isDesktopCheckpoint(value) {
  if (!isSessionCheckpoint(value) || !sessionJsonScalars(value) || !sessionScope(value.scope) || !sessionUuid(value.id) || value.previousId !== null && (!sessionUuid(value.previousId) || value.previousId === value.id) || !value.producer || !value.frontend || !value.sugarcubes || value.placement !== null && sessionPlacementError(value.placement) !== null)
    return false;
  const entries = /* @__PURE__ */ new Set();
  const outputs = /* @__PURE__ */ new Set();
  const prompts = /* @__PURE__ */ new Set();
  let count = 0;
  for (const item of value.entries) {
    if (entries.has(item.id) || !entry(item, value)) return false;
    entries.add(item.id);
    count += item.outputs.length;
    for (const submission of item.submissions) {
      if (prompts.has(submission.promptId)) return false;
      prompts.add(submission.promptId);
    }
    for (const output of item.outputs) {
      if (outputs.has(output.id)) return false;
      outputs.add(output.id);
    }
  }
  if (count > 65536 || (value.activeEntry === null ? entries.size !== 0 : !entries.has(value.activeEntry)))
    return false;
  const assets = new Map(value.assets.map((asset) => [asset.digest, asset]));
  if (assets.size !== value.assets.length || value.assets.some(
    (asset) => !sessionDigest(asset.digest) || (["mp4", "webm"].includes(asset.extension) ? asset.media !== "video" : asset.media !== "image")
  ) || value.assets.reduce((total, asset) => total + asset.bytes, 0) > 8589934592)
    return false;
  const referenced = /* @__PURE__ */ new Set();
  for (const item of value.entries)
    for (const output of item.outputs) {
      if (output.assetDigest === null) continue;
      const asset = assets.get(output.assetDigest);
      if (!asset || asset.digest !== output.digest || asset.bytes !== output.bytes || asset.media !== output.media)
        return false;
      referenced.add(asset.digest);
    }
  return referenced.size === assets.size && utf8Bytes(JSON.stringify(value)) <= 134217728;
}

// integrations/shared/SessionProtocol.ts
var SESSION_MESSAGE_BYTES = 524288;
var SESSION_CHUNK_BYTES = 65536;
function bindings(values) {
  return new Set(values.map((item) => item.entryId)).size === values.length && new Set(values.map((item) => item.runtimeSession)).size === values.length && values.every((item) => sessionUuid(item.entryId) && item.runtimeSession.length > 0);
}
function sameSessionFence(left, right) {
  if (left.webview !== right.webview || left.revision !== right.revision || left.bindings.length !== right.bindings.length || !bindings(left.bindings) || !bindings(right.bindings))
    return false;
  return left.bindings.every(
    (item) => right.bindings.some(
      (other) => item.entryId === other.entryId && item.runtimeSession === other.runtimeSession && item.generation === other.generation && item.graphRevision === other.graphRevision && item.sourceRevision === other.sourceRevision && item.navigationRevision === other.navigationRevision
    )
  );
}
function sessionMessageError(value, scope, origin) {
  if (!isSessionMessage(value) || !sessionJsonScalars(value) || !sessionScope(value.scope) || !sessionUuid(value.requestId))
    return "invalid";
  if (!origin || value.origin !== origin) return "untrusted-origin";
  if (Object.keys(scope).some(
    (key) => scope[key] !== value.scope[key]
  ))
    return "stale";
  const operation = value.operation;
  switch (operation.kind) {
    case "ready":
      if (!sessionUuid(operation.fence.webview) || !bindings(operation.fence.bindings))
        return "invalid";
      if ((operation.capabilities.capture || operation.capabilities.materialize) && !operation.capabilities.chunkTransfer)
        return "invalid";
      break;
    case "capture":
    case "materialize":
      if (!sessionUuid(operation.fence.webview) || !bindings(operation.fence.bindings) || !sessionUuid(operation.kind === "capture" ? operation.checkpointId : operation.transferId))
        return "invalid";
      break;
    case "begin": {
      const transfer = operation.transfer;
      if (![transfer.id, transfer.checkpointId].every(sessionUuid) || !sessionDigest(transfer.digest) || transfer.bytes < transfer.chunks || transfer.bytes > transfer.chunks * SESSION_CHUNK_BYTES)
        return "invalid";
      break;
    }
    case "chunk":
      if (!sessionUuid(operation.transferId) || !operation.text || utf8Bytes(operation.text) > SESSION_CHUNK_BYTES)
        return "invalid";
      break;
    case "commit":
      if (!sessionUuid(operation.transferId)) return "invalid";
      break;
    case "cancel":
      if (!sessionUuid(operation.targetRequestId)) return "invalid";
      break;
    case "acknowledged":
      if (!sessionUuid(operation.checkpointId) || !bindings(operation.bindings)) return "invalid";
      break;
    case "error":
      if ([operation.failure.entryId, operation.failure.outputId].some(
        (id) => id !== null && !sessionUuid(id)
      ))
        return "invalid";
      break;
    case "resolve":
      if (!sessionOutput(operation.output) || operation.output.backend !== scope.backend)
        return "invalid";
      break;
    case "resolved":
      if (!sessionUuid(operation.outputId) || !sessionDigest(operation.digest) || !sessionFile(operation.file))
        return "invalid";
      break;
  }
  return utf8Bytes(JSON.stringify(value)) <= SESSION_MESSAGE_BYTES ? null : "too-large";
}
function sessionAdmission(message, capabilities, committed, workflowPersistence, current) {
  const operation = message.operation;
  if (operation.kind === "ready") {
    const fence = operation.fence;
    if (fence.webview !== current.webview || fence.revision < current.revision) return "stale";
    if (fence.revision === current.revision && (!sameSessionFence(fence, current) || operation.committed !== committed || operation.workflowPersistence !== workflowPersistence || Object.keys(capabilities).some(
      (key) => capabilities[key] !== operation.capabilities[key]
    )))
      return "stale";
    return null;
  }
  if (operation.kind === "begin" || operation.kind === "chunk" || operation.kind === "commit")
    return capabilities.chunkTransfer ? null : "unsupported";
  if (operation.kind === "resolve") return capabilities.artifactResolution ? null : "unsupported";
  if (operation.kind !== "capture" && operation.kind !== "materialize") return null;
  if (!capabilities.chunkTransfer || !capabilities[operation.kind]) return "unsupported";
  if (!workflowPersistence) return "opted-out";
  if (!committed) return "not-committed";
  return sameSessionFence(operation.fence, current) ? null : "stale";
}

// integrations/shared/SessionDecode.ts
function uniqueMembers(text) {
  let offset = 0;
  function space() {
    while (/[\x20\t\r\n]/.test(text[offset] ?? "\0")) offset++;
  }
  function string() {
    const start = offset++;
    while (offset < text.length) {
      const character = text[offset++];
      if (character === "\\") offset++;
      else if (character === '"') return text.slice(start, offset);
    }
    throw new Error("Unterminated JSON string");
  }
  function value(depth) {
    if (depth > 64) throw new Error("JSON nesting exceeds session bound");
    space();
    const open = text[offset];
    if (open === '"') {
      string();
      return;
    }
    if (open === "{" || open === "[") {
      const close = open === "{" ? "}" : "]";
      const keys = /* @__PURE__ */ new Set();
      offset++;
      space();
      if (text[offset] === close) {
        offset++;
        return;
      }
      while (offset < text.length) {
        if (open === "{") {
          space();
          if (text[offset] !== '"') throw new Error("Expected JSON member");
          const key = JSON.parse(string());
          if (typeof key !== "string" || keys.has(key)) throw new Error("Duplicate JSON member");
          keys.add(key);
          space();
          if (text[offset++] !== ":") throw new Error("Expected JSON colon");
        }
        value(depth + 1);
        space();
        if (text[offset] === close) {
          offset++;
          return;
        }
        if (text[offset++] !== ",") throw new Error("Expected JSON comma");
      }
      throw new Error("Unterminated JSON container");
    }
    const start = offset;
    while (offset < text.length && !/[\x20\t\r\n,\]}]/.test(text[offset] ?? "")) offset++;
    if (offset === start) throw new Error("Missing JSON value");
  }
  try {
    value(0);
    space();
    return offset === text.length;
  } catch {
    return false;
  }
}
function parseSessionJson(text, maximum) {
  if (text.length > maximum || utf8Bytes(text) > maximum || !uniqueMembers(text)) return null;
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}
function decodeDesktopCheckpoint(text) {
  const value = parseSessionJson(text, 134217728);
  return isDesktopCheckpoint(value) ? value : null;
}
export {
  SESSION_CHUNK_BYTES,
  SESSION_DOCUMENT_BYTES,
  decodeDesktopCheckpoint,
  isDesktopCheckpoint,
  isSessionBinding,
  isSessionDocument,
  isSessionMessage,
  isSessionScope,
  parseSessionJson,
  sameSessionFence,
  sessionAdmission,
  sessionGraph,
  sessionJsonScalars,
  sessionMessageError,
  sessionScope,
  sessionUuid,
  utf8Bytes
};
