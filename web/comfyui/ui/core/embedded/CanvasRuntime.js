// integrations/shared/ContractRecord.ts
function isRecord(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

// integrations/shared/CanvasSurfaceValidation.generated.ts
function isCanvasSurfaceCapabilities(value) {
  return isRecord(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "schema") && value["schema"] === 1 && Object.hasOwn(value, "imageFormats") && Array.isArray(value["imageFormats"]) && value["imageFormats"].length <= 3 && value["imageFormats"].every(
    (item) => item === "png" || item === "jpeg" || item === "webp"
  ) && Object.hasOwn(value, "viewportCommands") && Array.isArray(value["viewportCommands"]) && value["viewportCommands"].length <= 4 && value["viewportCommands"].every(
    (item) => item === "fit" || item === "native" || item === "zoom" || item === "pan"
  ) && Object.hasOwn(value, "viewportState") && typeof value["viewportState"] === "boolean" && Object.hasOwn(value, "previewFormats") && Array.isArray(value["previewFormats"]) && value["previewFormats"].length <= 2 && value["previewFormats"].every((item) => item === "png" || item === "jpeg") && Object.hasOwn(value, "comparison") && typeof value["comparison"] === "boolean" && Object.hasOwn(value, "overview") && typeof value["overview"] === "boolean";
}
function isCanvasSurfaceCapabilitiesRequest(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "surface.capabilities.request" && Object.hasOwn(value, "schema") && value["schema"] === 1;
}
function isCanvasSurfaceCapabilitiesReply(value) {
  return isRecord(value) && Object.keys(value).length === 8 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "surface.capabilities.reply" && Object.hasOwn(value, "status") && (value["status"] === "available" || value["status"] === "unsupported" || value["status"] === "unavailable" || value["status"] === "error") && Object.hasOwn(value, "capabilities") && isCanvasSurfaceCapabilities(value["capabilities"]) && Object.hasOwn(value, "message") && typeof value["message"] === "string" && value["message"].length <= 1024 && Array.from(value["message"]).length <= 512 && new RegExp("^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$").test(
    value["message"]
  );
}
function isCanvasSurfaceAddress(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "sessionId") && typeof value["sessionId"] === "string" && value["sessionId"].length <= 512 && Array.from(value["sessionId"]).length <= 256 && new RegExp(
    "^canvas:(?:[^\\u0000-\\u001F\\u007F-\\u009F\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])+(?![\\s\\S])"
  ).test(value["sessionId"]) && Object.hasOwn(value, "viewId") && typeof value["viewId"] === "string" && value["viewId"].length <= 256 && Array.from(value["viewId"]).length <= 128 && new RegExp(
    "^view:(?:[^\\u0000-\\u001F\\u007F-\\u009F\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])+(?![\\s\\S])"
  ).test(value["viewId"]) && Object.hasOwn(value, "windowId") && typeof value["windowId"] === "string" && value["windowId"].length <= 256 && Array.from(value["windowId"]).length <= 128 && new RegExp(
    "^window:(?:[^\\u0000-\\u001F\\u007F-\\u009F\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])+(?![\\s\\S])"
  ).test(value["windowId"]) && Object.hasOwn(value, "attachmentId") && typeof value["attachmentId"] === "string" && value["attachmentId"].length <= 256 && Array.from(value["attachmentId"]).length <= 128 && new RegExp(
    "^attachment:(?:[^\\u0000-\\u001F\\u007F-\\u009F\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])+(?![\\s\\S])"
  ).test(value["attachmentId"]) && Object.hasOwn(value, "generation") && typeof value["generation"] === "number" && Number.isSafeInteger(value["generation"]) && value["generation"] >= 1 && value["generation"] <= 9007199254740991;
}
function isCanvasSurfaceFileReference(value) {
  return isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "filename") && typeof value["filename"] === "string" && value["filename"].length <= 8192 && Array.from(value["filename"]).length <= 4096 && new RegExp("^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$").test(
    value["filename"]
  ) && Object.hasOwn(value, "subfolder") && typeof value["subfolder"] === "string" && value["subfolder"].length <= 8192 && Array.from(value["subfolder"]).length <= 4096 && new RegExp("^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$").test(
    value["subfolder"]
  ) && Object.hasOwn(value, "type") && (value["type"] === "input" || value["type"] === "output" || value["type"] === "temp");
}
function isCanvasSurfacePoint(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "x") && typeof value["x"] === "number" && Number.isFinite(value["x"]) && value["x"] >= -1e6 && value["x"] <= 1e6 && Object.hasOwn(value, "y") && typeof value["y"] === "number" && Number.isFinite(value["y"]) && value["y"] >= -1e6 && value["y"] <= 1e6;
}
function isCanvasSurfaceSize(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isFinite(value["width"]) && value["width"] >= 0 && value["width"] <= 1e6 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isFinite(value["height"]) && value["height"] >= 0 && value["height"] <= 1e6;
}
function isCanvasSurfaceClip(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "x") && typeof value["x"] === "number" && Number.isFinite(value["x"]) && value["x"] >= -1e6 && value["x"] <= 1e6 && Object.hasOwn(value, "y") && typeof value["y"] === "number" && Number.isFinite(value["y"]) && value["y"] >= -1e6 && value["y"] <= 1e6 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isFinite(value["width"]) && value["width"] >= 0 && value["width"] <= 1e6 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isFinite(value["height"]) && value["height"] >= 0 && value["height"] <= 1e6;
}
function isCanvasSurfaceGeometry(value) {
  return isRecord(value) && Object.keys(value).length === 8 && Object.hasOwn(value, "coordinateSpace") && value["coordinateSpace"] === "client" && Object.hasOwn(value, "units") && value["units"] === "css-pixels" && Object.hasOwn(value, "origin") && isCanvasSurfacePoint(value["origin"]) && Object.hasOwn(value, "viewport") && isCanvasSurfaceSize(value["viewport"]) && Object.hasOwn(value, "clip") && isCanvasSurfaceClip(value["clip"]) && Object.hasOwn(value, "physicalPixelsPerCssPixel") && typeof value["physicalPixelsPerCssPixel"] === "number" && Number.isFinite(value["physicalPixelsPerCssPixel"]) && value["physicalPixelsPerCssPixel"] > 0 && value["physicalPixelsPerCssPixel"] <= 64 && Object.hasOwn(value, "visible") && typeof value["visible"] === "boolean" && Object.hasOwn(value, "focused") && typeof value["focused"] === "boolean";
}
function isCanvasSurfacePrepareRequest(value) {
  return isRecord(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "surface.prepare" && Object.hasOwn(value, "address") && isCanvasSurfaceAddress(value["address"]) && Object.hasOwn(value, "revision") && typeof value["revision"] === "number" && Number.isSafeInteger(value["revision"]) && value["revision"] >= 1 && value["revision"] <= 9007199254740991;
}
function isCanvasSurfaceGeometryRequest(value) {
  return isRecord(value) && Object.keys(value).length === 8 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "surface.geometry" && Object.hasOwn(value, "address") && isCanvasSurfaceAddress(value["address"]) && Object.hasOwn(value, "revision") && typeof value["revision"] === "number" && Number.isSafeInteger(value["revision"]) && value["revision"] >= 1 && value["revision"] <= 9007199254740991 && Object.hasOwn(value, "geometry") && isCanvasSurfaceGeometry(value["geometry"]);
}
function isCanvasSurfaceSelectRequest(value) {
  return isRecord(value) && Object.keys(value).length === 9 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "surface.select" && Object.hasOwn(value, "address") && isCanvasSurfaceAddress(value["address"]) && Object.hasOwn(value, "revision") && typeof value["revision"] === "number" && Number.isSafeInteger(value["revision"]) && value["revision"] >= 1 && value["revision"] <= 9007199254740991 && Object.hasOwn(value, "itemId") && typeof value["itemId"] === "string" && value["itemId"].length <= 8192 && Array.from(value["itemId"]).length <= 4096 && new RegExp(
    "^result:(?:[^\\u0000-\\u001F\\u007F-\\u009F\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])+(?![\\s\\S])"
  ).test(value["itemId"]) && Object.hasOwn(value, "file") && isCanvasSurfaceFileReference(value["file"]);
}
function isCanvasSurfaceCommitRequest(value) {
  return isRecord(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "surface.commit" && Object.hasOwn(value, "address") && isCanvasSurfaceAddress(value["address"]) && Object.hasOwn(value, "revision") && typeof value["revision"] === "number" && Number.isSafeInteger(value["revision"]) && value["revision"] >= 1 && value["revision"] <= 9007199254740991;
}
function isCanvasSurfaceReleaseRequest(value) {
  return isRecord(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "surface.release" && Object.hasOwn(value, "address") && isCanvasSurfaceAddress(value["address"]) && Object.hasOwn(value, "revision") && typeof value["revision"] === "number" && Number.isSafeInteger(value["revision"]) && value["revision"] >= 1 && value["revision"] <= 9007199254740991;
}
function isCanvasSurfaceReply(value) {
  return isRecord(value) && Object.keys(value).length === 9 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "surface.reply" && Object.hasOwn(value, "address") && isCanvasSurfaceAddress(value["address"]) && Object.hasOwn(value, "revision") && typeof value["revision"] === "number" && Number.isSafeInteger(value["revision"]) && value["revision"] >= 1 && value["revision"] <= 9007199254740991 && Object.hasOwn(value, "status") && (value["status"] === "ready" || value["status"] === "applied" || value["status"] === "stale" || value["status"] === "cancelled" || value["status"] === "unavailable" || value["status"] === "error") && Object.hasOwn(value, "message") && typeof value["message"] === "string" && value["message"].length <= 1024 && Array.from(value["message"]).length <= 512 && new RegExp("^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$").test(
    value["message"]
  );
}
function isCanvasSurfaceMessage(value) {
  return isCanvasSurfaceCapabilitiesRequest(value) || isCanvasSurfaceCapabilitiesReply(value) || isCanvasSurfacePrepareRequest(value) || isCanvasSurfaceGeometryRequest(value) || isCanvasSurfaceSelectRequest(value) || isCanvasSurfaceCommitRequest(value) || isCanvasSurfaceReleaseRequest(value) || isCanvasSurfaceReply(value);
}

// integrations/shared/CanvasValidation.generated.ts
function isCanvasCapabilities(value) {
  return isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "displayOutput") && typeof value["displayOutput"] === "boolean" && Object.hasOwn(value, "insertImageLayer") && typeof value["insertImageLayer"] === "boolean" && Object.hasOwn(value, "positionImageLayer") && typeof value["positionImageLayer"] === "boolean" || isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "displayOutput") && typeof value["displayOutput"] === "boolean" && Object.hasOwn(value, "insertImageLayer") && typeof value["insertImageLayer"] === "boolean" && Object.hasOwn(value, "positionImageLayer") && typeof value["positionImageLayer"] === "boolean" && Object.hasOwn(value, "geometry") && typeof value["geometry"] === "boolean";
}
function isCanvasDescriptor(value) {
  return isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 128 && Object.hasOwn(value, "label") && typeof value["label"] === "string" && value["label"].length <= 128 && Object.hasOwn(value, "capabilities") && isCanvasCapabilities(value["capabilities"]);
}
function isCanvasDocument(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 128 && Object.hasOwn(value, "geometryRevision") && typeof value["geometryRevision"] === "string" && value["geometryRevision"].length <= 512 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isSafeInteger(value["width"]) && value["width"] >= 1 && value["width"] <= 3e5 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isSafeInteger(value["height"]) && value["height"] >= 1 && value["height"] <= 3e5;
}
function isCanvasFailure(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "status") && (value["status"] === "unavailable" || value["status"] === "unsupported" || value["status"] === "stale" || value["status"] === "cancelled" || value["status"] === "error") && Object.hasOwn(value, "message") && typeof value["message"] === "string" && value["message"].length <= 512;
}
function isCanvasInsertedLayer(value) {
  return isRecord(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "status") && value["status"] === "inserted" && Object.hasOwn(value, "documentId") && typeof value["documentId"] === "string" && value["documentId"].length <= 128 && Object.hasOwn(value, "layerId") && typeof value["layerId"] === "string" && value["layerId"].length <= 128 && Object.hasOwn(value, "left") && typeof value["left"] === "number" && Number.isSafeInteger(value["left"]) && value["left"] >= -3e5 && value["left"] <= 3e5 && Object.hasOwn(value, "top") && typeof value["top"] === "number" && Number.isSafeInteger(value["top"]) && value["top"] >= -3e5 && value["top"] <= 3e5 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isSafeInteger(value["width"]) && value["width"] >= 1 && value["width"] <= 3e5 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isSafeInteger(value["height"]) && value["height"] >= 1 && value["height"] <= 3e5;
}
function isCanvasInsertResult(value) {
  return isCanvasInsertedLayer(value) || isCanvasFailure(value);
}
function isCanvasContext(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "canvas") && isCanvasDescriptor(value["canvas"]) && Object.hasOwn(value, "document") && (isCanvasDocument(value["document"]) || value["document"] === null);
}
function isCanvasInsertImage(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "document") && isCanvasDocument(value["document"]) && Object.hasOwn(value, "image") && isCanvasEncodedImage(value["image"]) && Object.hasOwn(value, "layerName") && typeof value["layerName"] === "string" && value["layerName"].length <= 128 && Object.hasOwn(value, "position") && (value["position"] === null || isCanvasImagePosition(value["position"]));
}
function isCanvasReadRequest(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "context.request";
}
function isCanvasReadReply(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "context.reply" && Object.hasOwn(value, "context") && isCanvasContext(value["context"]);
}
function isCanvasInsertRequest(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "image.insert" && Object.hasOwn(value, "image") && isCanvasInsertImage(value["image"]);
}
function isCanvasInsertReply(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "image.reply" && Object.hasOwn(value, "result") && isCanvasInsertResult(value["result"]);
}
function isCanvasCancelRequest(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "cancel";
}
function isCanvasErrorReply(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "error.reply" && Object.hasOwn(value, "failure") && isCanvasFailure(value["failure"]);
}
function isCanvasEncodedImage(value) {
  return isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "format") && value["format"] === "png" && Object.hasOwn(value, "encoding") && value["encoding"] === "base64" && Object.hasOwn(value, "data") && typeof value["data"] === "string" && value["data"].length <= 89478488;
}
function isCanvasSourceTarget(value) {
  return isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "kind") && value["kind"] === "selection" || isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "kind") && value["kind"] === "current-layer" || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "layer" && Object.hasOwn(value, "layerId") && typeof value["layerId"] === "string" && value["layerId"].length <= 128 || isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "kind") && value["kind"] === "composition" || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "composition-from-layer" && Object.hasOwn(value, "layerId") && typeof value["layerId"] === "string" && value["layerId"].length <= 128;
}
function isCanvasInputTarget(value) {
  return isCanvasSourceTarget(value) || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "file" && Object.hasOwn(value, "file") && (typeof value["file"] === "string" && value["file"].length <= 4096 || value["file"] === null);
}
function isCanvasPixelRaster(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isSafeInteger(value["width"]) && value["width"] >= 1 && value["width"] <= 65536 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isSafeInteger(value["height"]) && value["height"] >= 1 && value["height"] <= 65536 && Object.hasOwn(value, "format") && (value["format"] === "rgba8" || value["format"] === "coverage8") && Object.hasOwn(value, "data") && typeof value["data"] === "string" && value["data"].length <= 89478488;
}
function isCanvasSourceLayer(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 128 && Object.hasOwn(value, "name") && typeof value["name"] === "string" && value["name"].length <= 256 && Object.hasOwn(value, "parentId") && (typeof value["parentId"] === "string" && value["parentId"].length <= 128 || value["parentId"] === null) && Object.hasOwn(value, "depth") && typeof value["depth"] === "number" && Number.isSafeInteger(value["depth"]) && value["depth"] >= 0 && value["depth"] <= 128 && Object.hasOwn(value, "visible") && typeof value["visible"] === "boolean";
}
function isCanvasSourceContext(value) {
  return isRecord(value) && Object.keys(value).length === 8 && Object.hasOwn(value, "kind") && value["kind"] === "context" && Object.hasOwn(value, "supported") && typeof value["supported"] === "boolean" && Object.hasOwn(value, "document") && (isCanvasDocument(value["document"]) || value["document"] === null) && Object.hasOwn(value, "revision") && typeof value["revision"] === "string" && value["revision"].length <= 512 && Object.hasOwn(value, "targets") && Array.isArray(value["targets"]) && value["targets"].length <= 5 && value["targets"].every(
    (item) => item === "selection" || item === "current-layer" || item === "layer" || item === "composition" || item === "composition-from-layer"
  ) && Object.hasOwn(value, "layers") && Array.isArray(value["layers"]) && value["layers"].length <= 4096 && value["layers"].every((item) => isCanvasSourceLayer(item)) && Object.hasOwn(value, "currentLayerId") && (typeof value["currentLayerId"] === "string" && value["currentLayerId"].length <= 128 || value["currentLayerId"] === null) && Object.hasOwn(value, "observesChanges") && typeof value["observesChanges"] === "boolean";
}
function isCanvasSourceSnapshot(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "target") && isCanvasSourceTarget(value["target"]) && Object.hasOwn(value, "document") && isCanvasDocument(value["document"]) && Object.hasOwn(value, "revision") && typeof value["revision"] === "string" && value["revision"].length <= 512 && Object.hasOwn(value, "pixels") && isCanvasPixelRaster(value["pixels"]);
}
function isCanvasSourceCommand(value) {
  return isCanvasSourceQuery(value) || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "type") && value["type"] === "watch" && Object.hasOwn(value, "enabled") && typeof value["enabled"] === "boolean";
}
function isCanvasSourceResult(value) {
  return isCanvasSourceContext(value) || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "captured" && Object.hasOwn(value, "snapshots") && Array.isArray(value["snapshots"]) && value["snapshots"].length <= 64 && value["snapshots"].every((item) => isCanvasSourceSnapshot(item)) || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "watching" && Object.hasOwn(value, "supported") && typeof value["supported"] === "boolean" || isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "kind") && value["kind"] === "failure" && Object.hasOwn(value, "status") && (value["status"] === "unavailable" || value["status"] === "unsupported" || value["status"] === "stale" || value["status"] === "cancelled" || value["status"] === "error") && Object.hasOwn(value, "message") && typeof value["message"] === "string" && value["message"].length <= 512;
}
function isCanvasSourceRequest(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "source.request" && Object.hasOwn(value, "command") && isCanvasSourceCommand(value["command"]);
}
function isCanvasSourceReply(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "source.reply" && Object.hasOwn(value, "result") && isCanvasSourceResult(value["result"]);
}
function isCanvasSourceChanged(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && value["requestId"] === 0 && Object.hasOwn(value, "type") && value["type"] === "source.changed" && Object.hasOwn(value, "change") && isCanvasSourceChange(value["change"]);
}
function isCanvasSourceChange(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "documentId") && (typeof value["documentId"] === "string" && value["documentId"].length <= 128 || value["documentId"] === null) && Object.hasOwn(value, "revision") && typeof value["revision"] === "string" && value["revision"].length <= 512;
}
function isCanvasSourceQuery(value) {
  return isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "type") && value["type"] === "context" || isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "type") && value["type"] === "capture" && Object.hasOwn(value, "document") && isCanvasDocument(value["document"]) && Object.hasOwn(value, "targets") && Array.isArray(value["targets"]) && value["targets"].length <= 64 && value["targets"].every((item) => isCanvasSourceTarget(item)) && Object.hasOwn(value, "maximumSize") && typeof value["maximumSize"] === "number" && Number.isSafeInteger(value["maximumSize"]) && value["maximumSize"] >= 0 && value["maximumSize"] <= 1024;
}
function isCanvasInputSource(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "file" && Object.hasOwn(value, "file") && (typeof value["file"] === "string" && value["file"].length <= 4096 || value["file"] === null) || isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "kind") && value["kind"] === "canvas" && Object.hasOwn(value, "canvasId") && typeof value["canvasId"] === "string" && value["canvasId"].length <= 128 && Object.hasOwn(value, "documentId") && (typeof value["documentId"] === "string" && value["documentId"].length <= 128 || value["documentId"] === null) && Object.hasOwn(value, "target") && isCanvasSourceTarget(value["target"]);
}
function isCanvasImagePosition(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "left") && typeof value["left"] === "number" && Number.isSafeInteger(value["left"]) && value["left"] >= -3e5 && value["left"] <= 3e5 && Object.hasOwn(value, "top") && typeof value["top"] === "number" && Number.isSafeInteger(value["top"]) && value["top"] >= -3e5 && value["top"] <= 3e5;
}
function isCanvasDimensions(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isSafeInteger(value["width"]) && value["width"] >= 1 && value["width"] <= 3e5 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isSafeInteger(value["height"]) && value["height"] >= 1 && value["height"] <= 3e5;
}
function isCanvasSelectionBounds(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "left") && typeof value["left"] === "number" && Number.isSafeInteger(value["left"]) && value["left"] >= -3e5 && value["left"] <= 3e5 && Object.hasOwn(value, "top") && typeof value["top"] === "number" && Number.isSafeInteger(value["top"]) && value["top"] >= -3e5 && value["top"] <= 3e5 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isSafeInteger(value["width"]) && value["width"] >= 1 && value["width"] <= 3e5 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isSafeInteger(value["height"]) && value["height"] >= 1 && value["height"] <= 3e5;
}
function isCanvasGeometryCommand(value) {
  return isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "type") && value["type"] === "context" || isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "type") && value["type"] === "resize" && Object.hasOwn(value, "document") && isCanvasDocument(value["document"]) && Object.hasOwn(value, "size") && isCanvasDimensions(value["size"]);
}
function isCanvasGeometryResult(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "kind") && value["kind"] === "context" && Object.hasOwn(value, "supported") && typeof value["supported"] === "boolean" && Object.hasOwn(value, "resize") && typeof value["resize"] === "boolean" && Object.hasOwn(value, "selectionSupported") && typeof value["selectionSupported"] === "boolean" && Object.hasOwn(value, "document") && (isCanvasDocument(value["document"]) || value["document"] === null) && Object.hasOwn(value, "selection") && (isCanvasSelectionBounds(value["selection"]) || value["selection"] === null) || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "resized" && Object.hasOwn(value, "document") && isCanvasDocument(value["document"]) || isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "kind") && value["kind"] === "failure" && Object.hasOwn(value, "status") && (value["status"] === "unavailable" || value["status"] === "unsupported" || value["status"] === "stale" || value["status"] === "cancelled" || value["status"] === "error") && Object.hasOwn(value, "message") && typeof value["message"] === "string" && value["message"].length <= 512;
}
function isCanvasGeometryRequest(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "geometry.request" && Object.hasOwn(value, "command") && isCanvasGeometryCommand(value["command"]);
}
function isCanvasGeometryReply(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "type") && value["type"] === "geometry.reply" && Object.hasOwn(value, "result") && isCanvasGeometryResult(value["result"]);
}
function isCanvasMessage(value) {
  return isCanvasReadRequest(value) || isCanvasReadReply(value) || isCanvasInsertRequest(value) || isCanvasInsertReply(value) || isCanvasCancelRequest(value) || isCanvasErrorReply(value) || isCanvasSourceRequest(value) || isCanvasSourceReply(value) || isCanvasSourceChanged(value) || isCanvasGeometryRequest(value) || isCanvasGeometryReply(value) || isCanvasSurfaceMessage(value);
}

// integrations/shared/CanvasRemoteChanges.ts
var CanvasRemoteChanges = class {
  /** Keep protocol-specific requests and native event envelopes outside this lifetime owner. */
  constructor(port) {
    this.port = port;
  }
  #listeners = /* @__PURE__ */ new Set();
  #ready = null;
  #stop = null;
  #epoch = 0;
  #accepted = false;
  #buffer = [];
  #overflow = false;
  #watchSent = false;
  #disposed = false;
  /** Register before starting the watch so synchronous native notifications cannot escape the lease. */
  subscribe(listener) {
    if (this.#disposed) return { ready: Promise.resolve("unavailable"), stop: () => {
    } };
    this.#listeners.add(listener);
    const ready = this.#ready ?? this.#start();
    let stopped = false;
    return {
      ready,
      stop: () => {
        if (stopped) return;
        stopped = true;
        this.#listeners.delete(listener);
        if (!this.#listeners.size) this.#release();
      }
    };
  }
  /** Stop accepting events before releasing the session's transport. */
  dispose() {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#listeners.clear();
    this.#release();
  }
  /** Retain bounded early notifications without losing changes to different masks or documents. */
  #start() {
    const epoch = ++this.#epoch;
    this.#stop = this.port.listen((change) => {
      if (!this.#owns(epoch)) return;
      if (this.#accepted) this.#publish(change);
      else if (this.#buffer.length < 64) this.#buffer.push(change);
      else this.#overflow = true;
    });
    this.#watchSent = true;
    const ready = this.port.watch(true).then(
      (supported) => {
        if (!this.#owns(epoch)) return "unavailable";
        const buffered = this.#buffer;
        this.#buffer = [];
        if (supported && this.#overflow) {
          this.#stop?.();
          this.#stop = null;
          this.#stopWatch();
          this.port.report(
            new Error(
              "Canvas changes exceeded the pending observation budget. Refresh the canvas sources."
            )
          );
          return "unavailable";
        }
        this.#accepted = supported;
        if (supported)
          for (const change of buffered) {
            if (!this.#owns(epoch)) break;
            this.#publish(change);
          }
        if (!supported) {
          this.#stop?.();
          this.#stop = null;
        }
        return supported ? "observing" : "unsupported";
      },
      (error) => {
        if (this.#owns(epoch)) {
          this.#buffer = [];
          this.#stop?.();
          this.#stop = null;
          this.#stopWatch();
          this.port.report(error);
        }
        return "unavailable";
      }
    );
    this.#ready = ready;
    return ready;
  }
  /** Notify only consumers still mounted in the current observation lifetime. */
  #publish(change) {
    for (const listener of this.#listeners) listener(change);
  }
  /** Re-read ownership after consumer callbacks may dispose or replace the observation lifetime. */
  #owns(epoch) {
    return !this.#disposed && epoch === this.#epoch;
  }
  /** Send the matching stop even when its start acknowledgement has not arrived yet. */
  #release() {
    if (!this.#ready) return;
    this.#epoch++;
    this.#ready = null;
    this.#accepted = false;
    this.#buffer = [];
    this.#overflow = false;
    this.#stop?.();
    this.#stop = null;
    this.#stopWatch();
  }
  /** Send exactly one stop for each admitted native watch, including an overflowing start. */
  #stopWatch() {
    if (!this.#watchSent) return;
    this.#watchSent = false;
    void this.port.watch(false).catch((error) => {
      if (!this.#disposed) this.port.report(error);
    });
  }
};

// integrations/shared/CanvasSourceConnection.ts
var CanvasSourceConnection = class {
  /** Borrow the session transport without creating a second request-ID sequence. */
  constructor(port) {
    this.port = port;
    this.changes = new CanvasRemoteChanges({
      listen: (listener) => this.port.listen((change) => {
        this.#context = null;
        listener(change);
      }),
      watch: async (enabled) => {
        const result = await this.port.execute({ type: "watch", enabled });
        if (result.kind !== "watching")
          throw new Error(
            result.kind === "failure" ? result.message : "Invalid observation acknowledgement."
          );
        return result.supported;
      },
      report: (error) => {
        this.port.report(error);
      }
    });
  }
  changes;
  #disposed = false;
  #context = null;
  /** Resolve identity from the actual connected canvas rather than an editor-specific name. */
  get canvasId() {
    return this.port.identity();
  }
  /** Share simultaneous metadata reads while keeping later reads and captures current. */
  execute(command) {
    if (this.#disposed)
      return Promise.resolve({
        kind: "failure",
        status: "unavailable",
        message: "Canvas source connection closed."
      });
    if (command.type !== "context") return this.port.execute(command);
    if (this.#context) return this.#context;
    const pending = this.port.execute(command).finally(() => {
      if (this.#context === pending) this.#context = null;
    });
    this.#context = pending;
    return pending;
  }
  /** Release observation before the shared request client tears down its pending work. */
  dispose() {
    if (this.#disposed) return;
    this.changes.dispose();
    this.#disposed = true;
    this.#context = null;
  }
};

// integrations/shared/CanvasSurfaceClient.ts
async function requestCanvasSurface(command, cancellation, request) {
  if (cancellation?.cancelled())
    return { status: "cancelled", message: "Canvas surface request cancelled." };
  const address = Object.freeze({ ...command.address });
  const revision = command.revision;
  const type = command.type;
  const reply = await request({ ...command, address }, cancellation);
  if (reply.type !== "surface.reply" || reply.revision !== revision || !sameSurface(reply.address, address) || reply.status === "ready" && type !== "surface.prepare" || reply.status === "applied" && type === "surface.prepare")
    throw new Error("Canvas surface reply does not match its attachment.");
  return { status: reply.status, message: reply.message };
}
function sameSurface(left, right) {
  return left.sessionId === right.sessionId && left.viewId === right.viewId && left.windowId === right.windowId && left.attachmentId === right.attachmentId && left.generation === right.generation;
}

// integrations/shared/CanvasSurfaceDiscovery.ts
var EMPTY_CANVAS_SURFACE_CAPABILITIES = Object.freeze({
  schema: 1,
  imageFormats: Object.freeze([]),
  viewportCommands: Object.freeze([]),
  viewportState: false,
  previewFormats: Object.freeze([]),
  comparison: false,
  overview: false
});
function emptyCanvasSurfaceCapabilities(value) {
  return value.imageFormats.length === 0 && value.viewportCommands.length === 0 && !value.viewportState && value.previewFormats.length === 0 && !value.comparison && !value.overview;
}
function snapshotCanvasSurfaceCapabilities(value) {
  return Object.freeze({
    ...value,
    imageFormats: Object.freeze([...value.imageFormats]),
    viewportCommands: Object.freeze([...value.viewportCommands]),
    previewFormats: Object.freeze([...value.previewFormats])
  });
}
async function discoverCanvasSurface(port) {
  try {
    const reply = await port.read();
    if (!port.current()) return null;
    if (reply.type !== "surface.capabilities.reply")
      throw new Error("Invalid canvas surface capability reply.");
    if (reply.status !== "available") {
      if (!emptyCanvasSurfaceCapabilities(reply.capabilities))
        throw new Error("Unavailable canvas surface declared capabilities.");
      if (reply.status === "error")
        port.report(new Error(reply.message || "Canvas surface discovery failed."));
      return null;
    }
    const host = Object.freeze({
      capabilities: snapshotCanvasSurfaceCapabilities(reply.capabilities),
      execute: (command, cancellation) => command.type === "surface.release" || port.current() && port.owns(host) ? port.execute(command, cancellation) : Promise.resolve({ status: "unavailable", message: "The canvas context changed." })
    });
    return host;
  } catch (error) {
    if (port.current()) port.report(error);
    return null;
  }
}

// integrations/shared/CanvasClient.ts
var CanvasClient = class {
  /** Bind an authenticated session and injected clock without claiming its transport lifetime. */
  constructor(session, port) {
    this.session = session;
    this.port = port;
    this.geometry = {
      get canvasId() {
        return readIdentity();
      },
      execute: (command, cancellation) => this.#geometryRequest(command, cancellation)
    };
    const readIdentity = () => this.#context?.canvas.id ?? "";
    this.sources = new CanvasSourceConnection({
      identity: () => this.#context?.canvas.id ?? "",
      execute: (command) => this.#sourceRequest(command),
      listen: (listener) => {
        this.#sourceListeners.add(listener);
        return () => {
          this.#sourceListeners.delete(listener);
        };
      },
      report: (error) => {
        this.port.report(error);
      }
    });
  }
  geometry;
  sources;
  #surface = null;
  #pending = /* @__PURE__ */ new Map();
  #sourceListeners = /* @__PURE__ */ new Set();
  #next = 0;
  #context = null;
  #contextRevision = {};
  #disposed = false;
  /** Discover only capabilities actually reported by this canvas implementation. */
  readCanvas() {
    return this.#context?.canvas ?? null;
  }
  /** Expose only a surface discovered for the current context read in this live session. */
  get surface() {
    return this.#surface;
  }
  /** Read native context while keeping cached capabilities on the latest reply in this live session. */
  async readContext() {
    const revision = {};
    this.#contextRevision = revision;
    this.#surface = null;
    const reply = await this.#request({ type: "context.request" }, "context.reply");
    if (reply.type !== "context.reply") throw new Error("Invalid canvas context reply.");
    this.#requireConnected();
    if (revision === this.#contextRevision) {
      this.#context = reply.context;
      if (reply.context.canvas.capabilities.displayOutput) {
        const surface = await discoverCanvasSurface({
          read: () => this.#request(
            { type: "surface.capabilities.request", schema: 1 },
            "surface.capabilities.reply"
          ),
          current: () => !this.#disposed && revision === this.#contextRevision,
          owns: (host) => this.#surface === host,
          execute: (command, cancellation) => requestCanvasSurface(
            command,
            cancellation,
            (admitted, scope) => this.#request(admitted, "surface.reply", scope)
          ),
          report: (error) => {
            this.port.report(error);
          }
        });
        if (revision === this.#contextRevision) this.#surface = surface;
      }
    }
    this.#requireConnected();
    return reply.context;
  }
  /** Recheck page teardown after each awaited discovery step. */
  #requireConnected() {
    if (this.#disposed) throw new Error("Canvas connection closed.");
  }
  /** Insert once using the caller's captured destination rather than a later active document. */
  async insertImage(image, cancellation) {
    if (cancellation?.cancelled())
      return { status: "cancelled", message: "Canvas insertion cancelled." };
    const reply = await this.#request({ type: "image.insert", image }, "image.reply", cancellation);
    if (reply.type !== "image.reply") throw new Error("Invalid canvas image reply.");
    return reply.result;
  }
  /** Accept only the expected reply kind from this session and retire its deadline before callbacks run. */
  receive(value) {
    if (this.#disposed || !isCanvasMessage(value) || value.session !== this.session) return false;
    if (value.type === "source.changed") {
      for (const listener of this.#sourceListeners) listener(value.change);
      return true;
    }
    const pending = this.#pending.get(value.requestId);
    if (!pending || pending.type !== value.type && value.type !== "error.reply") return false;
    this.#pending.delete(value.requestId);
    pending.stop();
    if (value.type === "error.reply") pending.reject(new Error(value.failure.message));
    else if (value.type === "surface.capabilities.reply")
      pending.accept({
        ...value,
        capabilities: snapshotCanvasSurfaceCapabilities(value.capabilities)
      });
    else if (value.type === "context.reply")
      pending.accept({ ...value, context: snapshotCanvasContext(value.context) });
    else pending.accept(value);
    return true;
  }
  /** Refuse all late responses and cancel outstanding host work when the session leaves. */
  dispose() {
    if (this.#disposed) return;
    this.sources.dispose();
    this.#disposed = true;
    for (const pending of this.#pending.values()) {
      pending.stop();
      if (pending.type === "image.reply" || pending.type === "geometry.reply" || pending.type === "surface.reply" || pending.type === "surface.capabilities.reply")
        pending.cancel();
      pending.reject(new Error("Canvas connection closed."));
    }
    this.#pending.clear();
    this.#context = null;
    this.#surface = null;
    this.#sourceListeners.clear();
  }
  /** Translate source query failures without introducing another transport or request sequence. */
  async #sourceRequest(command) {
    try {
      const reply = await this.#request({ type: "source.request", command }, "source.reply");
      if (reply.type !== "source.reply") throw new Error("Invalid canvas source reply.");
      return reply.result;
    } catch (error) {
      return {
        kind: "failure",
        status: "unavailable",
        message: error instanceof Error ? error.message.slice(0, 512) : "Canvas source request failed."
      };
    }
  }
  /** Delegate geometry and native sizing through the existing correlated host transport. */
  async #geometryRequest(command, cancellation) {
    if (cancellation?.cancelled())
      return { kind: "failure", status: "cancelled", message: "Canvas sizing cancelled." };
    const reply = await this.#request(
      { type: "geometry.request", command },
      "geometry.reply",
      cancellation
    );
    if (reply.type !== "geometry.reply") throw new Error("Invalid canvas geometry reply.");
    return reply.result;
  }
  /** Register reply ownership before synchronous transports can answer, and bound every request. */
  #request(request, reply, cancellation) {
    if (this.#disposed) return Promise.reject(new Error("Canvas connection closed."));
    if (this.#next === 2147483647)
      return Promise.reject(new Error("Canvas request identity exhausted."));
    if (request.type.startsWith("surface.") && this.#pending.size >= 64)
      return Promise.reject(new Error("Canvas request capacity is exhausted."));
    const requestId = ++this.#next;
    return new Promise((resolve, reject) => {
      let stopDeadline = () => {
      };
      let stopCancellation = () => {
      };
      let transportOwned = false;
      let cancellationSent = false;
      const stop = () => {
        stopDeadline();
        stopCancellation();
      };
      const cancel = () => {
        if (cancellationSent) return;
        cancellationSent = true;
        this.#cancel(requestId);
      };
      this.#pending.set(requestId, { type: reply, accept: resolve, reject, stop, cancel });
      stopDeadline = this.port.later(3e4, () => {
        this.#pending.delete(requestId);
        stop();
        if (request.type === "image.insert" || request.type === "geometry.request" || request.type.startsWith("surface."))
          cancel();
        reject(new Error("Canvas request timed out; it was not retried."));
      });
      stopCancellation = cancellation?.subscribe(() => {
        const pending = this.#pending.get(requestId);
        if (!pending) return;
        if (isSurfaceRequest(request) && transportOwned) {
          stopCancellation();
          cancel();
          return;
        }
        this.#pending.delete(requestId);
        stop();
        if (isSurfaceRequest(request)) {
          resolve({
            protocol: "sugarcubes.canvas",
            version: 3,
            session: this.session,
            requestId,
            type: "surface.reply",
            address: request.address,
            revision: request.revision,
            status: "cancelled",
            message: "Canvas surface request cancelled."
          });
          return;
        }
        cancel();
        if (request.type === "geometry.request") {
          resolve({
            protocol: "sugarcubes.canvas",
            version: 3,
            session: this.session,
            requestId,
            type: "geometry.reply",
            result: { kind: "failure", status: "cancelled", message: "Canvas sizing cancelled." }
          });
          return;
        }
        resolve({
          protocol: "sugarcubes.canvas",
          version: 3,
          session: this.session,
          requestId,
          type: "image.reply",
          result: { status: "cancelled", message: "Canvas insertion cancelled." }
        });
      }) ?? (() => {
      });
      if (!this.#pending.has(requestId)) return;
      try {
        const envelope = {
          protocol: "sugarcubes.canvas",
          version: 3,
          session: this.session,
          requestId
        };
        const message = { ...envelope, ...request };
        if (request.type.startsWith("surface.") && !isCanvasMessage(message))
          throw new Error("Canvas surface request is invalid.");
        transportOwned = true;
        this.port.send(message);
      } catch (error) {
        this.#pending.delete(requestId);
        stop();
        reject(
          error instanceof Error ? error : new Error("Canvas transport failed.", { cause: error })
        );
      }
    });
  }
  /** Signal cancellation without inventing a new mutation identity. */
  #cancel(requestId) {
    try {
      this.port.send({
        protocol: "sugarcubes.canvas",
        version: 3,
        session: this.session,
        requestId,
        type: "cancel"
      });
    } catch (error) {
      this.port.report(error);
    }
  }
};
function isSurfaceRequest(request) {
  return request.type.startsWith("surface.") && request.type !== "surface.capabilities.request";
}
function snapshotCanvasContext(context) {
  return Object.freeze({
    canvas: Object.freeze({
      ...context.canvas,
      capabilities: Object.freeze({ ...context.canvas.capabilities })
    }),
    document: context.document ? Object.freeze({ ...context.document }) : null
  });
}

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

// integrations/shared/CanvasByteEncoding.ts
var ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
function encodeCanvasBytes(bytes) {
  const blocks = [];
  let block = "";
  for (let index = 0; index < bytes.length; index += 3) {
    const first = bytes[index] ?? 0, second = bytes[index + 1] ?? 0, third = bytes[index + 2] ?? 0;
    block += ALPHABET.charAt(first >> 2) + ALPHABET.charAt((first & 3) << 4 | second >> 4) + (index + 1 < bytes.length ? ALPHABET.charAt((second & 15) << 2 | third >> 6) : "=") + (index + 2 < bytes.length ? ALPHABET.charAt(third & 63) : "=");
    if (block.length >= 16384) {
      blocks.push(block);
      block = "";
    }
  }
  blocks.push(block);
  return blocks.join("");
}
function decodeCanvasBytes(data, maximumBytes) {
  if (!data.length || data.length > Math.ceil(maximumBytes / 3) * 4 || data.length % 4 !== 0)
    throw new Error("Invalid canvas byte encoding.");
  const padding = data.endsWith("==") ? 2 : data.endsWith("=") ? 1 : 0;
  const size = data.length / 4 * 3 - padding;
  if (size > maximumBytes) throw new Error("Canvas image transfer exceeds its byte budget.");
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (let index = 0; index < data.length; index += 4) {
    const a = ALPHABET.indexOf(data.charAt(index)), b = ALPHABET.indexOf(data.charAt(index + 1));
    const c = ALPHABET.indexOf(data.charAt(index + 2)), d = ALPHABET.indexOf(data.charAt(index + 3));
    const final = index + 4 === data.length;
    if (a < 0 || b < 0 || !final && (c < 0 || d < 0) || final && (padding === 0 ? c < 0 || d < 0 : padding === 1 ? c < 0 || d !== -1 || (c & 3) !== 0 : c !== -1 || d !== -1 || (b & 15) !== 0))
      throw new Error("Invalid canvas byte encoding.");
    bytes[offset++] = a << 2 | b >> 4;
    if (offset < size) bytes[offset++] = (b & 15) << 4 | c >> 2;
    if (offset < size) bytes[offset++] = (c & 3) << 6 | d;
  }
  return bytes;
}

// integrations/shared/CanvasImageEncoding.ts
var MAX_CANVAS_ENCODED_CHARACTERS = Math.ceil(MAX_CANVAS_IMAGE_BYTES / 3) * 4;
function encodeCanvasImage(bytes) {
  canvasPngDimensions(bytes);
  return { format: "png", encoding: "base64", data: encodeCanvasBytes(bytes) };
}

// integrations/shared/CanvasRasterEncoding.ts
var MAX_CANVAS_RASTER_BYTES = 64 * 1024 * 1024;
function canvasRasterByteLength(width, height, format) {
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width < 1 || height < 1 || width > 65536 || height > 65536)
    throw new Error("Invalid canvas raster geometry or format.");
  const size = width * height * (format === "rgba8" ? 4 : 1);
  if (size > MAX_CANVAS_RASTER_BYTES)
    throw new Error("The canvas raster exceeds its transfer budget.");
  return size;
}
function encodeCanvasRaster(width, height, format, bytes) {
  if (bytes.length !== canvasRasterByteLength(width, height, format))
    throw new Error("The canvas raster byte count differs from its geometry.");
  return { width, height, format, data: encodeCanvasBytes(bytes) };
}
function decodeCanvasRaster(pixels) {
  const size = canvasRasterByteLength(pixels.width, pixels.height, pixels.format);
  const bytes = decodeCanvasBytes(pixels.data, size);
  if (bytes.length !== size)
    throw new Error("The canvas raster byte count differs from its geometry.");
  return bytes;
}

// integrations/shared/CanvasImageSampling.ts
function sampleCanvasComponent(bytes, width, components, component, x, y, bounds, premultiply = false) {
  const left = Math.max(bounds.left, Math.min(bounds.left + bounds.width - 1, x));
  const top = Math.max(bounds.top, Math.min(bounds.top + bounds.height - 1, y));
  const x0 = Math.floor(left), y0 = Math.floor(top);
  const x1 = Math.min(x0 + 1, bounds.left + bounds.width - 1);
  const y1 = Math.min(y0 + 1, bounds.top + bounds.height - 1);
  const dx = left - x0, dy = top - y0;
  const value = (column, row) => {
    const offset = (row * width + column) * components;
    return (bytes[offset + component] ?? 0) * (premultiply ? (bytes[offset + 3] ?? 0) / 255 : 1);
  };
  return value(x0, y0) * (1 - dx) * (1 - dy) + value(x1, y0) * dx * (1 - dy) + value(x0, y1) * (1 - dx) * dy + value(x1, y1) * dx * dy;
}

// integrations/shared/CanvasImageRegionCrop.ts
function resolveCanvasImageRegion(image, region) {
  const frame = region.mapping.imageSize;
  if (region.outputFrame !== "vae-center-crop" || image.width > frame.width || image.height > frame.height || image.width === frame.width && image.height === frame.height)
    return region;
  return cropCanvasImageRegion(region, {
    left: Math.floor((frame.width - image.width) / 2),
    top: Math.floor((frame.height - image.height) / 2),
    width: image.width,
    height: image.height
  });
}
function cropCanvasImageRegion(region, crop) {
  const { imageSize, imageBounds: source, documentBounds: target } = region.mapping;
  if (region.mask.format !== "coverage8" || region.mask.width !== imageSize.width || region.mask.height !== imageSize.height || !Object.values(crop).every(Number.isSafeInteger) || crop.left < 0 || crop.top < 0 || crop.width <= 0 || crop.height <= 0 || crop.left + crop.width > imageSize.width || crop.top + crop.height > imageSize.height)
    throw new Error("Invalid processing frame crop.");
  const left = Math.max(source.left, crop.left), top = Math.max(source.top, crop.top);
  const right = Math.min(source.left + source.width, crop.left + crop.width);
  const bottom = Math.min(source.top + source.height, crop.top + crop.height);
  if (right <= left || bottom <= top)
    throw new Error("The processing crop removed its image support.");
  const original = decodeCanvasRaster(region.mask), mask = new Uint8Array(crop.width * crop.height);
  for (let y = 0; y < crop.height; y++) {
    const start = (crop.top + y) * imageSize.width + crop.left;
    mask.set(original.subarray(start, start + crop.width), y * crop.width);
  }
  const scaleX = target.width / source.width, scaleY = target.height / source.height;
  return Object.freeze({
    coverage: region.coverage,
    mask: Object.freeze(encodeCanvasRaster(crop.width, crop.height, "coverage8", mask)),
    mapping: Object.freeze({
      imageSize: Object.freeze({ width: crop.width, height: crop.height }),
      imageBounds: Object.freeze({
        left: left - crop.left,
        top: top - crop.top,
        width: right - left,
        height: bottom - top
      }),
      documentBounds: Object.freeze({
        left: target.left + (left - source.left) * scaleX,
        top: target.top + (top - source.top) * scaleY,
        width: (right - left) * scaleX,
        height: (bottom - top) * scaleY
      })
    })
  });
}

// integrations/shared/CanvasImageRegions.ts
function validateMapping(image, region) {
  const { mapping, mask } = region;
  const { imageSize, imageBounds, documentBounds } = mapping;
  if (image.format !== "rgba8" || mask.format !== "coverage8" || mask.width !== imageSize.width || mask.height !== imageSize.height)
    throw new Error(
      "The returned image or mask geometry does not match its captured processing frame."
    );
  for (const rectangle of [imageBounds, documentBounds]) {
    if (!Object.values(rectangle).every(Number.isFinite) || rectangle.width <= 0 || rectangle.height <= 0 || rectangle.width > 3e5 || rectangle.height > 3e5 || Math.abs(rectangle.left) > 3e5 || Math.abs(rectangle.top) > 3e5)
      throw new Error("Invalid canvas return mapping.");
  }
  if (!Object.values(imageBounds).every(Number.isSafeInteger) || imageBounds.left < 0 || imageBounds.top < 0 || imageBounds.left + imageBounds.width > imageSize.width || imageBounds.top + imageBounds.height > imageSize.height)
    throw new Error("Invalid canvas return image mapping.");
}
function coverageBounds(mask, width, bounds) {
  let left = Infinity, top = Infinity, right = -Infinity, bottom = -Infinity;
  for (let y = bounds.top; y < bounds.top + bounds.height; y++)
    for (let x = bounds.left; x < bounds.left + bounds.width; x++)
      if ((mask[y * width + x] ?? 0) > 0) {
        left = Math.min(left, x);
        top = Math.min(top, y);
        right = Math.max(right, x + 1);
        bottom = Math.max(bottom, y + 1);
      }
  return Number.isFinite(left) ? { left, top, width: right - left, height: bottom - top } : null;
}
function documentRectangle(bounds, mapping) {
  const source = mapping.imageBounds, target = mapping.documentBounds;
  const scaleX = target.width / source.width, scaleY = target.height / source.height;
  const borderX = scaleX === 1 ? 0 : 1, borderY = scaleY === 1 ? 0 : 1;
  const x0 = Math.max(source.left, bounds.left - borderX), y0 = Math.max(source.top, bounds.top - borderY);
  const x1 = Math.min(source.left + source.width, bounds.left + bounds.width + borderX);
  const y1 = Math.min(source.top + source.height, bounds.top + bounds.height + borderY);
  const left = Math.floor(target.left + (x0 - source.left) * scaleX);
  const top = Math.floor(target.top + (y0 - source.top) * scaleY);
  return {
    left,
    top,
    width: Math.ceil(target.left + (x1 - source.left) * scaleX) - left,
    height: Math.ceil(target.top + (y1 - source.top) * scaleY) - top
  };
}
function prepareCanvasImagePatch(image, region) {
  region = resolveCanvasImageRegion(image, region);
  validateMapping(image, region);
  const pixels = decodeCanvasRaster(image), mask = decodeCanvasRaster(region.mask);
  const frame = region.mapping.imageSize;
  const support = coverageBounds(mask, frame.width, region.mapping.imageBounds);
  if (!support) return null;
  const rectangle = documentRectangle(support, region.mapping);
  const result = new Uint8Array(canvasRasterByteLength(rectangle.width, rectangle.height, "rgba8"));
  const source = region.mapping.imageBounds, target = region.mapping.documentBounds;
  const scaleX = source.width / target.width, scaleY = source.height / target.height;
  const direct = scaleX === 1 && scaleY === 1 && Number.isInteger(target.left) && Number.isInteger(target.top);
  const imageScaleX = image.width / frame.width, imageScaleY = image.height / frame.height;
  const directImage = direct && imageScaleX === 1 && imageScaleY === 1;
  const imageBounds = {
    left: Math.floor(source.left * imageScaleX),
    top: Math.floor(source.top * imageScaleY),
    width: Math.ceil((source.left + source.width) * imageScaleX) - Math.floor(source.left * imageScaleX),
    height: Math.ceil((source.top + source.height) * imageScaleY) - Math.floor(source.top * imageScaleY)
  };
  let visible = false;
  for (let y = 0; y < rectangle.height; y++)
    for (let x = 0; x < rectangle.width; x++) {
      const sx = source.left + (rectangle.left + x + 0.5 - target.left) * scaleX - 0.5;
      const sy = source.top + (rectangle.top + y + 0.5 - target.top) * scaleY - 0.5;
      if (rectangle.left + x + 0.5 < target.left || rectangle.top + y + 0.5 < target.top || rectangle.left + x + 0.5 >= target.left + target.width || rectangle.top + y + 0.5 >= target.top + target.height)
        continue;
      const ix = (sx + 0.5) * imageScaleX - 0.5, iy = (sy + 0.5) * imageScaleY - 0.5;
      const sourceOffset = (iy * image.width + ix) * 4;
      const coverage = direct ? mask[sy * frame.width + sx] ?? 0 : sampleCanvasComponent(mask, frame.width, 1, 0, sx, sy, source);
      const alpha = directImage ? pixels[sourceOffset + 3] ?? 0 : sampleCanvasComponent(pixels, image.width, 4, 3, ix, iy, imageBounds);
      const opacity = Math.round(
        alpha * (region.coverage === "support" ? coverage > 0 ? 1 : 0 : coverage / 255)
      );
      if (opacity === 0) continue;
      visible = true;
      const offset = (y * rectangle.width + x) * 4;
      for (let component = 0; component < 3; component++)
        result[offset + component] = directImage ? pixels[sourceOffset + component] ?? 0 : Math.round(
          sampleCanvasComponent(pixels, image.width, 4, component, ix, iy, imageBounds, true) * 255 / alpha
        );
      result[offset + 3] = opacity;
    }
  return visible ? {
    pixels: encodeCanvasRaster(rectangle.width, rectangle.height, "rgba8", result),
    position: { left: rectangle.left, top: rectangle.top }
  } : null;
}

// integrations/shared/CanvasCancellation.ts
var CanvasCancellation = class {
  #cancelled = false;
  #listeners = /* @__PURE__ */ new Set();
  /** Read cancellation before allocating a resource or committing document changes. */
  cancelled() {
    return this.#cancelled;
  }
  /** Observe cancellation and immediately notify late subscribers to an already cancelled request. */
  subscribe(listener) {
    if (this.#cancelled) {
      listener();
      return () => {
      };
    }
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }
  /** Cancel once and retire every observer before invoking its callback. */
  cancel() {
    if (this.#cancelled) return;
    this.#cancelled = true;
    const listeners = [...this.#listeners];
    this.#listeners.clear();
    for (const listener of listeners) listener();
  }
};

// integrations/contracts/CanvasHost.ts
var WEB_CANVAS = Object.freeze({
  id: "web",
  label: "SugarCubes Web",
  capabilities: Object.freeze({
    displayOutput: true,
    insertImageLayer: false,
    positionImageLayer: false
  })
});

// integrations/shared/CanvasMasksValidation.generated.ts
function isRecord2(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function isCanvasDocument2(value) {
  return isRecord2(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 128 && Object.hasOwn(value, "geometryRevision") && typeof value["geometryRevision"] === "string" && value["geometryRevision"].length <= 512 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isSafeInteger(value["width"]) && value["width"] >= 1 && value["width"] <= 3e5 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isSafeInteger(value["height"]) && value["height"] >= 1 && value["height"] <= 3e5;
}
function isCanvasMaskPixels(value) {
  return isRecord2(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "width") && typeof value["width"] === "number" && Number.isSafeInteger(value["width"]) && value["width"] >= 1 && value["width"] <= 65536 && Object.hasOwn(value, "height") && typeof value["height"] === "number" && Number.isSafeInteger(value["height"]) && value["height"] >= 1 && value["height"] <= 65536 && Object.hasOwn(value, "data") && typeof value["data"] === "string" && value["data"].length <= 89478488;
}
function isCanvasMaskReference(value) {
  return isRecord2(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "canvasId") && typeof value["canvasId"] === "string" && value["canvasId"].length <= 128 && Object.hasOwn(value, "document") && isCanvasDocument2(value["document"]) && Object.hasOwn(value, "id") && typeof value["id"] === "string" && value["id"].length <= 128;
}
function isCanvasMaskSnapshot(value) {
  return isRecord2(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "mask") && isCanvasMaskReference(value["mask"]) && Object.hasOwn(value, "revision") && typeof value["revision"] === "string" && value["revision"].length <= 512 && Object.hasOwn(value, "pixels") && isCanvasMaskPixels(value["pixels"]);
}
function isCanvasMaskContext(value) {
  return isRecord2(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "kind") && value["kind"] === "context" && Object.hasOwn(value, "supported") && typeof value["supported"] === "boolean" && Object.hasOwn(value, "document") && (isCanvasDocument2(value["document"]) || value["document"] === null) && Object.hasOwn(value, "observesChanges") && typeof value["observesChanges"] === "boolean";
}
function isCanvasMaskCreate(value) {
  return isRecord2(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "type") && value["type"] === "create" && Object.hasOwn(value, "document") && isCanvasDocument2(value["document"]) && Object.hasOwn(value, "name") && typeof value["name"] === "string" && value["name"].length <= 128 && Object.hasOwn(value, "source") && (value["source"] === "selection" || value["source"] === "blank" || value["source"] === "pixels") && Object.hasOwn(value, "pixels") && (isCanvasMaskPixels(value["pixels"]) || value["pixels"] === null);
}
function isCanvasMaskActivate(value) {
  return isRecord2(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "type") && value["type"] === "activate" && Object.hasOwn(value, "mask") && isCanvasMaskReference(value["mask"]);
}
function isCanvasMaskCapture(value) {
  return isRecord2(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "type") && value["type"] === "capture" && Object.hasOwn(value, "masks") && Array.isArray(value["masks"]) && value["masks"].length <= 64 && value["masks"].every((item) => isCanvasMaskReference(item)) && Object.hasOwn(value, "maximumSize") && typeof value["maximumSize"] === "number" && Number.isSafeInteger(value["maximumSize"]) && value["maximumSize"] >= 0 && value["maximumSize"] <= 1024;
}
function isCanvasMaskCommand(value) {
  return isCanvasMaskQuery(value) || isRecord2(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "enabled") && typeof value["enabled"] === "boolean" && Object.hasOwn(value, "type") && value["type"] === "watch";
}
function isCanvasMaskResult(value) {
  return isCanvasMaskContext(value) || isRecord2(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "created" && Object.hasOwn(value, "mask") && isCanvasMaskReference(value["mask"]) || isRecord2(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "kind") && value["kind"] === "activated" || isRecord2(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "kind") && value["kind"] === "captured" && Object.hasOwn(value, "snapshots") && Array.isArray(value["snapshots"]) && value["snapshots"].length <= 64 && value["snapshots"].every((item) => isCanvasMaskSnapshot(item)) || isRecord2(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "kind") && value["kind"] === "failure" && Object.hasOwn(value, "status") && (value["status"] === "unavailable" || value["status"] === "unsupported" || value["status"] === "stale" || value["status"] === "cancelled" || value["status"] === "error") && Object.hasOwn(value, "message") && typeof value["message"] === "string" && value["message"].length <= 512 || isRecord2(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "supported") && typeof value["supported"] === "boolean" && Object.hasOwn(value, "kind") && value["kind"] === "watching";
}
function isCanvasMaskRequest(value) {
  return isRecord2(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas.masks" && Object.hasOwn(value, "version") && value["version"] === 2 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "command") && isCanvasMaskCommand(value["command"]);
}
function isCanvasMaskReply(value) {
  return isRecord2(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas.masks" && Object.hasOwn(value, "version") && value["version"] === 2 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "requestId") && typeof value["requestId"] === "number" && Number.isSafeInteger(value["requestId"]) && value["requestId"] >= 1 && value["requestId"] <= 2147483647 && Object.hasOwn(value, "result") && isCanvasMaskResult(value["result"]);
}
function isCanvasMaskChanged(value) {
  return isRecord2(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.canvas.masks" && Object.hasOwn(value, "version") && value["version"] === 2 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "changed" && Object.hasOwn(value, "documentId") && typeof value["documentId"] === "string" && value["documentId"].length <= 128 && Object.hasOwn(value, "revision") && typeof value["revision"] === "string" && value["revision"].length <= 512 && Object.hasOwn(value, "maskId") && typeof value["maskId"] === "string" && value["maskId"].length <= 128;
}
function isCanvasMaskQuery(value) {
  return isRecord2(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "type") && value["type"] === "context" || isCanvasMaskCreate(value) || isCanvasMaskActivate(value) || isCanvasMaskCapture(value) || isRecord2(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "type") && value["type"] === "release" && Object.hasOwn(value, "mask") && (isCanvasMaskReference(value["mask"]) || value["mask"] === null);
}
function isCanvasMaskMessage(value) {
  return isCanvasMaskRequest(value) || isCanvasMaskReply(value) || isCanvasMaskChanged(value);
}

// integrations/shared/CanvasMaskClient.ts
var CanvasMaskClient = class {
  /** Bind a single trusted navigation and its runtime clock. */
  constructor(session, port, identity = () => "") {
    this.session = session;
    this.port = port;
    this.identity = identity;
    this.changes = new CanvasRemoteChanges({
      listen: (listener) => {
        this.#listeners.add(listener);
        return () => {
          this.#listeners.delete(listener);
        };
      },
      watch: async (enabled) => {
        const result = await this.#request({ type: "watch", enabled });
        if (result.kind !== "watching")
          throw new Error(
            result.kind === "failure" ? result.message : "Invalid mask observation acknowledgement."
          );
        return result.supported;
      },
      report: (error) => {
        this.port.report(error);
      }
    });
  }
  changes;
  #pending = /* @__PURE__ */ new Map();
  #listeners = /* @__PURE__ */ new Set();
  #next = 0;
  #disposed = false;
  /** Read the negotiated canvas identity without guessing from transport or editor names. */
  get canvasId() {
    return this.identity();
  }
  /** Submit one capability operation and retire it on timeout or connection loss. */
  execute(command) {
    return this.#request(command);
  }
  /** Keep observation requests in the same correlated sequence as native mask operations. */
  #request(command) {
    if (this.#disposed)
      return Promise.resolve({
        kind: "failure",
        status: "unavailable",
        message: "Canvas mask connection closed."
      });
    const requestId = ++this.#next;
    return new Promise((resolve) => {
      const stop = this.port.later(3e4, () => {
        this.#pending.delete(requestId);
        resolve({
          kind: "failure",
          status: "unavailable",
          message: "Canvas mask operation timed out; it was not retried."
        });
      });
      this.#pending.set(requestId, { resolve, stop });
      try {
        this.port.send({
          protocol: "sugarcubes.canvas.masks",
          version: 2,
          session: this.session,
          requestId,
          command
        });
      } catch (error) {
        this.#pending.delete(requestId);
        stop();
        resolve({
          kind: "failure",
          status: "unavailable",
          message: error instanceof Error ? error.message.slice(0, 512) : "Canvas mask transport failed."
        });
      }
    });
  }
  /** Validate the complete bounded envelope before exposing any native value. */
  receive(value) {
    if (this.#disposed || !isCanvasMaskMessage(value) || value.session !== this.session)
      return false;
    if ("type" in value) {
      for (const listener of this.#listeners) listener(value);
      return true;
    }
    if (!("result" in value)) return false;
    const pending = this.#pending.get(value.requestId);
    if (!pending) return false;
    this.#pending.delete(value.requestId);
    pending.stop();
    pending.resolve(value.result);
    return true;
  }
  /** Settle outstanding operations before releasing native-change consumers. */
  dispose() {
    this.changes.dispose();
    this.#disposed = true;
    for (const pending of this.#pending.values()) {
      pending.stop();
      pending.resolve({
        kind: "failure",
        status: "unavailable",
        message: "Canvas mask connection closed."
      });
    }
    this.#pending.clear();
    this.#listeners.clear();
  }
};

// integrations/shared/CanvasMaskPixels.ts
var MAX_MASK_PIXELS = 64 * 1024 * 1024;
function encodeMaskPixels(width, height, bytes) {
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width < 1 || height < 1 || width > 65536 || height > 65536 || width * height !== bytes.length || bytes.length > MAX_MASK_PIXELS)
    throw new Error("Invalid or oversized mask coverage.");
  return { width, height, data: encodeCanvasBytes(bytes) };
}
function decodeMaskPixels(pixels) {
  const size = pixels.width * pixels.height;
  if (!Number.isSafeInteger(pixels.width) || !Number.isSafeInteger(pixels.height) || pixels.width < 1 || pixels.height < 1 || pixels.width > 65536 || pixels.height > 65536 || size > MAX_MASK_PIXELS)
    throw new Error("Invalid or oversized mask coverage.");
  const bytes = decodeCanvasBytes(pixels.data, MAX_MASK_PIXELS);
  if (bytes.length !== size) throw new Error("Invalid mask coverage dimensions.");
  return bytes;
}

// integrations/shared/CanvasSourceIdentity.ts
function canvasSourceTargetKey(target) {
  return JSON.stringify([
    target.kind,
    target.kind === "layer" || target.kind === "composition-from-layer" ? target.layerId : null
  ]);
}
function canvasInputSourceKey(source) {
  return source.kind === "file" ? JSON.stringify(["file", source.file]) : JSON.stringify([
    "canvas",
    source.canvasId,
    source.documentId,
    canvasSourceTargetKey(source.target)
  ]);
}
function sameCanvasSourceDocument(first, second) {
  return first.id === second.id && first.geometryRevision === second.geometryRevision && first.width === second.width && first.height === second.height;
}

// integrations/shared/CanvasDimensions.ts
function constrainCanvasDimension(value, limits) {
  if (![limits.minimum, limits.maximum, limits.multiple].every(
    (limit) => Number.isSafeInteger(limit) && limit > 0
  ))
    throw new Error("Dimension limits must be positive integer pixel values.");
  const minimum = Math.ceil(limits.minimum / limits.multiple) * limits.multiple;
  const maximum = Math.floor(limits.maximum / limits.multiple) * limits.multiple;
  if (!Number.isFinite(value) || value <= 0 || minimum > maximum)
    throw new Error("These dimensions cannot satisfy the field limits.");
  return Math.min(
    maximum,
    Math.max(minimum, Math.round(value / limits.multiple) * limits.multiple)
  );
}
function canvasSizingProposal(request, scale = request.scale) {
  if (scale.length > 32 || !/^[0-9]+(?:\.[0-9]+)?$/.test(scale))
    throw new Error("Enter a positive decimal scale factor of at most 32 characters.");
  const factor = Number(scale);
  if (!Number.isFinite(factor) || factor <= 0 || factor > 1e3)
    throw new Error("Enter a scale factor greater than zero and no greater than 1000.");
  const source = request.direction === "to-canvas" ? request.generation : request.reference;
  const multiplier = request.direction === "to-canvas" ? factor : 1 / factor;
  return {
    width: constrainCanvasDimension(source.width * multiplier, request.widthLimits),
    height: constrainCanvasDimension(source.height * multiplier, request.heightLimits)
  };
}
export {
  CanvasCancellation,
  CanvasClient,
  CanvasMaskClient,
  MAX_CANVAS_IMAGE_BYTES,
  MAX_CANVAS_IMAGE_PIXELS,
  MAX_CANVAS_RASTER_BYTES,
  WEB_CANVAS,
  canvasInputSourceKey,
  canvasPngDimensions,
  canvasRasterByteLength,
  canvasSizingProposal,
  canvasSourceTargetKey,
  constrainCanvasDimension,
  cropCanvasImageRegion,
  decodeCanvasRaster,
  decodeMaskPixels,
  encodeCanvasImage,
  encodeCanvasRaster,
  encodeMaskPixels,
  isCanvasInputSource,
  isCanvasInputTarget,
  isCanvasMaskReference,
  isCanvasPixelRaster,
  isCanvasSurfaceGeometry,
  prepareCanvasImagePatch,
  resolveCanvasImageRegion,
  sameCanvasSourceDocument
};
