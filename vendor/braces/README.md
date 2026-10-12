# Local braces security patch

Copied from the published `braces@3.0.3` package (MIT license retained).
GHSA-vfj7-8cjw-p6xm has no patched upstream release. The parser now limits
brace and parenthesis nesting, and compile, expand and stringify also limit
recursion for caller-provided ASTs. Ordinary patterns keep the upstream API.

The hard limit is 128 tree levels. Remove this copy and the Yarn resolution
when an upstream release fixes the advisory. Run `node vendor/braces/test.cjs`
after the Kotlin npm install to check the installed override and regression cases.
