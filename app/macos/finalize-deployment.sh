#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 3 ]]; then
    echo "Usage: $0 <app-bundle> <output-directory> <entitlements>" >&2
    exit 2
fi

app_bundle=$1
output_directory=$2
entitlements=$3
identity=${CRAFTWARD_SIGN_IDENTITY:--}

if [[ ! -d $app_bundle ]]; then
    echo "Application bundle not found: $app_bundle" >&2
    exit 1
fi

info_plist="$app_bundle/Contents/Info.plist"
app_name=$(basename "$app_bundle" .app)
app_version=$(plutil -extract CFBundleShortVersionString raw "$info_plist")
build_version=$(plutil -extract CFBundleVersion raw "$info_plist")
executable_name=$(plutil -extract CFBundleExecutable raw "$info_plist")
architectures=$(lipo -archs "$app_bundle/Contents/MacOS/$executable_name")

if [[ " $architectures " == *" arm64 "* &&
      " $architectures " == *" x86_64 "* ]]; then
    architecture_name=Universal2
elif [[ $architectures == "arm64" ]]; then
    architecture_name=Arm64
elif [[ $architectures == "x86_64" ]]; then
    architecture_name=X86_64
else
    echo "Unsupported application architectures: $architectures" >&2
    exit 1
fi

dmg_path="$output_directory/$app_name-$app_version-Build.$build_version-$architecture_name.dmg"

plutil -lint "$entitlements"

app_sign_options=(
    --force
    --sign "$identity"
    --entitlements "$entitlements"
)
dmg_sign_options=(
    --force
    --sign "$identity"
)

if [[ $identity == "-" ]]; then
    app_sign_options+=(--timestamp=none)
    dmg_sign_options+=(--timestamp=none)
else
    app_sign_options+=(--options runtime --timestamp)
    dmg_sign_options+=(--timestamp)
fi

codesign "${app_sign_options[@]}" "$app_bundle"
codesign --verify --deep --strict --verbose=4 "$app_bundle"

hdiutil create \
    -volname "Craftward" \
    -srcfolder "$app_bundle" \
    -fs APFS \
    -format ULFO \
    -ov \
    "$dmg_path"

codesign "${dmg_sign_options[@]}" "$dmg_path"
codesign --verify --verbose=4 "$dmg_path"

echo "Deployment image: $dmg_path"
