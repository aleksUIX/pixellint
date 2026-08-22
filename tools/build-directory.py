"""Build the Pixellint vendor endpoint directory, keeping only hosts that resolve.

The directory makes one claim per entry: this host belongs to this vendor. It
carries no parameter contracts, so nothing here is invented rule text. Hosts
that do not resolve are dropped rather than shipped on faith.
"""

import json
import pathlib
import socket
from concurrent.futures import ThreadPoolExecutor

# vendor slug -> (display name, category, [hosts], rulepack or None)
VENDORS = {
    # Social and platform pixels
    "meta": ("Meta", "social", ["connect.facebook.net", "www.facebook.com", "graph.facebook.com", "business.facebook.com"], "vendor/meta"),
    "tiktok": ("TikTok", "social", ["analytics.tiktok.com", "business-api.tiktok.com"], "vendor/tiktok"),
    "snapchat": ("Snapchat", "social", ["tr.snapchat.com", "sc-static.net", "tr6.snapchat.com"], "vendor/snapchat"),
    "pinterest": ("Pinterest", "social", ["ct.pinterest.com", "s.pinimg.com", "api.pinterest.com"], "vendor/pinterest"),
    "linkedin": ("LinkedIn", "social", ["px.ads.linkedin.com", "snap.licdn.com"], "vendor/linkedin"),
    "reddit": ("Reddit", "social", ["alb.reddit.com", "www.redditstatic.com", "pixel-config.reddit.com", "conversions-api.reddit.com", "ads-api.reddit.com"], "vendor/reddit"),
    "x": ("X (Twitter)", "social", ["analytics.twitter.com", "static.ads-twitter.com", "ads-api.twitter.com", "ads-api.x.com", "t.co"], "vendor/x-conversions-api"),
    "quora": ("Quora", "social", ["a.quora.com", "q.quora.com"], None),
    "nextdoor": ("Nextdoor", "social", ["ads.nextdoor.com"], None),

    # Search and large ad platforms
    "google": ("Google", "search", [
        "www.googletagmanager.com", "www.google-analytics.com", "analytics.google.com",
        "region1.google-analytics.com", "www.googleadservices.com",
        "googleads.g.doubleclick.net", "ad.doubleclick.net", "fls.doubleclick.net",
        "stats.g.doubleclick.net", "cm.g.doubleclick.net",
        "pagead2.googlesyndication.com", "www.googletagservices.com",
        "www.google.com", "adservice.google.com",
    ], "vendor/google-tag-manager"),
    "microsoft": ("Microsoft Advertising", "search", ["bat.bing.com", "bat.bing.net", "c.clarity.ms", "www.clarity.ms"], "vendor/microsoft-uet"),
    "yahoo": ("Yahoo", "search", ["sp.analytics.yahoo.com", "s.yimg.com", "ads.yahoo.com"], None),
    "yandex": ("Yandex", "search", ["mc.yandex.ru", "mc.yandex.com", "an.yandex.ru"], None),
    "baidu": ("Baidu", "search", ["hm.baidu.com", "cpro.baidustatic.com"], None),
    "naver": ("Naver", "search", ["wcs.naver.net", "wcs.naver.com"], None),

    # Demand and supply side platforms
    "thetradedesk": ("The Trade Desk", "programmatic", ["insight.adsrvr.org", "js.adsrvr.org", "match.adsrvr.org"], None),
    "amazon": ("Amazon Ads", "programmatic", ["s.amazon-adsystem.com", "aax.amazon-adsystem.com", "c.amazon-adsystem.com", "fls-na.amazon-adsystem.com"], None),
    "criteo": ("Criteo", "programmatic", ["sslwidget.criteo.com", "static.criteo.net", "gum.criteo.com", "dis.criteo.com", "widget.criteo.com"], None),
    "adform": ("Adform", "programmatic", ["track.adform.net", "s1.adform.net", "server.adform.net"], None),
    "xandr": ("Xandr", "programmatic", ["ib.adnxs.com", "secure.adnxs.com", "acdn.adnxs.com"], None),
    "pubmatic": ("PubMatic", "programmatic", ["ads.pubmatic.com", "image6.pubmatic.com", "simage2.pubmatic.com"], None),
    "magnite": ("Magnite", "programmatic", ["pixel.rubiconproject.com", "eus.rubiconproject.com", "fastlane.rubiconproject.com"], None),
    "openx": ("OpenX", "programmatic", ["us-ads.openx.net", "rtb.openx.net"], None),
    "indexexchange": ("Index Exchange", "programmatic", ["js-sec.indexww.com", "htlb.casalemedia.com"], None),
    "tripleLift": ("TripleLift", "programmatic", ["eb2.3lift.com", "tlx.3lift.com"], None),
    "mediamath": ("MediaMath", "programmatic", ["pixel.mathtag.com", "sync.mathtag.com"], None),
    "taboola": ("Taboola", "native", ["trc.taboola.com", "cdn.taboola.com"], None),
    "outbrain": ("Outbrain", "native", ["tr.outbrain.com", "widgets.outbrain.com", "amplify.outbrain.com"], None),
    "sharethrough": ("Sharethrough", "programmatic", ["btlr.sharethrough.com", "match.sharethrough.com"], None),

    # Identity, data, and onboarding
    "liveramp": ("LiveRamp", "identity", ["idsync.rlcdn.com", "api.rlcdn.com", "ats.rlcdn.com"], None),
    "id5": ("ID5", "identity", ["id5-sync.com", "lb.eu-1-id5-sync.com"], None),
    "lotame": ("Lotame", "identity", ["tags.crwdcntrl.net", "bcp.crwdcntrl.net"], None),
    "oracle": ("Oracle Advertising", "identity", ["tags.bluekai.com", "stags.bluekai.com"], None),
    "salesforce": ("Salesforce", "identity", ["beacon.krxd.net", "cdn.krxd.net"], None),
    "neustar": ("TransUnion (Neustar)", "identity", ["aa.agkn.com"], None),
    "tapad": ("Tapad", "identity", ["pixel.tapad.com"], None),
    "adobe_aam": ("Adobe Audience Manager", "identity", ["dpm.demdex.net", "cm.everesttech.net"], None),

    # Verification and measurement
    "doubleverify": ("DoubleVerify", "verification", ["cdn.doubleverify.com", "tps.doubleverify.com", "rtb0.doubleverify.com"], None),
    "ias": ("Integral Ad Science", "verification", ["pixel.adsafeprotected.com", "static.adsafeprotected.com", "dt.adsafeprotected.com"], None),
    "moat": ("Oracle Moat", "verification", ["px.moatads.com", "z.moatads.com", "js.moatads.com"], None),
    "nielsen": ("Nielsen", "measurement", ["secure-dcr.imrworldwide.com", "secure-gl.imrworldwide.com"], None),
    "comscore": ("Comscore", "measurement", ["sb.scorecardresearch.com", "b.scorecardresearch.com"], None),
    "quantcast": ("Quantcast", "measurement", ["pixel.quantserve.com", "secure.quantserve.com"], None),
    "innovid": ("Innovid", "video", ["dts.innovid.com", "static.innovid.com"], None),
    "freewheel": ("FreeWheel", "video", ["bea4.v.fwmrm.net", "cdn.stickyadstv.com"], None),

    # Product and web analytics
    "adobe_analytics": ("Adobe Analytics", "analytics", ["sc.omtrdc.net", "smetrics.adobe.com", "assets.adobedtm.com"], "vendor/adobe-analytics"),
    "amplitude": ("Amplitude", "analytics", ["api.amplitude.com", "api2.amplitude.com", "cdn.amplitude.com"], "vendor/amplitude"),
    "mixpanel": ("Mixpanel", "analytics", ["api.mixpanel.com", "cdn.mxpnl.com", "api-js.mixpanel.com"], "vendor/mixpanel"),
    "segment": ("Segment", "analytics", ["api.segment.io", "cdn.segment.com"], "vendor/segment"),
    "heap": ("Heap", "analytics", ["heapanalytics.com", "cdn.heapanalytics.com"], None),
    "hotjar": ("Hotjar", "analytics", ["script.hotjar.com", "in.hotjar.com", "static.hotjar.com"], None),
    "fullstory": ("FullStory", "analytics", ["edge.fullstory.com", "rs.fullstory.com"], None),
    "crazyegg": ("Crazy Egg", "analytics", ["script.crazyegg.com", "tracking.crazyegg.com"], None),
    "chartbeat": ("Chartbeat", "analytics", ["ping.chartbeat.net", "static.chartbeat.com"], None),
    "parsely": ("Parse.ly", "analytics", ["p1.parsely.com", "cdn.parsely.com"], None),
    "posthog": ("PostHog", "analytics", ["us.i.posthog.com", "eu.i.posthog.com", "app.posthog.com"], "vendor/posthog"),
    "plausible": ("Plausible", "analytics", ["plausible.io"], None),
    "matomo": ("Matomo", "analytics", ["cdn.matomo.cloud"], None),
    "cloudflare": ("Cloudflare Web Analytics", "analytics", ["static.cloudflareinsights.com"], None),
    "mouseflow": ("Mouseflow", "analytics", ["cdn.mouseflow.com"], None),
    "statcounter": ("StatCounter", "analytics", ["www.statcounter.com", "c.statcounter.com"], None),

    # Marketing automation, CRM, and commerce
    "hubspot": ("HubSpot", "martech", ["js.hs-scripts.com", "track.hubspot.com", "js.hs-analytics.net"], None),
    "marketo": ("Adobe Marketo", "martech", ["munchkin.marketo.net"], None),
    "pardot": ("Salesforce Account Engagement", "martech", ["pi.pardot.com", "pi.demand.salesforce.com"], None),
    "klaviyo": ("Klaviyo", "martech", ["static.klaviyo.com", "a.klaviyo.com"], "vendor/klaviyo"),
    "braze": ("Braze", "martech", ["sdk.iad-01.braze.com", "js.appboycdn.com"], "vendor/braze"),
    "mailchimp": ("Mailchimp", "martech", ["chimpstatic.com", "us1.list-manage.com"], None),
    "shopify": ("Shopify", "commerce", ["monorail-edge.shopifysvc.com", "cdn.shopify.com"], None),
    "stripe": ("Stripe", "commerce", ["m.stripe.com", "js.stripe.com"], None),
    "intercom": ("Intercom", "martech", ["api-iam.intercom.io", "widget.intercom.io"], None),
    "drift": ("Drift", "martech", ["js.driftt.com", "event.api.drift.com"], None),
    "zendesk": ("Zendesk", "martech", ["static.zdassets.com", "ekr.zdassets.com"], None),

    # Affiliate and mobile attribution
    "impact": ("impact.com", "affiliate", ["utt.impactcdn.com", "app.impact.com"], None),
    "cj": ("CJ Affiliate", "affiliate", ["www.emjcd.com", "www.mczbf.com"], None),
    "rakuten": ("Rakuten Advertising", "affiliate", ["track.linksynergy.com", "click.linksynergy.com"], None),
    "awin": ("Awin", "affiliate", ["www.awin1.com", "www.dwin1.com"], None),
    "shareasale": ("ShareASale", "affiliate", ["www.shareasale.com", "shareasale.com"], None),
    "partnerize": ("Partnerize", "affiliate", ["prf.hn", "cdn.performancehorizon.com"], None),
    "appsflyer": ("AppsFlyer", "mobile", ["impression.appsflyer.com", "launches.appsflyer.com", "onelink.me", "api3.appsflyer.com"], "vendor/appsflyer"),
    "adjust": ("Adjust", "mobile", ["app.adjust.com", "s2s.adjust.com", "view.adjust.com"], "vendor/adjust"),
    "branch": ("Branch", "mobile", ["api2.branch.io", "cdn.branch.io", "app.link"], "vendor/branch"),
    "kochava": ("Kochava", "mobile", ["control.kochava.com", "imp.control.kochava.com"], None),
    "singular": ("Singular", "mobile", ["sdk-api-v1.singular.net", "i.sng.link"], None),

    # Consent management
    "onetrust": ("OneTrust", "consent", ["cdn.cookielaw.org", "geolocation.onetrust.com"], None),
    "trustarc": ("TrustArc", "consent", ["consent.trustarc.com", "consent-st.trustarc.com"], None),
    "sourcepoint": ("Sourcepoint", "consent", ["cdn.privacy-mgmt.com"], None),
    "didomi": ("Didomi", "consent", ["sdk.privacy-center.org", "api.privacy-center.org"], None),
    "usercentrics": ("Usercentrics", "consent", ["app.usercentrics.eu", "web.cmp.usercentrics.eu"], None),
    "cookiebot": ("Cookiebot", "consent", ["consent.cookiebot.com", "consentcdn.cookiebot.com"], None),
}


def resolves(host: str) -> bool:
    try:
        socket.getaddrinfo(host, 443, proto=socket.IPPROTO_TCP)
        return True
    except OSError:
        return False


def main() -> None:
    hosts = sorted({host for _, _, entry_hosts, _ in VENDORS.values() for host in entry_hosts})

    with ThreadPoolExecutor(max_workers=32) as pool:
        results = dict(zip(hosts, pool.map(resolves, hosts)))

    dropped = sorted(host for host, ok in results.items() if not ok)
    entries = []

    for vendor, (display_name, category, entry_hosts, rulepack) in sorted(VENDORS.items()):
        kept = [host for host in entry_hosts if results[host]]
        if not kept:
            continue
        entry = {
            "vendor": vendor,
            "display_name": display_name,
            "category": category,
            "hosts": sorted(kept),
        }
        if rulepack:
            entry["rulepack"] = rulepack
        entries.append(entry)

    directory = {"entries": entries}
    out = pathlib.Path("crates/pixellint-core/rulepacks/directory.json")
    out.write_text(json.dumps(directory, indent=2) + "\n")

    print(f"vendors: {len(entries)}")
    print(f"hosts kept: {sum(len(e['hosts']) for e in entries)}")
    print(f"hosts dropped (no DNS): {dropped}")


if __name__ == "__main__":
    main()
