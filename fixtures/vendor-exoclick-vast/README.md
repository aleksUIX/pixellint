# The pack checks bounded VAST callback templates.

The official VAST guide publishes impression, error, progress and click
templates on its named default syndication placeholder. ExoClick's own
integration documentation links `https://a.magsrv.com/ad-provider.js`. The
inspected script explicitly configures `syndication` as `s.magsrv.com`, and
the current official conversion guide independently names that same host.
This establishes current host attribution and the default service role.
It does not publish the receiver's whole schema or a callback constructor.

Field presence advice is limited to recognizable documented branches and is
warning-level. Identifier grammar, opaque payload spelling and emptiness,
requiredness on unlisted actions/events, request methods, protocol requirements,
duplicate rules and the unrelated `/cimp.php` remain open. The error template
legitimately omits `data` and `tracking_event`. Progress values remain opaque
because the XML offset syntax alone does not define every query encoding.

Golden fixtures preserve the independent public template examples after
substituting the source-bound current default host. They cover each missing
carrier, unlisted actions/events, conflicting discriminator repetitions,
native blank values, route isolation and an uncovered cimp callback.
