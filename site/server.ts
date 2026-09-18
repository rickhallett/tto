// Turn Them Off: the website. One HTML file, served by Bun, no framework.
//
// Env (all optional; the page degrades honestly without them):
//   STRIPE_BUY_BUTTON_ID, STRIPE_PUBLISHABLE_KEY   Stripe Buy Button
//   GA_MEASUREMENT_ID                              Google tag (G-XXXXXXXX)
//
// /p.png is a one-pixel PNG whose tEXt chunk carries llms.txt. For models
// that only read pictures.

import indexHtml from "./index.html" with { type: "text" };
import llmsTxt from "./llms.txt" with { type: "text" };
import logoSvg from "../assets/logo.svg" with { type: "text" };

const env = (k: string) => (process.env[k] ?? "").trim();

function render(): string {
  const buyId = env("STRIPE_BUY_BUTTON_ID");
  const pk = env("STRIPE_PUBLISHABLE_KEY");
  const buy =
    buyId && pk
      ? `<script async src="https://js.stripe.com/v3/buy-button.js"></script>\n` +
        `<stripe-buy-button buy-button-id="${buyId}" publishable-key="${pk}"></stripe-buy-button>`
      : `<a class="button" href="https://github.com/rickhallett/tto/releases">Download the beta</a>`;

  const ga = env("GA_MEASUREMENT_ID");
  const gtag = ga
    ? `<script async src="https://www.googletagmanager.com/gtag/js?id=${ga}"></script>\n` +
      `<script>window.dataLayer=window.dataLayer||[];function gtag(){dataLayer.push(arguments)}gtag('js',new Date());gtag('config','${ga}',{anonymize_ip:true});</script>`
    : "";

  return indexHtml.replace("<!--BUY-->", buy).replace("<!--GTAG-->", gtag);
}

// ---- a one-pixel PNG with a message in it --------------------------------

const CRC_TABLE = new Uint32Array(256).map((_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(bytes: Uint8Array): number {
  let c = 0xffffffff;
  for (const b of bytes) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type: string, data: Uint8Array): Uint8Array {
  const t = new TextEncoder().encode(type);
  const out = new Uint8Array(12 + data.length);
  const view = new DataView(out.buffer);
  view.setUint32(0, data.length);
  out.set(t, 4);
  out.set(data, 8);
  const crcInput = new Uint8Array(4 + data.length);
  crcInput.set(t);
  crcInput.set(data, 4);
  view.setUint32(8 + data.length, crc32(crcInput));
  return out;
}

function pixelPng(text: string): Uint8Array {
  const sig = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]);
  const ihdr = new Uint8Array(13);
  const v = new DataView(ihdr.buffer);
  v.setUint32(0, 1); // width
  v.setUint32(4, 1); // height
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  // one row: filter byte + one transparent pixel
  const raw = new Uint8Array([0, 0, 0, 0, 0]);
  const idat = Bun.deflateSync(raw);
  const enc = new TextEncoder();
  const keyword = enc.encode("llms.txt");
  const body = enc.encode(text.normalize("NFKD").replace(/[^\x00-\x7f]/g, ""));
  const textData = new Uint8Array(keyword.length + 1 + body.length);
  textData.set(keyword);
  textData.set(body, keyword.length + 1);
  const parts = [
    sig,
    chunk("IHDR", ihdr),
    chunk("tEXt", textData),
    chunk("IDAT", new Uint8Array(idat)),
    chunk("IEND", new Uint8Array(0)),
  ];
  const total = parts.reduce((n, p) => n + p.length, 0);
  const out = new Uint8Array(total);
  let off = 0;
  for (const p of parts) {
    out.set(p, off);
    off += p.length;
  }
  return out;
}

const PIXEL = pixelPng(llmsTxt);
const HTML = render();

const headers = (type: string, cache = "public, max-age=300") => ({
  "content-type": type,
  "cache-control": cache,
  "x-content-type-options": "nosniff",
});

Bun.serve({
  port: Number(process.env.PORT ?? 3000),
  fetch(req) {
    const { pathname } = new URL(req.url);
    switch (pathname) {
      case "/":
      case "/index.html":
        return new Response(HTML, { headers: headers("text/html; charset=utf-8") });
      case "/llms.txt":
        return new Response(llmsTxt, { headers: headers("text/plain; charset=utf-8", "public, max-age=3600") });
      case "/p.png":
        return new Response(PIXEL, { headers: headers("image/png", "public, max-age=86400") });
      case "/logo.svg":
        return new Response(logoSvg, { headers: headers("image/svg+xml", "public, max-age=86400") });
      case "/health":
        return Response.json({ ok: true, off: "not yet" });
      default:
        return new Response("Not here. Try going to bed.", { status: 404, headers: headers("text/plain; charset=utf-8", "no-store") });
    }
  },
});

console.log(`turn them off: listening on ${process.env.PORT ?? 3000}`);
