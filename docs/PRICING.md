# Pricing and distribution

Decisions, and the reasons, so nobody has to re-derive them.

## 99p, once

- One-off payment. No subscription. People trying to get out of a loop with
  a product do not want another product with a loop.
- The price is a statement, not a revenue line. At 99p including UK VAT,
  the ex-VAT price is about 82p; a merchant of record takes roughly 5% plus
  a fixed 30 to 50p; the net is around 40p per sale. That is fine. If it
  ever matters, £1.99 is the same statement with 2.5x the net.
- Future major versions are sold separately. 1.x updates are free. The
  blocklist is free forever, for every version, because a one-off purchase
  that quietly stops working the week a new chatbot launches is a betrayal.

## Direct download, not the App Store

The App Store sandbox forbids everything the daemon does: a root helper,
`/etc/hosts`, `pfctl`, killing other apps, a LaunchDaemon. So:

- Notarized DMG from the website. Sparkle for in-major updates.
- Apple Developer Program ($99/yr) for Developer ID signing and
  notarization. For a non-technical audience, Gatekeeper is not optional:
  "right-click, Open" is exactly the chops we are removing. About 100 sales
  a year covers it.
- A merchant of record (Paddle, Lemon Squeezy) so EU VAT on digital goods is
  their problem. There is no registration threshold for EU consumer sales;
  from sale one it is either an MoR or VAT OSS.
- Until the MoR is set up, the site uses a Stripe Buy Button. That is fine
  for UK sales.

## Licensing

- Licence key signed with Ed25519, checked offline, keyed to the major
  version. No activation server, no phone-home. At 99p, piracy is not a
  threat, and a privacy tool that calls home would be wrong.
- The source is public under MIT. The 99p buys the signed, notarized,
  self-updating build. Engineers who build from source do not pay, and are
  not the customer; they are the people who audit it and vouch for it.

## Claims

Say "cannot be turned off early without a ten-minute recovery procedure",
not "99% effective". A concrete promise beats a statistic, and it is the one
we can keep.
