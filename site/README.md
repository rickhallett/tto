# turnthemoff site

One `index.html`, served by `server.ts` under Bun. No JS framework, no CSS
framework, no build step.

```
cd site && bun run dev        # http://localhost:3000
```

Environment (optional):

| var | what |
|---|---|
| `GA_MEASUREMENT_ID` | Google tag. Without it, no analytics script is emitted at all. |

Routes: `/`, `/llms.txt`, `/p.png` (a one-pixel PNG whose `tEXt` chunk is
`llms.txt`, for models that only read pictures), `/logo.svg`, `/health`.

Deployed on Vercel as a Bun function with `rootDirectory = site`.

After editing HTML or logo assets, run `bun site/build-assets.ts` from the repo root.
The generated JSON is committed to keep the Bun deployment self-contained; CI checks freshness.
