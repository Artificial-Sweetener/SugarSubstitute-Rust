// integrations/shared/UpdatesValidation.generated.ts
function isRecord(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function isUpdatesChannel(value) {
  return value === "stable" || value === "beta";
}
function isUpdatesFailure(value) {
  return value === "check" || value === "update";
}
function isUpdatesPhase(value) {
  return value === "check" || value === "download" || value === "verify" || value === "install" || value === "start";
}
function isUpdatesScheduledRestart(value) {
  return value === "none" || value === "after-workflow";
}
function isUpdatesRejection(value) {
  return value === "busy" || value === "not-ready" || value === "unavailable";
}
function isUpdatesCheck(value) {
  return isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "completedAt") && typeof value["completedAt"] === "number" && Number.isSafeInteger(value["completedAt"]) && value["completedAt"] >= 0 && value["completedAt"] <= 9007199254740991;
}
function isUpdatesStatus(value) {
  return isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "state") && value["state"] === "up-to-date" || isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "state") && value["state"] === "checking" || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "state") && value["state"] === "available" && Object.hasOwn(value, "version") && typeof value["version"] === "string" && value["version"].length <= 64 && new RegExp("^[a-z0-9.-]{1,64}$").test(value["version"]) || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "state") && value["state"] === "ready" && Object.hasOwn(value, "version") && typeof value["version"] === "string" && value["version"].length <= 64 && new RegExp("^[a-z0-9.-]{1,64}$").test(value["version"]) || isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "state") && value["state"] === "failed" && Object.hasOwn(value, "during") && isUpdatesFailure(value["during"]);
}
function isUpdatesCause(value) {
  return isRecord(value) && Object.keys(value).length === 1 && Object.hasOwn(value, "message") && typeof value["message"] === "string" && value["message"].length <= 2048 && Array.from(value["message"]).length <= 1024 && new RegExp("^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$").test(
    value["message"]
  );
}
function isUpdatesReport(value) {
  return isRecord(value) && Object.keys(value).length === 3 && Object.hasOwn(value, "version") && typeof value["version"] === "string" && value["version"].length <= 64 && new RegExp("^[a-z0-9.-]{1,64}$").test(value["version"]) && Object.hasOwn(value, "phase") && isUpdatesPhase(value["phase"]) && Object.hasOwn(value, "causes") && Array.isArray(value["causes"]) && value["causes"].length >= 1 && value["causes"].length <= 8 && value["causes"].every((item) => isUpdatesCause(item));
}
function isUpdatesState(value) {
  return isRecord(value) && Object.keys(value).length === 6 + (Object.hasOwn(value, "lastSuccessfulCheck") ? 1 : 0) + (Object.hasOwn(value, "report") ? 1 : 0) && Object.hasOwn(value, "currentVersion") && typeof value["currentVersion"] === "string" && value["currentVersion"].length <= 64 && new RegExp("^[a-z0-9.-]{1,64}$").test(value["currentVersion"]) && Object.hasOwn(value, "channel") && isUpdatesChannel(value["channel"]) && (!Object.hasOwn(value, "lastSuccessfulCheck") || isUpdatesCheck(value["lastSuccessfulCheck"])) && Object.hasOwn(value, "status") && isUpdatesStatus(value["status"]) && (!Object.hasOwn(value, "report") || isUpdatesReport(value["report"])) && Object.hasOwn(value, "previousVersionRunning") && typeof value["previousVersionRunning"] === "boolean" && Object.hasOwn(value, "workflowRunning") && typeof value["workflowRunning"] === "boolean" && Object.hasOwn(value, "scheduledRestart") && isUpdatesScheduledRestart(value["scheduledRestart"]);
}
function isUpdatesStateMessage(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.updates" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "state" && Object.hasOwn(value, "revision") && typeof value["revision"] === "number" && Number.isSafeInteger(value["revision"]) && value["revision"] >= 1 && value["revision"] <= 9007199254740991 && Object.hasOwn(value, "update") && isUpdatesState(value["update"]);
}
function isUpdatesRejectedMessage(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.updates" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "rejected" && Object.hasOwn(value, "request") && typeof value["request"] === "string" && value["request"].length <= 48 && new RegExp("^[a-z0-9.-]{1,48}$").test(value["request"]) && Object.hasOwn(value, "reason") && isUpdatesRejection(value["reason"]);
}
function isUpdatesAcceptedMessage(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.updates" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "accepted" && Object.hasOwn(value, "request") && typeof value["request"] === "string" && value["request"].length <= 48 && new RegExp("^[a-z0-9.-]{1,48}$").test(value["request"]);
}
function isUpdatesHostMessage(value) {
  return isUpdatesStateMessage(value) || isUpdatesAcceptedMessage(value) || isUpdatesRejectedMessage(value);
}
export {
  isUpdatesHostMessage
};
