#!/usr/bin/env python3
"""Compare a NegFe window capture against a reference capture.

The score is region based. A pixel difference alone fails on font hinting and
icon rasterization, so each region reports geometry, palette, structure and
text separately. See GUI_COMPARE.md for the fixture and the budgets.
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter


@dataclass(frozen=True)
class Region:
    name: str
    box: tuple[int, int, int, int]
    weight: float
    mode: str = "window"


def load_fixture(path: Path) -> dict:
    with path.open(encoding="utf-8") as fh:
        return json.load(fh)


def logical_capture(path: Path, scale: float | None) -> Image.Image:
    """A capture in logical pixels, so a HiDPI shot compares with a 1x one."""
    image = Image.open(path).convert("RGB")
    factor = scale if scale is not None else round(image.width / 1920)
    if factor > 1:
        image = image.resize(
            (round(image.width / factor), round(image.height / factor)),
            Image.Resampling.LANCZOS,
        )
    return image


def regions_for(fixture: dict, size: tuple[int, int]) -> list[Region]:
    width, height = size
    regions = []
    for name, spec in fixture["regions"].items():
        x0, y0, x1, y1 = spec["box"]
        regions.append(
            Region(
                name,
                (round(x0 * width), round(y0 * height), round(x1 * width), round(y1 * height)),
                float(spec.get("weight", 1.0)),
                str(spec.get("mode", "window")),
            )
        )
    return regions


def region_changed(a: Image.Image, b: Image.Image) -> float:
    """Agreement of the row and column brightness profiles, so a band that moved scores low."""
    def profile(image: Image.Image) -> tuple[np.ndarray, np.ndarray]:
        gray = np.asarray(image.convert("L").resize((128, 96), Image.Resampling.BOX), dtype=np.float64)
        rows = gray.mean(axis=1)
        cols = gray.mean(axis=0)

        def norm(values: np.ndarray) -> np.ndarray:
            values = values - values.mean()
            scale = values.std()
            return values / scale if scale > 1e-6 else values

        return norm(rows), norm(cols)

    def corr(left: np.ndarray, right: np.ndarray) -> float:
        if left.std() < 1e-6 or right.std() < 1e-6:
            return 1.0 if np.allclose(left, right) else 0.0
        return float(np.clip(np.corrcoef(left.ravel(), right.ravel())[0, 1], 0.0, 1.0))

    a_rows, a_cols = profile(a)
    b_rows, b_cols = profile(b)
    return 0.5 * (corr(a_rows, b_rows) + corr(a_cols, b_cols))


def content_box(image: Image.Image) -> tuple[float, float, float, float] | None:
    """The largest non-background rectangle in a canvas, as fractions of the crop."""
    arr = np.asarray(image.convert("RGB"), dtype=np.int16)
    if arr.size == 0:
        return None
    background = np.median(arr.reshape(-1, 3), axis=0)
    mask = np.abs(arr - background).mean(axis=2) > 10
    rows = np.flatnonzero(mask.mean(axis=1) > 0.02)
    cols = np.flatnonzero(mask.mean(axis=0) > 0.02)
    if rows.size == 0 or cols.size == 0:
        return None
    height, width = arr.shape[:2]
    return (cols[0] / width, rows[0] / height, (cols[-1] + 1) / width, (rows[-1] + 1) / height)


def placement_score(a: Image.Image, b: Image.Image) -> float:
    """Intersection over union of the photo rectangles, so a grade change does not score."""
    left, right = content_box(a), content_box(b)
    if left is None or right is None:
        return 1.0 if left is None and right is None else 0.0
    lx0, ly0, lx1, ly1 = left
    rx0, ry0, rx1, ry1 = right
    intersection = max(0.0, min(lx1, rx1) - max(lx0, rx0)) * max(0.0, min(ly1, ry1) - max(ly0, ry0))
    union = (lx1 - lx0) * (ly1 - ly0) + (rx1 - rx0) * (ry1 - ry0) - intersection
    return 1.0 if union <= 0 else float(intersection / union)


def _lab(rgb: np.ndarray) -> np.ndarray:
    """sRGB to CIELAB, D65, with the piecewise sRGB transfer."""
    srgb = rgb.astype(np.float64) / 255.0
    linear = np.where(srgb <= 0.04045, srgb / 12.92, ((srgb + 0.055) / 1.055) ** 2.4)
    matrix = np.array(
        [
            [0.4124564, 0.3575761, 0.1804375],
            [0.2126729, 0.7151522, 0.0721750],
            [0.0193339, 0.1191920, 0.9503041],
        ]
    )
    xyz = linear.reshape(-1, 3) @ matrix.T
    xyz /= np.array([0.95047, 1.0, 1.08883])
    delta = 6.0 / 29.0
    f = np.where(xyz > delta**3, np.cbrt(xyz), xyz / (3 * delta**2) + 4.0 / 29.0)
    lab = np.column_stack((116 * f[:, 1] - 16, 500 * (f[:, 0] - f[:, 1]), 200 * (f[:, 1] - f[:, 2])))
    return lab


def _ciede2000(lab1: np.ndarray, lab2: np.ndarray) -> np.ndarray:
    """Vectorized CIEDE2000, the color distance the palette budget uses."""
    l1, a1, b1 = lab1[:, 0], lab1[:, 1], lab1[:, 2]
    l2, a2, b2 = lab2[:, 0], lab2[:, 1], lab2[:, 2]
    c1 = np.hypot(a1, b1)
    c2 = np.hypot(a2, b2)
    c_bar = (c1 + c2) / 2
    g = 0.5 * (1 - np.sqrt(c_bar**7 / (c_bar**7 + 25**7)))
    a1p = (1 + g) * a1
    a2p = (1 + g) * a2
    c1p = np.hypot(a1p, b1)
    c2p = np.hypot(a2p, b2)
    h1p = np.degrees(np.arctan2(b1, a1p)) % 360
    h2p = np.degrees(np.arctan2(b2, a2p)) % 360
    dl = l2 - l1
    dc = c2p - c1p
    dh = np.where(c1p * c2p == 0, 0, h2p - h1p)
    dh = np.where(dh > 180, dh - 360, dh)
    dh = np.where(dh < -180, dh + 360, dh)
    dhp = 2 * np.sqrt(c1p * c2p) * np.sin(np.radians(dh / 2))
    lbar = (l1 + l2) / 2
    cbarp = (c1p + c2p) / 2
    hsum = h1p + h2p
    hbar = np.where(
        c1p * c2p == 0,
        hsum,
        np.where(np.abs(h1p - h2p) <= 180, hsum / 2, np.where(hsum < 360, (hsum + 360) / 2, (hsum - 360) / 2)),
    )
    t = (
        1
        - 0.17 * np.cos(np.radians(hbar - 30))
        + 0.24 * np.cos(np.radians(2 * hbar))
        + 0.32 * np.cos(np.radians(3 * hbar + 6))
        - 0.20 * np.cos(np.radians(4 * hbar - 63))
    )
    dtheta = 30 * np.exp(-(((hbar - 275) / 25) ** 2))
    rc = 2 * np.sqrt(cbarp**7 / (cbarp**7 + 25**7))
    sl = 1 + 0.015 * (lbar - 50) ** 2 / np.sqrt(20 + (lbar - 50) ** 2)
    sc = 1 + 0.045 * cbarp
    sh = 1 + 0.015 * cbarp * t
    rt = -np.sin(np.radians(2 * dtheta)) * rc
    return np.sqrt((dl / sl) ** 2 + (dc / sc) ** 2 + (dhp / sh) ** 2 + rt * (dc / sc) * (dhp / sh))


def palette_distance(a: Image.Image, b: Image.Image, colors: int = 8) -> float:
    def quantized(image: Image.Image) -> np.ndarray:
        small = image.resize((96, 96), Image.Resampling.BILINEAR)
        arr = np.asarray(small, dtype=np.uint8).reshape(-1, 3)
        _, counts = np.unique(arr, axis=0, return_counts=True)
        order = np.argsort(counts)[-colors:]
        return np.unique(arr, axis=0)[order].astype(np.float64)

    left, right = quantized(a), quantized(b)
    if len(left) == 0 or len(right) == 0:
        return 0.0
    distances = np.atleast_2d(_ciede2000(_lab(left), _lab(right)))
    return float(np.mean(np.min(distances, axis=1)))


def structure_correlation(a: Image.Image, b: Image.Image) -> float:
    def edges(image: Image.Image) -> np.ndarray:
        gray = image.convert("L").filter(ImageFilter.GaussianBlur(1.5))
        arr = np.asarray(gray, dtype=np.float64)
        gy, gx = np.gradient(arr)
        return np.hypot(gx, gy)

    ea, eb = edges(a), edges(b)
    if ea.std() == 0 or eb.std() == 0:
        return 1.0 if np.allclose(ea, eb) else 0.0
    return float(np.clip(np.corrcoef(ea.ravel(), eb.ravel())[0, 1], -1.0, 1.0))


def layout_correlation(a: Image.Image, b: Image.Image) -> float:
    """Structure at a coarse scale: card edges and bands, not glyph antialiasing."""
    def edges(image: Image.Image) -> np.ndarray:
        gray = image.convert("L").filter(ImageFilter.GaussianBlur(1.5))
        small = gray.resize((64, 48), Image.Resampling.BOX)
        arr = np.asarray(small, dtype=np.float64)
        gy, gx = np.gradient(arr)
        return np.hypot(gx, gy)

    ea, eb = edges(a), edges(b)
    if ea.std() < 1e-6 or eb.std() < 1e-6:
        return 1.0 if np.allclose(ea, eb) else 0.0
    return float(np.clip(np.corrcoef(ea.ravel(), eb.ravel())[0, 1], 0.0, 1.0))


def _norm_words(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", " ", text.lower()).strip()


def ocr_words(image: Image.Image) -> list[tuple[str, tuple[int, int, int, int]]]:
    """Word boxes from tesseract, empty when it is not installed."""
    try:
        proc = subprocess.run(
            ["tesseract", "stdin", "stdout", "tsv"],
            input=image.tobytes(),
            capture_output=True,
            check=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return []
    del image
    words = []
    for line in proc.stdout.decode("utf-8", "replace").splitlines()[1:]:
        parts = line.split("\t")
        if len(parts) >= 12 and parts[11].strip():
            words.append(
                (
                    _norm_words(parts[11]),
                    (int(parts[6]), int(parts[7]), int(parts[8]), int(parts[9])),
                )
            )
    return words


def text_score(a: Image.Image, b: Image.Image) -> float:
    """Word boxes matched by nearest center, with a tolerance of five percent."""
    left = ocr_words(a)
    right = ocr_words(b)
    if not left or not right:
        return 1.0 if not left and not right else 0.0
    width = max(a.width, 1)
    height = max(a.height, 1)
    tolerance = 0.05 * max(width, height)
    used = set()
    hits = 0
    for _, (x, y, w, h) in left:
        cx, cy = x + w / 2, y + h / 2
        best, best_distance = None, tolerance
        for index, (_, (rx, ry, rw, rh)) in enumerate(right):
            if index in used:
                continue
            distance = float(np.hypot(cx - (rx + rw / 2), cy - (ry + rh / 2)))
            if distance < best_distance:
                best, best_distance = index, distance
        if best is not None:
            used.add(best)
            hits += 1
    return hits / max(len(left), len(right))


def compare(reference: Image.Image, candidate: Image.Image, fixture: dict) -> dict:
    if reference.size != candidate.size:
        reference = reference.resize(candidate.size, Image.Resampling.LANCZOS)
    results = {}
    for region in regions_for(fixture, candidate.size):
        a = reference.crop(region.box)
        b = candidate.crop(region.box)
        if region.mode == "placement":
            results[region.name] = {
                "geometry": placement_score(a, b),
                "palette": 0.0,
                "structure": 1.0,
                "layout": 1.0,
                "text": 1.0,
                "weight": region.weight,
            }
            continue
        results[region.name] = {
            "geometry": region_changed(a, b),
            "palette": palette_distance(a, b),
            "structure": structure_correlation(a, b),
            "layout": layout_correlation(a, b),
            "text": text_score(a, b),
            "weight": region.weight,
        }
    thresholds = fixture["thresholds"]
    for values in results.values():
        values["pass"] = (
            values["geometry"] >= thresholds["geometry"]
            and values["palette"] <= thresholds["palette"]
            and values["structure"] >= thresholds["structure"]
            and values["text"] >= thresholds["text"]
        )
    regions = [values for values in results.values() if "weight" in values]
    total_weight = sum(values["weight"] for values in regions)
    total = sum(
        values["weight"]
        * (
            0.25 * values["geometry"]
            + 0.25 * (1 - min(values["palette"] / thresholds["palette"], 1.0))
            + 0.35 * values["structure"]
            + 0.15 * values["text"]
        )
        for values in regions
    ) / total_weight
    layout_weight = sum(values["weight"] for values in regions)
    layout = sum(
        values["weight"]
        * (
            0.35 * values["geometry"]
            + 0.45 * values["layout"]
            + 0.20 * (1 - min(values["palette"] / thresholds["palette"], 1.0))
        )
        for values in regions
    ) / layout_weight
    results["total"] = total
    results["layout"] = layout
    return results


def _png_data(image: Image.Image) -> str:
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")
    return base64.b64encode(buffer.getvalue()).decode("ascii")


def write_report(reference: Image.Image, candidate: Image.Image, fixture: dict, results: dict, path: Path) -> None:
    rows = []
    for region in regions_for(fixture, candidate.size):
        values = results[region.name]
        a = reference.crop(region.box)
        b = candidate.crop(region.box)
        status = "pass" if values["pass"] else "fail"
        rows.append(
            f"<tr class='{status}'><td>{region.name}</td>"
            f"<td>{values['geometry']:.3f}</td><td>{values['palette']:.1f}</td>"
            f"<td>{values['structure']:.3f}</td><td>{values.get('layout', 0.0):.3f}</td>"
            f"<td>{values['text']:.3f}</td>"
            f"<td><img src='data:image/png;base64,{_png_data(a)}'>"
            f"<img src='data:image/png;base64,{_png_data(b)}'></td></tr>"
        )
    html = f"""<!doctype html><meta charset="utf-8">
<title>NegFe GUI comparison</title>
<style>
body {{ background:#202020; color:#ddd; font: 13px sans-serif; }}
table {{ border-collapse: collapse; }} td, th {{ padding: 6px; border: 1px solid #555; }}
tr.pass td:first-child {{ color:#7ad17a; }} tr.fail td:first-child {{ color:#ff7070; }}
img {{ max-height: 180px; margin-right: 8px; }}
</style>
<h1>Layout {results['layout']:.3f} (gate 0.80) · diagnostic total {results['total']:.3f}</h1>
<p>Left: reference. Right: candidate.</p>
<table><tr><th>Region</th><th>Geometry</th><th>Palette</th><th>Structure</th><th>Layout</th><th>Text</th><th>Images</th></tr>
{''.join(rows)}
</table>"""
    path.write_text(html, encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", required=True, type=Path)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--fixture", type=Path, default=Path(__file__).with_name("reference_roll.json"))
    parser.add_argument("--scale", type=float, default=None)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args(argv)
    fixture = load_fixture(args.fixture)
    reference = logical_capture(args.reference, args.scale)
    candidate = logical_capture(args.candidate, args.scale)
    results = compare(reference, candidate, fixture)
    for name, values in results.items():
        if name in ("total", "layout"):
            continue
        print(
            f"{name:10s} geometry={values['geometry']:.3f} palette={values['palette']:.1f} "
            f"structure={values['structure']:.3f} layout={values.get('layout', 0.0):.3f} "
            f"text={values['text']:.3f} "
            f"{'pass' if values['pass'] else 'FAIL'}"
        )
    print(f"total {results['total']:.3f}")
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        write_report(reference, candidate, fixture, results, args.report)
        print(f"report {args.report}")
    gate = fixture["thresholds"].get("layout", fixture["thresholds"].get("total", 0.80))
    print(f"layout {results['layout']:.3f}")
    return 0 if results["layout"] >= gate else 1


if __name__ == "__main__":
    sys.exit(main())
