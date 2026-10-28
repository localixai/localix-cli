# Kypros Mobile: the landing page

Kypros Mobile is a small phone shop in Limassol, Cyprus. It wants one page that sells the phones
in `src/phones.ts`: people come from ads on their phones, look, and order on WhatsApp.

## What the page has

- The shop's name, a headline and one line on why buy here: delivery across Cyprus in 1-2 days,
  free from €300, a 2-year warranty, payment on delivery.
- The phones, one card each: brand, name, storage, the price, and an **Order** link.
  - A price is written with the euro sign first and a comma between thousands, no cents:
    `€1,299`, `€949`.
  - **Order** opens WhatsApp: `https://wa.me/35799123456?text=` followed by the URL-encoded
    message `Hi! I'd like to order the <name> (<storage>).`
  - A phone with `stock` 0 shows **Sold out** instead of the Order link; one with 1 to 3 left
    shows `Only <stock> left`.
- Buttons that filter the cards by brand: `All`, then each brand that has phones, in the order
  they first appear. `All` is chosen at first.
- A contact block: the address (28 Anexartisias St, Limassol), the opening hours (Mon-Sat
  10:00-20:00), and the phone number `+357 99 123456` as a `tel:+35799123456` link.

## How it is built

- React and TypeScript, styles of its own (no CSS framework, nothing fetched from the network).
- It looks good on a phone first, then on a desktop.
- `App` is also rendered without a browser in the tests: nothing reads `window` or `document`
  while rendering.
