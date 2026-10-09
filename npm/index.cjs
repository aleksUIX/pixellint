/**
 * Pixellint: validator for pixels, postbacks, conversion API payloads, and tracking URLs.
 * CommonJS entry point backed by the pixellint-core WASM build.
 */

const wasm = require("./wasm/pixellint.js");

function validate(artifact, options = {}) {
  const { kind = "url", state, vendor } = options;
  return wasm.validate(kind, artifact, state, vendor);
}

function validateMany(document) {
  return wasm.validate_many(JSON.stringify(document));
}

/** Extract local HAR 1.2 requests. Returned captures can contain credentials. */
function importHar(har, options = {}) {
  const raw = typeof har === "string" ? har : JSON.stringify(har);
  return wasm.import_har(raw, options.headerPolicy ?? "unknown");
}

/** Replay local HAR requests at capture timestamps, or an explicit Unix clock. */
function validateHar(har, options = {}) {
  const raw = typeof har === "string" ? har : JSON.stringify(har);
  return wasm.validate_har(raw, options.headerPolicy ?? "unknown", options.at);
}

function isOk(summary) {
  return summary.reports.every((report) =>
    report.violations.every((violation) => violation.severity !== "error"),
  );
}

module.exports = {
  validate,
  validateMany,
  importHar,
  validateHar,
  isOk,
  rulepacks: () => wasm.rulepacks(),
  vendors: () => wasm.vendors(),
  vendorForHost: (host) => wasm.vendor_for_host(host),
  version: () => wasm.version(),
};
