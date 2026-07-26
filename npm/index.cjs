/**
 * Pixellint: validator for pixels, postbacks, and tracking URLs.
 * CommonJS entry point backed by the pixellint-core WASM build.
 */

const wasm = require("./wasm/pixellint.js");

function validate(artifact, options = {}) {
  const { kind = "url", state, vendor } = options;
  return wasm.validate(kind, artifact, state, vendor);
}

function isOk(summary) {
  return summary.reports.every((report) =>
    report.violations.every((violation) => violation.severity !== "error"),
  );
}

module.exports = {
  validate,
  isOk,
  rulepacks: () => wasm.rulepacks(),
  vendors: () => wasm.vendors(),
  vendorForHost: (host) => wasm.vendor_for_host(host),
  version: () => wasm.version(),
};
