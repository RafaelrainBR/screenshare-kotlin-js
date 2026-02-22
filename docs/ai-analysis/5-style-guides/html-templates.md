# Style Guide: html-templates

## Unique Conventions

### Single index.html for Both Screens
There is one `index.html` file. The "join" and "room" screens are both present in the DOM simultaneously; visibility is toggled via the `hidden` CSS class.

### DaisyUI + TailwindCSS via CDN (No Local Build)
Both libraries are loaded from CDN:
```html
<link href="https://cdn.jsdelivr.net/npm/daisyui@latest/dist/full.css" rel="stylesheet">
<script src="https://cdn.tailwindcss.com"></script>
```

No `tailwind.config.js` or PostCSS pipeline exists.

### Default Theme via data-theme Attribute
```html
<html lang="pt-BR" data-theme="night">
```
Theme switching uses DaisyUI's `theme-controller` pattern with radio inputs.

### `clientApp.js` Script Inclusion
The Kotlin/JS bundle is included at the base URL (served by Ktor static resources). The script tag is placed at the end of `<body>`.

### Google Fonts (Inter)
```html
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap" rel="stylesheet">
body { font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; }
```

### Custom Inline Styles for Animations and Scrollbar
Custom CSS is written inline in a `<style>` tag for scrollbar styling (`.custom-scroll`) and keyframe animations (`animate-fade-in`, `pulse-ring`).

### Element IDs are the Contract with Kotlin
All elements accessed from Kotlin code have `id` attributes. IDs use `kebab-case`. Adding a new interactive element requires adding a corresponding entry to `ui/Elements.kt`.

### Language: Brazilian Portuguese
`lang="pt-BR"` on the `<html>` tag. All `placeholder`, `label`, and button text are in Portuguese.
