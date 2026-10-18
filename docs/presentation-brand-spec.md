# Presentation design reference

The current About-page design and review are documented in [About page review](presence/about-review.md).

The page uses the existing Screenshare logo, local Barlow fonts and real interface imagery. It follows the system light/dark preference, uses one blue accent, and has no decorative animation. The shared template and localized copy are in `site/page.html` and `site/content.json`; generated pages and their stylesheet are in `server/src/jvmMain/resources/presentation/`.

Protected contracts: `/` HTTP app and WebSocket upgrade, form IDs/names, room codes, desktop startup endpoints, updater manifest/signatures, `/about` and `/about/en.html`, download and demo anchors, social metadata and transcripts. This is a server-only presentation change.
