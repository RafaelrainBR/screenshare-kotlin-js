# About page review — 2026-10-18

## Findings and direction

The original page already had real product imagery, local fonts, accessible links and honest platform limitations. Its weak points were repeated slogan-style headings, a small screenshot whose content was difficult to read, and a second oversized media section immediately repeating it. Different widths also relied on a forced no-wrap headline.

Preserve the existing logo, Barlow typeface, blue accent, URLs, metadata, downloads, transcripts and platform caveats. Make the interface the main evidence: a full-width screenshot, a compact optional demonstration, practical section headings and readable capability rows. No new dependencies, decorative icons, animation or invented claims.

Design read: a public product page for people sharing screens with friends or colleagues; restrained and practical, with real interface imagery. Variance 4, motion 1, density 4, asset dependence 9, brand fidelity 9. Headline establishes the purpose; the adjacent copy and primary action explain how to enter. The full-width image demonstrates the result. Phone layouts stack in reading order.

## System

- Local Barlow regular/bold; body 18px, section headings 28–36px, main heading 34–56px.
- Existing brand blue #2649cc on #f7f8fa in light mode; existing #8eb4fa on #101318 in dark mode. Follow the system preference. Text #202a3c / #eef2fa; secondary text #515d70 / #aab6c8.
- 1120px content width; 8px spacing base, 24/32/40/56/64px gaps; 6px corners for controls and media. No shadows or scroll effects.
- Existing logo variants chosen with picture; consistent navigation, keyboard focus, skip link, native video and disclosure controls.

## Research

- [Nielsen Norman Group: Aesthetic and Minimalist Design](https://www.nngroup.com/articles/aesthetic-minimalist-design/): relevance and clear priority matter more than pursuing a particular minimal visual style.
- [Joshua Snoddy: Why Do AI-Generated Websites All Look the Same?](https://www.joshuasnoddy.com/blog/why-ai-websites-look-the-same/): repeated template compositions and default visual choices erase product identity.
- [Nielsen Norman Group: Flat-Design Best Practices](https://www.nngroup.com/articles/flat-design-best-practices/): simplify without making controls hard to identify.

These are design guidance, not evidence that any single font, color or layout proves AI authorship. The changes address this page's hierarchy and content rather than replacing one fashionable template with another.

## Scope and verification

Source template, localized copy, presentation stylesheet, generated PT/EN HTML, visual reference and existing browser QA. Room UI, protocol, release packages, routes and download resolution are unchanged. Both light/dark modes and both languages are checked at 320, 390, 768, 1280 and 1440px. Existing QA also covers no-JavaScript navigation, macOS instructions, media seek, assets, metadata and two-peer chat.

Deployment replaces the web server and can interrupt in-memory rooms. No desktop release is needed for server-only presentation resources. Native capture hardware and human usability testing are outside this visual change.

## Results

- Presentation generator: 4 tests passed; generated PT/EN pages match their sources.
- Production server JAR builds successfully. The server JVM test task has no test sources.
- Browser QA: both languages without JavaScript; 20 viewport/theme combinations; skip link, downloads, macOS disclosure, media decoding/seek, static assets and two-peer chat.
- Local mobile Lighthouse: performance 94, accessibility 100, best practices 100, SEO 100. Performance was 84 before responsive media and font preload; simulated mobile LCP improved from 4.2s to 3.0s. These are lab measurements, not production field data.
- Screenshot delivered as lossless WebP: 720px / 74,760 bytes and 1440px / 199,988 bytes, compared with the retained 272,126-byte PNG. No artwork was generated or changed.
- Screenshots and JSON reports: `artifacts/qa/presence/` (local, ignored).
