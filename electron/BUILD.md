# Mergen ADE - Build Guide

## Build Output

| Target | Output | Format |
|--------|--------|--------|
| Windows x64 | `mergen-ade-<version>-windows-x64-portable.exe` | Single portable EXE |
| macOS ARM64 | `mergen-ade-<version>-macos-arm64.dmg` | Signed & notarized DMG |

## Quick Build (Windows)

```powershell
cd electron
npm ci
npm run build
```

Output: `electron/out/mergen-ade-<version>-windows-x64-portable.exe`

## Build Steps

1. **Install dependencies**
   ```powershell
   cd electron
   npm ci
   ```

2. **Build the app**
   ```powershell
   npm run build
   ```

   This runs:
   - `tsc` - TypeScript compilation
   - `vite build` - Bundle renderer and main process
   - `electron-builder` - Package into portable EXE

3. **Output location**
   ```
   electron/out/mergen-ade-<version>-windows-x64-portable.exe
   ```

## Build Configuration

Build settings are in `electron/package.json` under `"build"`:

```json
{
  "build": {
    "appId": "com.Mergen.MergenADE",
    "productName": "Mergen ADE",
    "win": {
      "target": [
        {
          "target": "portable",
          "arch": ["x64"]
        }
      ],
      "artifactName": "mergen-ade-${version}-windows-x64-portable.${ext}"
    }
  }
}
```

### Key Settings

| Setting | Value | Description |
|---------|-------|-------------|
| `target` | `portable` | Single EXE, no installer |
| `arch` | `x64` | 64-bit Windows only |
| `artifactName` | `mergen-ade-${version}-...` | Version-stamped filename |

## Portable EXE Details

- **No installation required** - Run directly
- **Single file** - Everything bundled in one EXE
- **No registry entries** - Clean uninstall (just delete)
- **No Start Menu shortcuts** - Optional manual shortcut creation
- **Config location** - `%APPDATA%\Mergen\MergenADE\`

## Native Dependencies

`node-pty` requires native compilation. It's unpacked from ASAR:

```json
{
  "asarUnpack": [
    "node_modules/node-pty/**/*"
  ]
}
```

## Development

```powershell
cd electron
npm run dev
```

Runs Vite dev server with hot-reload.

## Testing

```powershell
cd electron
npm run test
# or
npx vitest run
```

## macOS Build

macOS builds require:

1. Apple Developer ID Application certificate
2. App Store Connect API key for notarization
3. GitHub secrets configured:
   - `APPLE_DEVELOPER_ID_APP_CERT_BASE64`
   - `APPLE_DEVELOPER_ID_APP_CERT_PASSWORD`
   - `APPLE_DEVELOPER_IDENTITY`
   - `APPLE_NOTARY_API_KEY_ID`
   - `APPLE_NOTARY_API_ISSUER_ID`
   - `APPLE_NOTARY_API_PRIVATE_KEY_BASE64`

## GitHub Actions Release

Triggered by pushing a `v*` tag:

```powershell
git tag v0.1.40
git push origin v0.1.40
```

This builds and publishes:
- Windows portable EXE
- macOS signed/notarized DMG

## Troubleshooting

### Build fails

1. Delete `node_modules` and retry:
   ```powershell
   Remove-Item -Recurse -Force node_modules
   npm ci
   npm run build
   ```

2. Ensure Node.js 18+ is installed

3. Check `electron-builder` is available:
   ```powershell
   npx electron-builder --version
   ```

### node-pty compilation fails

- Requires Python and C++ build tools
- Install: `npm install -g windows-build-tools`
- Or install Visual Studio Build Tools manually

### EXE doesn't run

- Check Windows Defender / antivirus isn't blocking
- Try running as administrator
- Check `%APPDATA%\Mergen\MergenADE\` for config issues

## File Structure After Build

```
electron/
├── out/
│   ├── win-unpacked/          # Unpacked app files
│   └── mergen-ade-0.1.40-windows-x64-portable.exe
├── renderer/
│   ├── dist/                  # Bundled renderer
│   └── dist-electron/         # Bundled main process
└── package.json               # Build config
```
