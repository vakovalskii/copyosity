# Notarizing a macOS app (Developer ID) — runbook

A self-contained guide for notarizing a Developer ID–signed macOS app + DMG.
Battle-tested on Copyosity (Tauri, Apple Silicon, hardened runtime).

## Prerequisites
- **Developer ID Application** certificate in the keychain:
  ```bash
  security find-identity -v -p codesigning | grep "Developer ID Application"
  # => "Developer ID Application: <Name> (<TEAMID>)"
  ```
- **Apple ID** (email of an account in team `<TEAMID>`).
- **App-specific password**: appleid.apple.com → Sign-In and Security →
  **App-Specific Passwords** → create (format `xxxx-xxxx-xxxx-xxxx`). NOT the
  normal Apple ID password.
- App must be signed with **hardened runtime** (`--options runtime`) + **timestamp**.

## Step 0 — store credentials in the keychain (once)
```bash
xcrun notarytool store-credentials "myprofile" \
  --apple-id "you@example.com" \
  --team-id "TEAMID" \
  --password "xxxx-xxxx-xxxx-xxxx"
# wait for: "Credentials validated."  -> then always pass --keychain-profile "myprofile"
```

## Step 1 — sign (skip if your toolchain signs, e.g. Tauri/electron-builder)
```bash
codesign --force --deep --options runtime --timestamp \
  --entitlements Entitlements.plist \
  --sign "Developer ID Application: <Name> (<TEAMID>)" \
  "MyApp.app"
codesign -dvvv "MyApp.app" 2>&1 | grep -E "Authority|flags|TeamIdentifier"
# flags must include "runtime"
```
> Tauri: signing identity + entitlements come from
> `tauri.conf.json → bundle.macOS.signingIdentity / entitlements`; `npm run tauri build` signs automatically.

## Step 2 — notarize the .app
```bash
ditto -c -k --keepParent "MyApp.app" "MyApp.zip"
xcrun notarytool submit "MyApp.zip" --keychain-profile "myprofile" --wait
# need: status: Accepted
xcrun stapler staple "MyApp.app"
```
On `status: Invalid`, get the reason:
```bash
xcrun notarytool log <submission-id> --keychain-profile "myprofile"
```

## Step 3 — DMG (important format gotcha)
notarytool only accepts **`.zip` / `.pkg` / UDIF `.dmg`**. A DMG built with
`hdiutil makehybrid` is NOT UDIF and gets rejected. Convert it:
```bash
# staging = a folder containing MyApp.app + a symlink to /Applications:
#   mkdir staging && cp -R MyApp.app staging/ && ln -s /Applications staging/Applications

# build (makehybrid doesn't mount a volume -> works where `create -srcfolder`
# fails with "Operation not permitted" in restricted environments)
hdiutil makehybrid -hfs -hfs-volume-name "MyApp" -o /tmp/hyb.dmg staging
# convert to UDIF (UDZO)
hdiutil convert /tmp/hyb.dmg -format UDZO -o "MyApp.dmg"
# sign the DMG
codesign --force --sign "Developer ID Application: <Name> (<TEAMID>)" "MyApp.dmg"
# notarize + staple
xcrun notarytool submit "MyApp.dmg" --keychain-profile "myprofile" --wait   # Accepted
xcrun stapler staple "MyApp.dmg"
```
> Staple BOTH the `.app` (before building the DMG) AND the DMG itself.

## Step 4 — verify (Gatekeeper)
```bash
spctl -a -vvv -t exec    "MyApp.app"   # => accepted, source=Notarized Developer ID
spctl -a -vvv -t install "MyApp.dmg"   # => accepted, source=Notarized Developer ID
xcrun stapler validate   "MyApp.dmg"
```

## Gotchas (verified in practice)
1. **`-1743 / errAEEventNotPermitted`, silent failure** — if the app sends Apple
   Events (AppleScript) under hardened runtime without the
   `com.apple.security.automation.apple-events` entitlement +
   `NSAppleEventsUsageDescription`. Does NOT affect notarization, but the feature
   silently fails.
2. **DMG "must be a UDIF disk image"** — convert the `makehybrid` image via
   `hdiutil convert -format UDZO`.
3. **`hdiutil create -srcfolder` → "Operation not permitted"** in environments
   without Full Disk Access — use `makehybrid` + `convert` (doesn't mount `/Volumes`).
4. **Notarization ≠ signing.** You can only notarize something already signed with
   Developer ID + `--options runtime`. Ad-hoc/dev signatures are rejected.
5. **App Store ≠ Developer ID.** Developer ID notarization accepts any entitlements;
   the entitlement-rejection issues people hit are App Store Connect only.
6. **TCC is tied to the signature** — re-signing / changing identity resets granted
   permissions (Automation/Calendar/etc.). Reset while testing:
   `tccutil reset AppleEvents <bundle-id>`.
7. **`stapler staple` is required** so the app opens offline (ticket embedded in the
   bundle); without it Gatekeeper needs an online check.
8. **`--wait` can take 2–8 minutes** — normal (Apple processes server-side).

## TL;DR happy-path
```bash
xcrun notarytool store-credentials "p" --apple-id E --team-id T --password P   # once
ditto -c -k --keepParent MyApp.app MyApp.zip
xcrun notarytool submit MyApp.zip --keychain-profile p --wait     # Accepted
xcrun stapler staple MyApp.app
hdiutil makehybrid -hfs -hfs-volume-name MyApp -o /tmp/h.dmg staging
hdiutil convert /tmp/h.dmg -format UDZO -o MyApp.dmg
codesign --force --sign "Developer ID Application: N (T)" MyApp.dmg
xcrun notarytool submit MyApp.dmg --keychain-profile p --wait     # Accepted
xcrun stapler staple MyApp.dmg
spctl -a -vvv -t install MyApp.dmg                                 # Notarized Developer ID
```
