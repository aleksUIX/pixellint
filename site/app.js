import init, {
  rulepacks,
  validate,
  vendors,
  version,
} from "./wasm/pixellint.js";

const artifactInput = document.querySelector("#artifact");
const kindSelect = document.querySelector("#kind");
const stateSelect = document.querySelector("#state");
const validateButton = document.querySelector("#validate");
const resultsSection = document.querySelector("#results");

await init();

document.querySelector("#version").textContent = `pixellint ${version()}`;
renderCoverage();

validateButton.addEventListener("click", run);
artifactInput.addEventListener("keydown", (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
    run();
  }
});

for (const button of document.querySelectorAll(".example")) {
  button.addEventListener("click", () => {
    artifactInput.value = button.dataset.artifact;
    // Examples carry their own kind, since a conversion API body is not a URL.
    kindSelect.value = button.dataset.kind ?? "url";
    run();
  });
}

function run() {
  const artifact = artifactInput.value.trim();
  resultsSection.replaceChildren();

  if (!artifact) {
    return;
  }

  let summary;
  try {
    summary = validate(kindSelect.value, artifact, stateSelect.value, undefined);
  } catch (error) {
    resultsSection.append(element("p", { class: "muted" }, String(error)));
    return;
  }

  renderSummary(artifact, summary);
}

function renderSummary(artifact, summary) {
  const findings = summary.reports.flatMap((report) => report.violations);
  const tally = { error: 0, warning: 0, info: 0 };
  for (const finding of findings) {
    tally[finding.severity] += 1;
  }

  const verdict = element(
    "div",
    { class: "verdict" },
    element(
      "span",
      { class: `severity ${tally.error ? "error" : "info"}` },
      tally.error ? "Errors present" : "No errors",
    ),
    element(
      "span",
      { class: "tally" },
      `${tally.error} error, ${tally.warning} warning, ${tally.info} info across ${summary.reports.length} rulepack(s)`,
    ),
  );
  resultsSection.append(verdict);

  for (const report of summary.reports) {
    const header = element(
      "h3",
      {},
      element("span", {}, report.plugin_id),
      report.detected_vendor
        ? element("span", { class: "badge" }, `vendor: ${report.detected_vendor}`)
        : null,
    );

    const card = element(
      "article",
      { class: `report${report.violations.length ? "" : " clean"}` },
      header,
    );

    for (const violation of report.violations) {
      card.append(renderFinding(artifact, violation));
    }

    resultsSection.append(card);
  }
}

function renderFinding(artifact, violation) {
  const node = element(
    "div",
    { class: "finding" },
    element(
      "div",
      { class: "finding-head" },
      element("span", { class: `severity ${violation.severity}` }, violation.severity),
      element("span", { class: "code" }, violation.code),
    ),
    element("p", {}, violation.message),
  );

  if (violation.fix_hint) {
    node.append(element("p", { class: "fix" }, `Fix: ${violation.fix_hint}`));
  }

  const target = (violation.targets ?? []).find(
    (candidate) => candidate.end > candidate.start && candidate.end <= artifact.length,
  );
  if (target) {
    node.append(
      element(
        "div",
        { class: "target" },
        document.createTextNode(artifact.slice(0, target.start)),
        element("mark", {}, artifact.slice(target.start, target.end)),
        document.createTextNode(artifact.slice(target.end)),
      ),
    );
  }

  const reference = violation.source?.reference;
  const cite = element(
    "p",
    { class: "cite" },
    `${labelEvidence(violation.source?.level)} · `,
  );
  if (reference) {
    cite.append(element("a", { href: reference, rel: "noreferrer" }, "source"));
  } else {
    cite.append(document.createTextNode(violation.source?.name ?? ""));
  }
  node.append(cite);

  return node;
}

function labelEvidence(level) {
  return (
    {
      normative: "formal standard",
      official_vendor: "vendor documented",
      official_template: "vendor template",
      ecosystem_reference: "ecosystem evidence",
      heuristic: "heuristic",
    }[level] ?? "unknown evidence"
  );
}

function renderCoverage() {
  const packs = rulepacks();
  document.querySelector("#rulepack-count").textContent = `(${packs.length})`;
  const packList = document.querySelector("#rulepacks");
  for (const pack of packs) {
    packList.append(
      element(
        "li",
        {},
        element("div", { class: "id" }, pack.id),
        element("div", { class: "muted" }, `${pack.display_name} · ${labelEvidence(pack.source_level)}`),
      ),
    );
  }

  const directory = vendors();
  document.querySelector("#vendor-count").textContent = `(${directory.length})`;
  const vendorList = document.querySelector("#vendors");
  const filter = document.querySelector("#vendor-filter");

  const draw = () => {
    const needle = filter.value.trim().toLowerCase();
    const matches = directory.filter(
      (entry) =>
        !needle ||
        entry.vendor.includes(needle) ||
        entry.display_name.toLowerCase().includes(needle) ||
        entry.category.includes(needle) ||
        entry.hosts.some((host) => host.includes(needle)),
    );

    vendorList.replaceChildren(
      ...matches.map((entry) =>
        element(
          "li",
          {},
          element(
            "div",
            {},
            element("span", { class: "id" }, entry.display_name),
            element("span", { class: "muted" }, ` · ${entry.category}`),
            entry.rulepack ? element("span", { class: "badge" }, entry.rulepack) : null,
          ),
          element("div", { class: "hosts" }, entry.hosts.join(", ")),
        ),
      ),
    );
  };

  filter.addEventListener("input", draw);
  draw();
}

function element(tag, attributes = {}, ...children) {
  const node = document.createElement(tag);

  for (const [name, value] of Object.entries(attributes)) {
    node.setAttribute(name, value);
  }

  for (const child of children) {
    if (child === null || child === undefined) {
      continue;
    }
    node.append(child);
  }

  return node;
}
