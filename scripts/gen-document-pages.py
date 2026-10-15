#!/usr/bin/env python3
"""Draw the forty document pages of `examples/jud/stamps-cases.jud`, and the
cases document itself, from one seed, so that the labels are the drawing's
own coordinates and no labeller's judgment is in them.

    python3 -I scripts/gen-document-pages.py [--out examples/jud] [--scale 1] [--seed 20261015]

Each page is a 600 by 780 pixel invoice, delivery note, purchase order or
receipt: a header with a round logo (a distractor the Box could take for
the target), a title, address lines, a table of lines, a totals block, a
signature scribble and a barcode (two more distractors). Thirty pages carry
one round ink stamp, 48 to 72 pixels across, rotated, semi-transparent,
anywhere under the header, text and table included; ten carry none. The
stamp's bounding box is the label, in fractions of the page, so the same
labels hold at every `--scale` (a page drawn at scale 2 is 1200 by 1560,
every coordinate doubled). The PNGs are written with a 64-colour palette,
so a page is a few kilobytes, and only Pillow is needed.

The committed pages are the ones the recordings under
`examples/recordings/stamps/` were made on: a page drawn again with another
Pillow or another font changes its bytes, and with them every request
fingerprint, so the recordings would no longer answer.
"""

import argparse
import os
import random

from PIL import Image, ImageDraw, ImageFont

WIDTH, HEIGHT = 600, 780
PAGES, WITH_STAMP = 40, 30
FONT_DIR = "/usr/share/fonts/truetype/dejavu"

WORDS = (
    "meridian halden corvid lantern ridgeway sable thorne ashby verdant kestrel "
    "pembroke orrin tallow bramble quill fenwick lumen marlow saffron wynter"
).split()
TRADES = ["Supplies", "Freight", "Works", "Logistics", "Trading", "Print", "Timber", "Foods"]
TITLES = ["INVOICE", "DELIVERY NOTE", "PURCHASE ORDER", "RECEIPT"]
ITEMS = (
    "pallet strap kit, cardboard box 40x30, label roll 1000, packing tape, "
    "bubble wrap 50 m, courier bag large, document pouch, stretch film, "
    "wooden crate small, foam insert, fragile stickers, corner protector"
).split(", ")
STAMP_WORDS = ["RECEIVED", "PAID", "APPROVED", "ENTERED", "CHECKED"]
INKS = [(200, 30, 40), (30, 50, 180), (20, 120, 60), (120, 40, 140)]


def font(name, size):
    return ImageFont.truetype(os.path.join(FONT_DIR, name), size)


def words(rng, n):
    return " ".join(rng.choice(WORDS).capitalize() if i == 0 else rng.choice(WORDS) for i in range(n))


def draw_page(rng, scale, stamp):
    """One page at `scale`; returns the image and the stamp's box in
    fractions (top, bottom, left, right), or None."""
    s = scale
    w, h = WIDTH * s, HEIGHT * s
    page = Image.new("RGB", (w, h), (255, 255, 255))
    d = ImageDraw.Draw(page)
    body = font("DejaVuSans.ttf", 11 * s)
    bold = font("DejaVuSans-Bold.ttf", 12 * s)
    big = font("DejaVuSans-Bold.ttf", 20 * s)
    small = font("DejaVuSans.ttf", 9 * s)

    # Header: a round logo with two letters, the company, the title.
    company = f"{words(rng, 1)} {rng.choice(TRADES)}"
    logo_colour = rng.choice([(40, 60, 120), (20, 110, 90), (150, 60, 30)])
    d.ellipse((30 * s, 26 * s, 74 * s, 70 * s), fill=logo_colour)
    initials = "".join(part[0] for part in company.split()[:2])
    d.text((52 * s, 48 * s), initials, fill=(255, 255, 255), font=bold, anchor="mm")
    d.text((90 * s, 36 * s), company, fill=(20, 20, 20), font=big, anchor="lm")
    d.text((90 * s, 60 * s), f"{words(rng, 2)} road, {words(rng, 1)}", fill=(80, 80, 80), font=small, anchor="lm")
    title = rng.choice(TITLES)
    d.text((570 * s, 40 * s), title, fill=(20, 20, 20), font=big, anchor="rm")
    d.text((570 * s, 62 * s), f"No. {rng.randint(10000, 99999)}   {rng.randint(1, 28):02d}/{rng.randint(1, 12):02d}/2026", fill=(80, 80, 80), font=small, anchor="rm")
    d.line((30 * s, 84 * s, 570 * s, 84 * s), fill=(60, 60, 60), width=s)

    # Two address blocks.
    for x, label in ((30, "Bill to"), (320, "Deliver to")):
        d.text((x * s, 100 * s), label, fill=(100, 100, 100), font=small, anchor="lm")
        for i in range(3):
            d.text((x * s, (116 + 14 * i) * s), words(rng, rng.randint(2, 4)), fill=(30, 30, 30), font=body, anchor="lm")

    # The table.
    top = 180
    columns = ((30, "Item"), (330, "Qty"), (400, "Unit"), (480, "Total"))
    d.rectangle((30 * s, top * s, 570 * s, (top + 18) * s), fill=(235, 235, 235))
    for x, name in columns:
        d.text((x * s + 4 * s, (top + 9) * s), name, fill=(20, 20, 20), font=bold, anchor="lm")
    rows = rng.randint(5, 9)
    total = 0
    for i in range(rows):
        y = top + 18 + 20 * i
        qty, unit = rng.randint(1, 40), rng.randint(2, 90) + rng.randint(0, 99) / 100
        line = qty * unit
        total += line
        cells = (rng.choice(ITEMS), str(qty), f"{unit:.2f}", f"{line:.2f}")
        for (x, _), text in zip(columns, cells):
            d.text((x * s + 4 * s, (y + 10) * s), text, fill=(30, 30, 30), font=body, anchor="lm")
        d.line((30 * s, (y + 20) * s, 570 * s, (y + 20) * s), fill=(210, 210, 210), width=s)
    y = top + 18 + 20 * rows + 16
    for label, value in (("Subtotal", total), ("VAT 20 %", total * 0.2), ("Total due", total * 1.2)):
        d.text((400 * s, y * s), label, fill=(30, 30, 30), font=bold if label.startswith("Total") else body, anchor="lm")
        d.text((570 * s, y * s), f"{value:.2f}", fill=(30, 30, 30), font=bold if label.startswith("Total") else body, anchor="rm")
        y += 18

    # Terms, a signature scribble and a barcode: three things that are not
    # a stamp.
    y += 10
    for _ in range(rng.randint(2, 4)):
        d.text((30 * s, y * s), words(rng, rng.randint(6, 11)) + ".", fill=(60, 60, 60), font=small, anchor="lm")
        y += 13
    sig_y = HEIGHT - 110
    d.text((30 * s, sig_y * s), "Signed", fill=(100, 100, 100), font=small, anchor="lm")
    points = []
    x = 90
    while x < 260:
        points.append((x * s, (sig_y + rng.randint(-14, 14)) * s))
        x += rng.randint(6, 14)
    d.line(points, fill=(20, 30, 90), width=2 * s, joint="curve")
    d.line((90 * s, (sig_y + 18) * s, 270 * s, (sig_y + 18) * s), fill=(120, 120, 120), width=s)
    bx = 400
    for _ in range(50):
        bar = rng.randint(1, 3)
        if rng.random() < 0.55:
            d.rectangle((bx * s, (HEIGHT - 120) * s, (bx + bar) * s - 1, (HEIGHT - 80) * s), fill=(0, 0, 0))
        bx += bar + rng.randint(1, 2)
    d.text((470 * s, (HEIGHT - 70) * s), f"{rng.randint(10**11, 10**12 - 1)}", fill=(0, 0, 0), font=small, anchor="mm")
    d.text((300 * s, (HEIGHT - 20) * s), f"{company} · page 1 of 1", fill=(130, 130, 130), font=small, anchor="mm")

    if not stamp:
        return page, None

    # The stamp: two rings, a word across the middle and a date under it,
    # drawn on its own layer, rotated, and laid over the page with its ink
    # half transparent, so the text under it shows through as under a real
    # one. Its bounding box is the outer ring's, which a rotation keeps.
    r = rng.randint(24, 36)
    cx = rng.randint(r + 12, WIDTH - r - 12)
    cy = rng.randint(96 + r, HEIGHT - r - 12)
    ink = rng.choice(INKS)
    pad = 4
    size = (2 * r + 2 * pad) * s
    layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    ld = ImageDraw.Draw(layer)
    c = size // 2
    ld.ellipse((c - r * s, c - r * s, c + r * s, c + r * s), outline=ink + (255,), width=3 * s)
    inner = r - 6
    ld.ellipse((c - inner * s, c - inner * s, c + inner * s, c + inner * s), outline=ink + (255,), width=s)
    word = rng.choice(STAMP_WORDS)
    ld.text((c, c - 4 * s), word, fill=ink + (255,), font=font("DejaVuSans-Bold.ttf", max(7, r // 3) * s), anchor="mm")
    ld.text((c, c + 9 * s), f"{rng.randint(1, 28):02d} {rng.choice(['JAN', 'FEB', 'MAR', 'APR', 'MAY', 'JUN', 'JUL', 'AUG', 'SEP', 'OCT'])} 2026", fill=ink + (255,), font=font("DejaVuSans.ttf", max(6, r // 4) * s), anchor="mm")
    layer = layer.rotate(rng.uniform(-28, 28), resample=Image.BICUBIC)
    alpha = layer.getchannel("A").point(lambda a: int(a * 0.78))
    layer.putalpha(alpha)
    page.paste(layer, ((cx - r - pad) * s, (cy - r - pad) * s), layer)
    box = ((cy - r) / HEIGHT, (cy + r) / HEIGHT, (cx - r) / WIDTH, (cx + r) / WIDTH)
    return page, box


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--out", default="examples/jud", help="the directory holding images/ and the cases document")
    parser.add_argument("--scale", type=int, default=1)
    parser.add_argument("--seed", type=int, default=20261015)
    parser.add_argument("--name", default="stamps", help="the rubric's name and the cases document's stem")
    args = parser.parse_args()
    rng = random.Random(args.seed)
    stamped = [True] * WITH_STAMP + [False] * (PAGES - WITH_STAMP)
    rng.shuffle(stamped)
    images = os.path.join(args.out, "images")
    os.makedirs(images, exist_ok=True)
    cases = []
    for i, stamp in enumerate(stamped):
        page, box = draw_page(rng, args.scale, stamp)
        name = f"page-{i:02d}"
        page.convert("P", palette=Image.ADAPTIVE, colors=64).save(os.path.join(images, f"{name}.png"), optimize=True)
        if box:
            top, bottom, left, right = (round(v, 4) for v in box)
            expect = f"        stamp_present: true\n        stamp: {{box: {{top: {top}, bottom: {bottom}, left: {left}, right: {right}}}}}\n"
            tags = "[stamp]"
        else:
            expect = "        stamp_present: false\n"
            tags = "[none]"
        cases.append(
            f"    - id: {name}\n      state: {{page: {name}}}\n      assets:\n"
            f"        {name}: {{type: image, path: images/{name}.png}}\n      expect:\n{expect}      tags: {tags}\n"
        )
    with open(os.path.join(args.out, f"{args.name}-cases.jud"), "w", encoding="utf-8") as f:
        f.write(
            "# Forty generated document pages for the stamps rubric: thirty with one\n"
            "# round ink stamp, labelled with its bounding box, and ten without, so the\n"
            "# Noul has both outcomes. Drawn by scripts/gen-document-pages.py from one\n"
            "# seed, which is why the labels are exact. Each case names its page once,\n"
            "# in `assets`, by a path relative to this file; the reference is what the\n"
            "# state carries.\n"
            "apiVersion: jud/v1\nkind: Cases\nmetadata:\n"
            f"  name: {args.name}-cases\n  minor: 4\n"
            "  description: Forty generated document pages, labelled by the generator that drew them.\n"
            f"spec:\n  rubric: {args.name}\n  cases:\n" + "".join(cases)
        )
    print(f"wrote {PAGES} pages ({WIDTH * args.scale} by {HEIGHT * args.scale}) under {images} and {args.name}-cases.jud")


if __name__ == "__main__":
    main()
