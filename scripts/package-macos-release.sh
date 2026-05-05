#!/usr/bin/env bash
set -euo pipefail

tag="${1:?usage: package-macos-release.sh <tag>}"
target_triple="${2:-aarch64-apple-darwin}"
version="${tag#v}"
asset_name="mergen-ade-${tag}-macos-arm64.dmg"
asset_path="$(pwd)/${asset_name}"
binary_path="target/${target_triple}/release/mergen-ade"
stage_root="$(pwd)/release-staging/macos"
app_name="Mergen ADE.app"
app_dir="${stage_root}/${app_name}"
contents_dir="${app_dir}/Contents"
macos_dir="${contents_dir}/MacOS"
resources_dir="${contents_dir}/Resources"
iconset_dir="${stage_root}/MergenADE.iconset"
icns_path="${resources_dir}/MergenADE.icns"
dmg_root="${stage_root}/dmg-root"
bundle_id="${MACOS_BUNDLE_ID:-com.mergen.MergenADE}"
sign_and_notarize="${MACOS_SIGN_AND_NOTARIZE:-0}"
notary_submit_log="${stage_root}/notary-submit.json"
notary_detail_log="${stage_root}/notary-detail.json"

require_env() {
    local name="$1"
    if [[ -z "${!name:-}" ]]; then
        echo "Missing required environment variable: ${name}" >&2
        exit 1
    fi
}

extract_notary_id() {
    local file="$1"
    if [[ -f "${file}" ]]; then
        sed -n 's/.*"id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "${file}" | head -n 1
    fi
}

if [[ ! -f "${binary_path}" ]]; then
    echo "macOS release binary not found at ${binary_path}" >&2
    exit 1
fi

rm -rf "${stage_root}"
mkdir -p "${macos_dir}" "${resources_dir}" "${iconset_dir}" "${dmg_root}"

cp "${binary_path}" "${macos_dir}/Mergen ADE"
chmod +x "${macos_dir}/Mergen ADE"

sips -z 16 16 logo.png --out "${iconset_dir}/icon_16x16.png" >/dev/null
sips -z 32 32 logo.png --out "${iconset_dir}/icon_16x16@2x.png" >/dev/null
sips -z 32 32 logo.png --out "${iconset_dir}/icon_32x32.png" >/dev/null
sips -z 64 64 logo.png --out "${iconset_dir}/icon_32x32@2x.png" >/dev/null
sips -z 128 128 logo.png --out "${iconset_dir}/icon_128x128.png" >/dev/null
sips -z 256 256 logo.png --out "${iconset_dir}/icon_128x128@2x.png" >/dev/null
sips -z 256 256 logo.png --out "${iconset_dir}/icon_256x256.png" >/dev/null
sips -z 512 512 logo.png --out "${iconset_dir}/icon_256x256@2x.png" >/dev/null
sips -z 512 512 logo.png --out "${iconset_dir}/icon_512x512.png" >/dev/null
sips -z 1024 1024 logo.png --out "${iconset_dir}/icon_512x512@2x.png" >/dev/null
iconutil -c icns "${iconset_dir}" -o "${icns_path}"

cat > "${contents_dir}/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>Mergen ADE</string>
    <key>CFBundleExecutable</key>
    <string>Mergen ADE</string>
    <key>CFBundleIconFile</key>
    <string>MergenADE</string>
    <key>CFBundleIdentifier</key>
    <string>${bundle_id}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Mergen ADE</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${version}</string>
    <key>CFBundleVersion</key>
    <string>${version}</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

if [[ "${sign_and_notarize}" == "1" ]]; then
    require_env "MACOS_CODESIGN_IDENTITY"
    require_env "MACOS_NOTARY_KEY_ID"
    require_env "MACOS_NOTARY_ISSUER"
    require_env "MACOS_NOTARY_KEY_PATH"

    xattr -cr "${app_dir}"
    codesign --force --timestamp --options runtime --sign "${MACOS_CODESIGN_IDENTITY}" "${macos_dir}/Mergen ADE"
    codesign --force --timestamp --options runtime --sign "${MACOS_CODESIGN_IDENTITY}" "${app_dir}"
    codesign --verify --deep --strict --verbose=2 "${app_dir}"
fi

cp -R "${app_dir}" "${dmg_root}/"
ln -s /Applications "${dmg_root}/Applications"
rm -f "${asset_path}"
hdiutil create -volname "Mergen ADE" -srcfolder "${dmg_root}" -ov -format UDZO "${asset_path}" >/dev/null

if [[ "${sign_and_notarize}" == "1" ]]; then
    set +e
    xcrun notarytool submit \
        "${asset_path}" \
        --key "${MACOS_NOTARY_KEY_PATH}" \
        --key-id "${MACOS_NOTARY_KEY_ID}" \
        --issuer "${MACOS_NOTARY_ISSUER}" \
        --wait \
        --output-format json \
        >"${notary_submit_log}" 2>&1
    notary_status=$?
    set -e

    if [[ ${notary_status} -ne 0 ]]; then
        submission_id="$(extract_notary_id "${notary_submit_log}")"
        if [[ -n "${submission_id}" ]]; then
            xcrun notarytool log \
                "${submission_id}" \
                --key "${MACOS_NOTARY_KEY_PATH}" \
                --key-id "${MACOS_NOTARY_KEY_ID}" \
                --issuer "${MACOS_NOTARY_ISSUER}" \
                >"${notary_detail_log}" 2>&1 || true
        fi
        cat "${notary_submit_log}" >&2
        if [[ -f "${notary_detail_log}" ]]; then
            cat "${notary_detail_log}" >&2
        fi
        exit "${notary_status}"
    fi

    xcrun stapler staple "${asset_path}"
    xcrun stapler validate "${asset_path}"
fi

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    {
        echo "asset_name=${asset_name}"
        echo "asset_path=${asset_path}"
    } >> "${GITHUB_OUTPUT}"
fi

echo "Created ${asset_name}"
