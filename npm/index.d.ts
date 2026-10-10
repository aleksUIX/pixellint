export type ArtifactKind =
  | "url"
  | "html"
  | "js"
  | "gtm"
  | "request"
  | "vast"
  | "postback"
  | "json"
  | "unknown";

export type ExpansionState = "unknown" | "template" | "fired";

/** Serialize this capture with JSON.stringify and validate with kind: request. */
export interface HttpRequest {
  url: string;
  method: string;
  headers: Record<string, string> | Array<{ name: string; value: string }>;
  body?: string;
  body_base64?: never;
  /** Omitted capture fields remain unvalidated. Absent metadata retains complete-capture semantics. */
  capture?: HttpCaptureContext;
}

/** Original binary wire bytes in canonical padded standard Base64. */
export interface BinaryHttpRequest extends Omit<HttpRequest, "body" | "body_base64"> {
  body_base64: string;
  body?: never;
}

export type HttpCapture = HttpRequest | BinaryHttpRequest;

export type Severity = "error" | "warning" | "info";

export type EvidenceLevel =
  | "normative"
  | "official_vendor"
  | "official_template"
  | "ecosystem_reference"
  | "heuristic";

export interface RuleSource {
  level: EvidenceLevel;
  name: string;
  reference: string | null;
}

export interface ViolationTarget {
  component: string;
  name: string | null;
  value: string | null;
  start: number;
  end: number;
}

export interface Violation {
  code: string;
  message: string;
  severity: Severity;
  field: string | null;
  fix_hint: string | null;
  source: RuleSource;
  targets?: ViolationTarget[];
}

export interface ValidationReport {
  plugin_id: string;
  detected_vendor: string | null;
  violations: Violation[];
}

export interface ValidationSummary {
  reports: ValidationReport[];
}

export interface RulePackMetadata {
  id: string;
  display_name: string;
  version: string;
  description: string;
  source_level: EvidenceLevel;
  vendor: string | null;
}

export interface VendorEntry {
  vendor: string;
  display_name: string;
  category: string;
  hosts: string[];
  rulepack?: string | null;
}

export interface ValidateOptions {
  /** Artifact kind. Defaults to "url". */
  kind?: ArtifactKind;
  /** Whether macros are still unexpanded. Defaults to "unknown". */
  state?: ExpansionState;
  /** Vendor the caller believes the artifact belongs to. */
  vendor?: string;
}

/** Validate a measurement artifact against every applicable rulepack. */
export function validate(artifact: string, options?: ValidateOptions): ValidationSummary;

export interface FindingCounts {
  artifacts_total?: number;
  unique_artifacts?: number;
  errors: number;
  warnings: number;
  infos: number;
}

export interface ArtifactOccurrence {
  occurrence_id?: string | null;
  source_kind?: string | null;
  path?: string | null;
  line?: number | null;
  column?: number | null;
  context_label?: string | null;
}

export interface AggregatedArtifact {
  artifact_id: string;
  dedupe_key: string;
  artifact_kind: ArtifactKind;
  raw_artifact: string;
  normalized_artifact: string;
  ok: boolean;
  summary: FindingCounts;
  reports: ValidationReport[];
  occurrences: ArtifactOccurrence[];
}

export interface DocumentReport {
  document_kind: string;
  extractor?: { id: string; version?: string | null };
  summary: FindingCounts;
  artifacts: AggregatedArtifact[];
}

/** Validate extracted artifacts as one document. The caller already extracted URLs. */
export function validateMany(document: object | string[]): DocumentReport;

/** True when no error-severity finding is present. */
export function isOk(summary: ValidationSummary): boolean;

/** Rulepacks this build ships, with their evidence levels. */
export function rulepacks(): RulePackMetadata[];

/** The vendor endpoint directory. */
export function vendors(): VendorEntry[];

/** Attribute a host to a vendor, or null when the host is unknown. */
export function vendorForHost(host: string): VendorEntry | null;

/** The pixellint-core version this build wraps. */
export function version(): string;

export type HarBodyAvailability = "available" | "absent" | "unavailable" | "redacted";
export type HarHeaderPolicy = "unknown" | "complete" | "chrome_sanitized";

export interface HttpCaptureContext {
  headers_unavailable?: boolean;
  unavailable_headers?: string[];
  redacted_headers?: string[];
  body?: HarBodyAvailability;
}

export interface HarOptions {
  /** unknown assumes omitted Authorization/Cookie may have been removed by the exporter. */
  headerPolicy?: HarHeaderPolicy;
}

export interface HarValidateOptions extends HarOptions {
  /** Safe integer Unix seconds. Overrides every HAR capture timestamp. */
  at?: number;
}

export interface DocumentArtifactInput {
  artifact_kind?: ArtifactKind;
  artifact: string;
  claimed_vendor?: string | null;
  expansion_state?: ExpansionState;
  occurrences?: ArtifactOccurrence[];
}

export interface DocumentRequest {
  document_kind: string;
  extractor?: { id: string; version?: string | null };
  artifacts: DocumentArtifactInput[];
}

export interface HarEntryMetadata {
  entry_index: number;
  occurrence_id: string;
  path: string;
  started_date_time: string | null;
  reference_time_unix_seconds: number | null;
  page_ref: string | null;
  body_size: number | null;
  body_availability: HarBodyAvailability;
  body_reason: string | null;
  post_data: object | null;
  capture: HttpCaptureContext;
}

export interface HarImport {
  document: DocumentRequest;
  header_policy: HarHeaderPolicy;
  entries: HarEntryMetadata[];
}

export interface HarReport extends DocumentReport {
  header_policy: HarHeaderPolicy;
  reference_time_override: number | null;
  captures: HarEntryMetadata[];
}

/** Import local HAR requests. This does not fetch URLs or sanitize the returned private captures. */
export function importHar(har: string | object, options?: HarOptions): HarImport;

/** Validate local HAR requests at their recorded clocks. Missing clocks require an explicit at override. */
export function validateHar(har: string | object, options?: HarValidateOptions): HarReport;

export interface SessionGroup { session_id: string; artifact_indexes: number[]; }
export interface SessionRequest { document: DocumentRequest; sessions: SessionGroup[]; }
export type SessionRuleKind = "consistent_parameter" | "unique_parameter_across_sessions";
export type SessionEvaluationStatus = "evaluated" | "partially_evaluated" | "not_evaluated";
export type SessionSkipReason = "pack_not_selected" | "unsupported_artifact" | "invalid_url" | "endpoint_mismatch" | "missing_parameter" | "empty_parameter" | "installation_placeholder" | "unresolved_macro" | "conflicting_values" | "invalid_percent_encoding" | "invalid_utf8";
export interface SessionQuerySpan { component: "query_param"; name: string; value: string; start: number; end: number; }
export interface SessionArtifactTarget { artifact_index: number; artifact_id: string; session_id: string; query_spans: SessionQuerySpan[]; occurrences: ArtifactOccurrence[]; }
export interface SessionFinding extends Omit<Violation, "targets"> { plugin_id: string; detected_vendor: string | null; session_ids: string[]; targets: SessionArtifactTarget[]; }
export interface SessionSkippedArtifact { artifact_index: number; artifact_id: string; session_id: string; reason: SessionSkipReason; occurrences: ArtifactOccurrence[]; }
export interface SessionCheckCoverage { plugin_id: string; code: string; kind: SessionRuleKind; session_ids: string[]; status: SessionEvaluationStatus; participants_total: number; compared_total: number; reason?: "insufficient_observations"; skipped: SessionSkippedArtifact[]; }
export interface SessionCoverage { status: SessionEvaluationStatus; sessions_total: number; grouped_artifacts: number; ungrouped_artifact_indexes: number[]; profiles_total: number; checks_evaluated: number; checks_partially_evaluated: number; checks_not_evaluated: number; }
export interface SessionReport { document: DocumentReport; summary: FindingCounts; relationship_summary: FindingCounts; findings: SessionFinding[]; checks: SessionCheckCoverage[]; coverage: SessionCoverage; }
export interface SessionValidateOptions { at?: number; rulepacks?: string[]; exceptRulepacks?: string[]; }
/** Explicit grouping only. Show coverage with combined finding counts. */
export function validateSessions(request: SessionRequest | string, options?: SessionValidateOptions): SessionReport;
