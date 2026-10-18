# Presentation design reference

Mode: extension of the existing Screenshare brand, with a new public presentation route. The room interface and its behavior stay in place.

Design read: a product-led landing page for friends and developers, using the app's restrained dark visual language and real UI imagery. Visual variance 5, motion 2, density 4, asset dependence 9, brand fidelity 10.

- Colors: background `#101318`, surface `#171d26`, text `#eef2fa`, secondary text `#aab6c8`, accent `#8eb4fa`, accent text `#0c1022`. These are the current `ui.css` tokens.
- Typography: local Barlow regular/bold from `client/src/jsMain/resources/fonts/`; fluid headings, 18px body, no new font dependency.
- Spacing: 8px base; 24/40/64/96px section spacing; content width up to 1360px.
- Shapes: 8px controls and 10px media frames; borders instead of decorative shadows.
- Motion: short hover/focus feedback only; no automatic video playback or scroll effects.
- Logo: existing `brand/screenshare-logo-on-dark.svg` and `brand/screenshare-mark.svg`; no redrawn identity.
- Product imagery: `docs/images/room_sharing.png` and the recorded real two-peer local session. UI sources relevant to the demonstration were compared with tag `v0.11.1`; the HTML, styles and capture controls match.
- Narrative: concise left-aligned proposition, actual interface, demonstration, capabilities, platform downloads and practical limits.
- Protected contracts: `/` HTTP app and WebSocket upgrade, form IDs/names, room codes, desktop startup endpoints and updater manifest/signatures.

The initial `/about` sketch and subsequent browser QA use this same reference. Generated HTML comes from `site/page.html` and `site/content.json`; the public page is served by the existing Ktor application, not a separate hosting product.
