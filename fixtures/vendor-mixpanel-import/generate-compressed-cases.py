#!/usr/bin/env python3
"""Produce synthetic captured bytes and static source-derived expectations.

This script does not call Pixellint. Python gzip/zlib produce the bytes;
verify-compression-oracle.mjs checks them independently with Node zlib.
"""

import base64
import copy
import gzip
import hashlib
import json
from pathlib import Path
import zlib

ROOT = Path(__file__).resolve().parent
RFC = "https://www.rfc-editor.org/rfc/rfc1952"
IMPORT = "https://docs.mixpanel.com/reference/import-events"
BASE64 = "https://www.rfc-editor.org/rfc/rfc4648"
CORE = "core.request."
VENDOR = "vendor.mixpanel-import."
CLOCK = 1791600000
EVENT = {"event": "Signup", "properties": {"time": 1770000000, "distinct_id": "source-user", "$insert_id": "event-1"}}
CASES = []
ORACLES = []


def entity(events=None):
    return json.dumps([EVENT] if events is None else events, ensure_ascii=False, separators=(",", ":")).encode()


def compress(data):
    return gzip.compress(data, mtime=0)


def capture(wire, mime="application/json", encoding="gzip"):
    headers = {"Content-Type": mime, "Authorization": "Basic " + base64.b64encode(b"TEST_ONLY_PROJECT_TOKEN:").decode()}
    if encoding is not None:
        headers["Content-Encoding"] = encoding
    return {"url": "https://api.mixpanel.com/import?strict=1", "method": "POST", "headers": headers, "body_base64": base64.b64encode(wire).decode()}


def finding(code, severity="error"):
    return {"code": code, "severity": severity}


def case(name, request, expected=(), data=None, source=RFC, gzip_valid=None):
    artifact = request if isinstance(request, str) else json.dumps(request, separators=(",", ":"))
    CASES.append({"id": name, "source_case": name, "kind": "request", "artifact": artifact,
                  "expected_findings": list(expected), "reference_unix_seconds": CLOCK,
                  "rulepacks": ["core", "vendor/mixpanel-import"], "source_url": source,
                  "evidence_kind": "independent_python_bytes_and_static_source_contract"})
    if data is not None:
        encoded = request["body_base64"]
        wire = base64.b64decode(encoded, validate=True)
        ORACLES.append({"id": name, "body_base64": encoded, "wire_bytes": len(wire),
                        "wire_sha256": hashlib.sha256(wire).hexdigest(), "entity_bytes": len(data),
                        "entity_sha256": hashlib.sha256(data).hexdigest(), "gzip": gzip_valid if gzip_valid is not None else request["headers"].get("Content-Encoding", "identity").lower() == "gzip"})


def bad_gzip(name, wire):
    case(name, capture(wire), [finding(CORE + "invalid_gzip_body")])


def generate():
    good = entity()
    gz = compress(good)
    case("gzip_json_python", capture(gz), data=good, source=IMPORT)
    nd = json.dumps(EVENT, separators=(",", ":")).encode() + b"\r\n\n" + json.dumps({**EVENT, "properties": {**EVENT["properties"], "$insert_id": "event-2"}}, separators=(",", ":")).encode() + b"\n"
    case("gzip_ndjson", capture(compress(nd), "application/x-ndjson"), data=nd, source=IMPORT)
    unicode = entity([{**EVENT, "event": "Caf\u00e9 \U0001f600"}])
    case("gzip_unicode", capture(compress(unicode)), data=unicode)
    split = unicode.index("\u00e9".encode()) + 1
    case("gzip_members_split_unicode", capture(compress(unicode[:split]) + compress(unicode[split:])), data=unicode)
    case("gzip_members_split_json", capture(compress(good[:13]) + compress(good[13:])), data=good)
    case("gzip_empty_then_json", capture(compress(b"") + gz), data=good)
    case("gzip_json_then_empty", capture(gz + compress(b"")), data=good)
    header = bytearray(gz[:10]); header[3] = 4 | 8 | 16
    header += b"\x04\x00testsource.json\x00synthetic capture\x00"
    case("gzip_extra_name_comment", capture(bytes(header) + gz[10:]), data=good)
    header[3] |= 2
    header += (zlib.crc32(header) & 0xFFFF).to_bytes(2, "little")
    case("gzip_header_crc", capture(bytes(header) + gz[10:]), data=good)
    mixed = capture(gz); mixed["headers"]["Content-Encoding"] = " GZIP\t"; mixed["headers"]["Content-Type"] = "Application/JSON; charset=UTF-8"
    case("gzip_header_case_and_whitespace", mixed, data=good, gzip_valid=True)
    for name, enc in [("base64_identity_json", "identity"), ("base64_absent_encoding_json", None)]:
        case(name, capture(good, encoding=enc), data=good, source=BASE64)
    case("base64_empty_json", capture(b"", encoding="identity"), [finding("core.input.empty"), finding(VENDOR + "http.body.invalid")], data=b"", source=BASE64)
    raw = capture(good, encoding=None); raw.pop("body_base64"); raw["body"] = good.decode()
    case("ordinary_json_unchanged", raw, source=IMPORT)
    raw_gz = copy.deepcopy(raw); raw_gz["headers"]["Content-Encoding"] = "gzip"
    case("ordinary_raw_gzip_unchanged", raw_gz, [finding(CORE + "unsupported_body_encoding", "info")])
    for name, field, code in [("gzip_missing_distinct_id", "distinct_id", "properties.distinct_id.missing"), ("gzip_missing_time", "time", "properties.time.missing")]:
        event = copy.deepcopy(EVENT); event["properties"].pop(field); data = entity([event])
        case(name, capture(compress(data)), [finding(VENDOR + "body." + code)], data=data, source=IMPORT)
    for name, key, value, code in [("gzip_bad_insert_id", "$insert_id", "bad/id", "properties.$insert_id.invalid"), ("gzip_hashed_ip", "ip", "a" * 64, "hashed_plaintext_field")]:
        event = copy.deepcopy(EVENT); event["properties"][key] = value; data = entity([event])
        case(name, capture(compress(data)), [finding(VENDOR + "body." + code)], data=data, source=IMPORT)
    req = capture(gz); req["method"] = "GET"
    case("gzip_invalid_method", req, [finding(VENDOR + "http.method.invalid")], data=good, source=IMPORT)
    req = capture(gz); req["headers"]["Authorization"] = "Basic " + base64.b64encode(b"TEST_ONLY_USER:TEST_ONLY_PASSWORD").decode()
    case("gzip_service_account_no_project", req, [finding(VENDOR + "http.query.project_id.missing")], data=good, source=IMPORT)
    req = capture(gz, mime="application/xml")
    case("gzip_observed_bad_type", req, [finding(VENDOR + "http.content_type.invalid")], data=good, source=IMPORT)
    for count in [2000, 2001]:
        data = entity([EVENT] * count)
        expected = [] if count == 2000 else [finding(VENDOR + "body.root.invalid")]
        case("gzip_import_" + str(count), capture(compress(data)), expected, data=data, source=IMPORT)
    req = capture(gz); req["body"] = good.decode()
    case("both_body_fields", req, [finding(CORE + "invalid_envelope")], source=BASE64)
    req["body"] = None
    case("both_body_fields_raw_null", req, [finding(CORE + "invalid_envelope")], source=BASE64)
    for name, value in [("binary_body_nonstring", {}), ("binary_body_null", None), ("binary_body_integer", 5)]:
        req = capture(gz); req["body_base64"] = value
        case(name, req, [finding(CORE + "invalid_envelope")], source=BASE64)
    req = capture(gz)
    serialized = json.dumps(req); serialized = serialized[:-1] + ', "body_base64": ""}'
    case("duplicate_binary_field", serialized, [finding(CORE + "invalid_envelope")], source=BASE64)
    req = capture(gz); req.pop("body_base64"); req["capture"] = {"body": "available"}
    case("available_without_body", req, [finding(CORE + "invalid_envelope")], source=BASE64)
    req = capture(gz); req["capture"] = {"body": "absent"}
    case("absent_with_binary_body", req, [finding(CORE + "invalid_envelope")], source=BASE64)
    req = capture(gz); req["capture"] = {"body": "available"}
    case("available_with_binary_body", req, data=good, source=BASE64)
    for name, encoded in [("bad_base64_alphabet", "@@@@"), ("bad_base64_padding", "AA"), ("bad_base64_trailing_bits", "AB=="), ("bad_base64_url_alphabet", "_w=="), ("bad_base64_whitespace", "AA==\n")]:
        req = capture(gz); req["body_base64"] = encoded
        case(name, req, [finding(CORE + "invalid_base64_body")], source=BASE64)
    bad_gzip("gzip_empty_wire", b"")
    bad_gzip("gzip_bad_magic", b"not gzip bytes")
    for name, index, value in [("gzip_bad_method", 2, 0), ("gzip_reserved_flags", 3, 32), ("gzip_bad_crc", -8, gz[-8] ^ 1), ("gzip_bad_size", -4, gz[-4] ^ 1)]:
        damaged = bytearray(gz); damaged[index] = value; bad_gzip(name, damaged)
    damaged = bytearray(header); damaged[-1] ^= 1
    bad_gzip("gzip_bad_header_crc", bytes(damaged) + gz[10:])
    bad_gzip("gzip_truncated_fixed_header", gz[:9])
    bad_gzip("gzip_truncated_optional_header", b"\x1f\x8b\x08\x08" + gz[4:10] + b"unfinished-name")
    bad_gzip("gzip_truncated_deflate", gz[:len(gz) // 2])
    bad_gzip("gzip_truncated_footer", gz[:-1])
    bad_gzip("gzip_trailing_junk", gz + b"junk")
    bad_gzip("gzip_trailing_zero", gz + b"\0")
    bad_gzip("gzip_truncated_second_member", gz + gz[:8])
    damaged = bytearray(gz); damaged[-8] ^= 1
    bad_gzip("gzip_corrupt_second_member", gz + damaged)
    case("gzip_invalid_json_utf8", capture(compress(b'["\xff"]')), [finding(CORE + "invalid_body_utf8")])
    req = capture(b"\xff\x00\xfe", mime="application/octet-stream", encoding="identity")
    case("opaque_binary_utf8_unavailable", req, [finding(CORE + "unsupported_body_encoding", "info"), finding(VENDOR + "http.content_type.invalid")])
    for name, enc in [("unknown_encoding", "br"), ("multiple_encoding_tokens", "gzip, br"), ("gzip_alias_unsupported", "x-gzip")]:
        case(name, capture(gz, encoding=enc), [finding(CORE + "unsupported_body_encoding", "info")])
    req = capture(gz); req["headers"] = [{"name": k, "value": v} for k, v in req["headers"].items()] + [{"name": "content-encoding", "value": "gzip"}]
    case("repeated_encoding_headers", req, [finding(CORE + "unsupported_body_encoding", "info")])
    req = capture(gz); req["headers"] = [{"name": k, "value": v} for k, v in req["headers"].items()] + [{"name": "content-type", "value": "application/json"}]
    case("repeated_content_type", req, [finding(CORE + "unsupported_body_encoding", "info")])
    for name, key, available in [("unavailable_encoding", "Content-Encoding", "unavailable_headers"), ("redacted_encoding", "Content-Encoding", "redacted_headers"), ("unavailable_type", "Content-Type", "unavailable_headers")]:
        req = capture(gz); req["capture"] = {available: [key]}
        if available == "unavailable_headers": req["headers"].pop(key)
        req["body_base64"] = "uninspectable stored provenance"
        case(name, req, [finding(CORE + "capture_incomplete", "info")])
    req = capture(gz); req["capture"] = {"headers_unavailable": True}
    case("headers_incomplete_observed_decoder_headers", req, [finding(CORE + "capture_incomplete", "info")], data=good)
    req = capture(gz); req["headers"] = {}; req["capture"] = {"headers_unavailable": True}
    case("headers_incomplete_unobserved_decoder_headers", req, [finding(CORE + "capture_incomplete", "info")])
    for body in ["unavailable", "redacted"]:
        req = capture(gz); req["capture"] = {"body": body}; req["body_base64"] = "uninspectable stored provenance"
        case("body_" + body, req, [finding(CORE + "capture_incomplete", "info")])
    req = capture(b"not gzip bytes"); req["method"] = "GET"
    case("invalid_gzip_observed_method", req, [finding(CORE + "invalid_gzip_body"), finding(VENDOR + "http.method.invalid")])
    req = capture(gz, encoding=None)
    case("gzip_magic_with_observed_absent_encoding", req, [finding(CORE + "invalid_body_utf8")])
    # A real default bound is exercised without a giant committed raw body.
    # The highly compressible bytes have no payload semantics and must stay
    # uninspected after the local limit, so only the information finding fires.
    bomb = b" " * (16 * 1024 * 1024 + 1)
    case("default_entity_limit_plus_one", capture(compress(bomb)), [finding(CORE + "body_decode_limit", "info")], data=bomb)
    out = ROOT / "source-cases.json"
    out.write_text(json.dumps(CASES, indent=2, ensure_ascii=False) + "\n")
    oracle = {"producer": "Python gzip.compress/zlib, independent of Pixellint", "standard": RFC,
              "default_limit_control": "default_entity_limit_plus_one",
              "scope": "Hashes and byte counts prove captured and expanded representations, not vendor acceptance.",
              "cases": ORACLES}
    (ROOT / "compression-oracle.json").write_text(json.dumps(oracle, indent=2) + "\n")
    print(json.dumps({"source_cases": len(CASES), "byte_oracles": len(ORACLES)}))


if __name__ == "__main__":
    generate()
