#!/usr/bin/env python3
"""Headless Playwright smoke test for the Orbit Studio.

Usage:
    python3 tests/playwright_studio.py [--binary target/release/orbit] [--headed]

The script launches the real server, edits a rocket, validates it, runs the
Atlas lunar mission, checks the plots and downloads the CSV export. It prints
PASS on success and exits non-zero on any failure.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]


def start_server(binary: Path) -> tuple[subprocess.Popen, str]:
    proc = subprocess.Popen(
        [str(binary), "--no-open", "--port", "0"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        cwd=str(ROOT),
    )
    deadline = time.time() + 15
    url = None
    while time.time() < deadline:
        line = proc.stdout.readline()
        if not line:
            if proc.poll() is not None:
                err = proc.stderr.read()
                raise RuntimeError(f"server exited early: {err}")
            continue
        match = re.search(r"ORBIT_URL=(\S+)", line)
        if match:
            url = match.group(1)
            break
    if not url:
        proc.terminate()
        raise RuntimeError("server did not print an ORBIT_URL")
    return proc, url


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default=str(ROOT / "target" / "release" / "orbit"))
    parser.add_argument("--headed", action="store_true")
    args = parser.parse_args()
    binary = Path(args.binary)
    if not binary.exists():
        print(f"missing binary: {binary}; run `cargo build --release`", file=sys.stderr)
        return 2
    proc, url = start_server(binary)
    try:
        with sync_playwright() as pw:
            browser = pw.chromium.launch(headless=not args.headed)
            context = browser.new_context(accept_downloads=True, viewport={"width": 1440, "height": 1100})
            page = context.new_page()
            errors: list[str] = []
            page.on("pageerror", lambda err: errors.append(str(err)))
            page.goto(url, wait_until="domcontentloaded")
            page.wait_for_selector("#connection.ok", timeout=15000)
            assert page.locator("#preset-list .preset").count() >= 6, "presets did not load"

            # Build/edit a rocket.
            page.locator("#preset-list .preset", has_text="Atlas").first.click()
            page.fill("#rocket-name", "Atlas Studio")
            page.fill("#rocket-payload", "3500")
            page.click("#validate-btn")
            page.wait_for_selector("#garage-status.pill.ok, #garage-status.pill.bad", timeout=15000)
            assert "Atlas Studio" in page.inner_text("#mission-rocket-name")

            # Run the lunar mission.
            page.click(".tab-btn[data-tab='mission']")
            page.select_option("#script-select", "lunar-atlas-100")
            page.click("#run-btn")
            page.wait_for_selector("#result-pill.pill.ok", timeout=120000)
            assert page.inner_text("#result-pill").strip().lower() == "success"
            peri = page.inner_text("#result-peri")
            assert "km" in peri and peri.strip() != "—", f"bad periapsis: {peri}"
            assert page.locator("#event-table tr").count() > 3, "event log empty"

            # Plots must contain actual drawn pixels.
            page.click(".tab-btn[data-tab='plots']")
            page.wait_for_timeout(300)
            pixel = page.evaluate(
                """() => {
                    const c = document.querySelector('#plot-trajectory');
                    const ctx = c.getContext('2d');
                    const data = ctx.getImageData(0, 0, c.width, c.height).data;
                    let count = 0;
                    for (let i = 0; i < data.length; i += 4) {
                        if (data[i] + data[i+1] + data[i+2] > 90) count++;
                    }
                    return count;
                }"""
            )
            assert pixel > 500, f"trajectory plot has too few pixels: {pixel}"

            # CSV export.
            page.click(".tab-btn[data-tab='mission']")
            with tempfile.TemporaryDirectory() as tmp:
                with page.expect_download(timeout=30000) as download_info:
                    page.click("#export-csv")
                download = download_info.value
                target = Path(tmp) / "trajectory.csv"
                download.save_as(target)
                text = target.read_text()
                assert text.startswith("time_s,phase,altitude_m"), text[:80]
                assert len(text.splitlines()) > 20

            page.screenshot(path=str(ROOT / "tests" / "studio.png"), full_page=True)
            assert not errors, f"page errors: {errors}"
            browser.close()
        print("PASS: Studio rocket edit, validation, mission, plots and CSV export")
        return 0
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()


if __name__ == "__main__":
    sys.exit(main())
