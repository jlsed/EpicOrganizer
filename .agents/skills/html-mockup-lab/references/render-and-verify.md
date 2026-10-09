# Render and verify — exact recipes

All commands assume Windows + PowerShell 7, the workspace from SKILL.md Phase 1, and Chrome
(`C:\Program Files\Google\Chrome\Application\chrome.exe`). Python 3 is available for the server.

## 1. Serve the workspace over http

Fonts (`@font-face`) do not load reliably from `file://` in headless Chrome — always serve:

```powershell
$base = "C:\Users\sed\AppData\Local\Temp\opencode\<name>-mockups"
Start-Process -WindowStyle Hidden -FilePath python -ArgumentList '-m','http.server','8377','--directory',$base
# verify:
(Invoke-WebRequest -Uri "http://127.0.0.1:8377/mock.css" -UseBasicParsing).StatusCode   # 200
```

Cleanup when done: `Get-NetTCPConnection -LocalPort 8377 | ForEach-Object { Stop-Process -Id $_.OwningProcess }`.

## 2. Frame selector (put this in every variant file)

```html
<style>
  .phone { display: none; }
  .phone.show { display: flex; }   /* flex, not block — the frames lay out as columns */
</style>
<script>
  (function () {
    var i = parseInt(new URLSearchParams(location.search).get('i') || '0', 10);
    var phones = document.querySelectorAll('.phone');
    if (phones[i]) phones[i].classList.add('show');
  })();
</script>
```

**Why not stack + scroll:** under `--virtual-time-budget`, Chrome paints only the initial viewport;
scrolling to frame N before the shot leaves the new tiles unpainted (blank grey PNG). The visibility
toggle keeps the requested frame at y=0 and always paints.

## 3. Render one frame

```powershell
$chrome = "C:\Program Files\Google\Chrome\Application\chrome.exe"
$prof   = "$env:TEMP\opencode\chrome-mock-profile"
& $chrome --headless --disable-gpu --hide-scrollbars --no-first-run --no-default-browser-check `
  "--user-data-dir=$prof" --window-size=360,800 --force-device-scale-factor=2 `
  --virtual-time-budget=4000 `
  "--screenshot=$base\shots\A_expense_light.png" `
  "http://127.0.0.1:8377/variant-a.html?i=0"
```

- `--window-size` must equal the frame size (360×800) — the screenshot is exactly the viewport.
- `--force-device-scale-factor=2` doubles the PNG (720×1600); keep it for crisp boards.
- `--hide-scrollbars` keeps a 15 px scrollbar out of the right edge.
- `--virtual-time-budget=4000` gives fonts + layout time to settle before the shot.

## 4. Batch-render a variant file

```powershell
$frames = @(@("a",0,"A_expense_light"), @("a",1,"A_expense_dark"), @("a",2,"A_transfer_light"), @("a",3,"A_typing_light"))
foreach ($f in $frames) {
  & $chrome --headless --disable-gpu --hide-scrollbars --no-first-run --no-default-browser-check `
    "--user-data-dir=$prof" --window-size=360,800 --force-device-scale-factor=2 `
    --virtual-time-budget=4000 "--screenshot=$base\shots\$($f[2]).png" `
    "http://127.0.0.1:8377/variant-$($f[0]).html?i=$($f[1])" 2>$null | Out-Null
  if (Test-Path "$base\shots\$($f[2]).png") { "OK $($f[2])" } else { "FAIL $($f[2])" }
}
```

Naming: `{VARIANT}_{state}_{theme}.png` (`A_expense_light`, `C_transfer_light`, …) so the board and
the review stay readable.

## 5. Review — always read the image back

Use the Read tool on each PNG (it renders images). Fix the HTML → re-render → re-read. If a CSS edit
does not appear, the screenshot hit cache: bump the URL (`?i=0&v=2`) or delete the profile dir.

## 6. Option board

Build `board.html` **after** the individual frames exist; embed them at half size (a 720×1600 PNG at
`width: 360px` stays crisp):

```html
<img class="shot" src="shots/A_expense_light.png">
```

Render the board with its own window size (content width/height, roughly measured) at DPR 1:

```powershell
& $chrome --headless --disable-gpu --hide-scrollbars --no-first-run --no-default-browser-check `
  "--user-data-dir=$prof" --window-size=1228,3760 --virtual-time-budget=8000 `
  "--screenshot=$base\shots\board.png" "http://127.0.0.1:8377/board.html"
```

## 7. Gotchas checklist

- **Blank frame N>0** → you stacked + scrolled; switch to the visibility selector (§2).
- **Thin/missing custom font** → you loaded over `file://`; serve over http.
- **All frames identical** → the `?i=` script runs before the frames exist in the DOM; keep it at the
  end of `<body>`.
- **Frame rendered as a broken stack** → `.phone` lost its internal `flex` (a page-level
  `display: block` override beats the flex layout); selector CSS must set `display: flex` for `.show`.
- **Emoji look different from the app** → the browser uses the system emoji font (Segoe UI Emoji on
  Windows); the app uses the same — expected.
- **Text metrics slightly off vs Compose** → same TTF via `@font-face`, so close; still confirm the
  winner on the emulator (`emulator-visual-check`) — static images never validate IME, ripples or
  gestures.
- **Tap coordinates when later driving the emulator**: `image px = device px`; chat-rendered images
  are often scaled (a displayed coordinate is not a device coordinate) — read the PNG's real pixel
  positions or derive from `adb shell wm size`.
