# Scripts

| Script | Purpose |
|---|---|
| `verify.sh` | release build, debug and release tests, fuzz corpus, 20 missions |
| `demo.sh` | build and launch the Studio |

Neither script requires network access. The browser test is separate because it
needs Python Playwright and Chromium:

```sh
python3 tests/playwright_studio.py
```
