"""Generate synthetic URL cases from pinned official templates without network or engine code."""
import hashlib
import json
from html.parser import HTMLParser
from pathlib import Path
import re
import sys
from urllib.parse import quote

source_dir, output_path = map(Path, sys.argv[1:])
PINS = {
    "remerge-event": "9f2f9abe31ca513cf1f2d55018679512e0c53bc6e2d0d7a7438afd77a1ed9275",
    "remerge-bi": "f7300a96c13cbf2df5e580924920870e5f341775574b855eea22e838c314078b",
    "hybrid-en": "9436ccbf4ed6c4c5ab9568dcfa40c5390a09ad6d8aee1d30c11b8a787c7d75ff",
}


class DocumentText(HTMLParser):
    def __init__(self):
        super().__init__()
        self.parts = []

    def handle_data(self, data):
        self.parts.append(data)


def read(name):
    raw = (source_dir / f"{name}.html").read_bytes()
    assert hashlib.sha256(raw).hexdigest() == PINS[name], name
    parsed = DocumentText()
    parsed.feed(raw.decode("utf8"))
    return "".join(parsed.parts)


remerge, bi, hybrid = (read(name) for name in PINS)
event_template = re.search(r"https://remerge\.events/event\?app_id=\{AppId\}[^\n]+", remerge)[0]
bi_template = re.search(r"https://remerge\.events/event\?app_id=\{GooglePlay or AppstoreID\}&event=whales_user[^\n]+", bi)[0]
pixel_template = re.search(r"https://dss\.hybrid\.ai/pixel/trackapi2\?[^\n]+", hybrid)[0].strip()
assert "20 characters (possible characters 0-9a-f)" in hybrid
zero = "00000000-0000-0000-0000-000000000000"
values = {
    "AppId": "com.synthetic.example", "EventName": "purchase", "IDFA": zero, "AAID": zero,
    "Country": "DE", "DeviceName": "Synthetic Device", "OS": "android", "OSVersion": "1.2",
    "Timestamp": "1792886400", "RevenueAmount": "170000", "Currency": "USD",
    "GooglePlay or AppstoreID": "com.synthetic.example", "IDFV": zero, "OSName": "ios",
}


def substitute(template):
    return re.sub(r"\{([^}]+)\}", lambda m: quote(values[m[1]], safe=""), template).replace(
        "partner=yourname", "partner=synthetic-partner"
    ).replace("key=yourkey", "key=synthetic-key")


cases = [
    {"id": "source-event-template", "pack": "vendor/remerge-event", "url": substitute(event_template)},
    {"id": "source-bi-template", "pack": "vendor/remerge-event", "url": substitute(bi_template)},
    {"id": "source-pixel-template", "pack": "vendor/hybrid-dss-pixel", "url": pixel_template.replace(
        "PIXEL_ID", "synthetic-pixel"
    ).replace("VISITOR_ID", "0123456789abcdef0123").replace("EVENT_NAME", "synthetic_event")},
]
output_path.write_text(json.dumps({
    "sources": PINS, "method": "Exact published URL templates with synthetic substitutions.",
    "engine_derived": False, "network_calls": 0, "cases": cases,
}, indent=2) + "\n")
print(f"{len(cases)} independent source template outputs; no network or engine code.")
