# Build Corvene's Windows packages from a release build:
#   target\windows\Corvene-<version>-<x86_64|aarch64|i686>-setup.exe   per-user installer (Inno Setup)
#   target\windows\Corvene-<version>-<arch>.msi                        machine-wide package (WiX)
#   target\windows\Corvene-<version>-windows-<arch>-portable.zip       no installer
# and with FULL=1 the same as Corvene-Full-<version>-….
#
# Needs Inno Setup 6 (`iscc.exe` on the PATH, in its default folder, or
# $env:ISCC) and WiX Toolset 5 (`wix` on the PATH:
# `dotnet tool install --global wix --version 5.0.2`; without it there is no
# .msi).
#
# Env:
#   TARGET=<Rust target>  another architecture than this machine's
#                         (aarch64-pc-windows-msvc, i686-pc-windows-msvc;
#                         needs `rustup target add` and its MSVC libraries)
#   FULL=1                the "full" variant: every tree-sitter grammar
#                         compiled in (needs tools/ts-queries/fetch.py's sources)
#   SKIP_BUILD=1          reuse the release binary that is there
#   PACKAGE_BIN=<binary>  package that binary instead (CI's debug build)
#   FORMATS="setup msi zip"  a subset of the packages
#   SIGN_PFX=<file>, SIGN_PFX_PASSWORD
#                         Authenticode-sign corvene.exe and the installers with
#                         this certificate (signtool from the Windows SDK);
#                         without it they are unsigned
#   SIGN_TIMESTAMP_URL    the timestamp server (default: DigiCert's)
$ErrorActionPreference = "Stop"
$root = (Resolve-Path "$PSScriptRoot\..\..").Path
Set-Location $root

$metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
$version = ($metadata.packages | Where-Object { $_.name -eq "corvene" }).version
$machine = switch ($env:PROCESSOR_ARCHITECTURE) {
    "AMD64" { "x86_64" }
    "ARM64" { "aarch64" }
    "x86" { "i686" }
    default { throw "unsupported architecture $($env:PROCESSOR_ARCHITECTURE)" }
}
$target = $env:TARGET
$arch = if ($target) { $target.Split("-")[0] } else { $machine }
if ($arch -notin "x86_64", "aarch64", "i686") { throw "unsupported architecture $arch" }
$full = $env:FULL -eq "1"
$name = if ($full) { "Corvene-Full" } else { "Corvene" }
$out = "$root\target\windows"

if (-not $env:SKIP_BUILD -and -not $env:PACKAGE_BIN) {
    $build = @("build", "--release", "-p", "corvene")
    if ($target) { $build += @("--target", $target) }
    if ($full) { $build += @("--features", "full") }
    cargo @build
    if ($LASTEXITCODE -ne 0) { throw "the build failed" }
}
$bin = if ($env:PACKAGE_BIN) {
    $env:PACKAGE_BIN
} elseif ($target) {
    "$root\target\$target\release\corvene.exe"
} else {
    "$root\target\release\corvene.exe"
}
if (-not (Test-Path $bin)) { throw "no $bin" }

# Authenticode: `signtool sign` from the newest Windows SDK
function Sign-File([string]$file) {
    if (-not $env:SIGN_PFX) { return }
    $signtool = (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source
    if (-not $signtool) {
        $signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
            Sort-Object FullName | Select-Object -Last 1 -ExpandProperty FullName
    }
    if (-not $signtool) { throw "signtool.exe (Windows SDK) was not found" }
    $timestamp = if ($env:SIGN_TIMESTAMP_URL) { $env:SIGN_TIMESTAMP_URL } else { "http://timestamp.digicert.com" }
    $sign = @("sign", "/fd", "SHA256", "/f", $env:SIGN_PFX, "/tr", $timestamp, "/td", "SHA256", "/d", "Corvene")
    if ($env:SIGN_PFX_PASSWORD) { $sign += @("/p", $env:SIGN_PFX_PASSWORD) }
    & $signtool @sign $file
    if ($LASTEXITCODE -ne 0) { throw "signing $file failed" }
}

$stage = "$out\stage-$name-$arch"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force "$stage\bin" | Out-Null
Copy-Item $bin "$stage\corvene.exe"
Copy-Item packaging\windows\corvene.bat "$stage\bin\corvene.bat"
Sign-File "$stage\corvene.exe"

$formats = if ($env:FORMATS) { $env:FORMATS.Split(" ", [StringSplitOptions]::RemoveEmptyEntries) } else { @("setup", "msi", "zip") }

# ---- the portable zip ------------------------------------------------------
# corvene.exe and bin\corvene.bat in a Corvene folder: no installer, no
# PATH entry, no self-update (updater.rs: no unins000.exe next to it)
if ("zip" -in $formats) {
    $zip = "$out\$name-$version-windows-$arch-portable.zip"
    $portable = "$out\portable-$name-$arch"
    if (Test-Path $portable) { Remove-Item -Recurse -Force $portable }
    New-Item -ItemType Directory -Force $portable | Out-Null
    Copy-Item -Recurse $stage "$portable\Corvene"
    if (Test-Path $zip) { Remove-Item -Force $zip }
    Compress-Archive -Path "$portable\Corvene" -DestinationPath $zip -CompressionLevel Optimal
    Remove-Item -Recurse -Force $portable
    Get-Item $zip | Select-Object Name, Length
}

# ---- the MSI -----------------------------------------------------------------
if ("msi" -in $formats) {
    $wix = (Get-Command wix.exe -ErrorAction SilentlyContinue).Source
    if (-not $wix) {
        Write-Warning "wix (WiX Toolset 5) was not found: skipping the .msi"
    } else {
        $msiArch = switch ($arch) { "x86_64" { "x64" } "aarch64" { "arm64" } "i686" { "x86" } }
        # Windows Installer versions are numeric: a pre-release (0.2.0-beta.1) installs as 0.2.0
        $msiVersion = $version.Split("-")[0]
        $msi = "$out\$name-$version-$arch.msi"
        & $wix build -arch $msiArch -d "Version=$msiVersion" -d "Stage=$stage" -d ("Full=" + [int]$full) `
            -bindpath packaging\windows -o $msi packaging\windows\corvene.wxs
        if ($LASTEXITCODE -ne 0) { throw "wix build failed" }
        # the .wixpdb next to it is not wanted
        Remove-Item -Force -ErrorAction SilentlyContinue ([IO.Path]::ChangeExtension($msi, ".wixpdb"))
        Sign-File $msi
        Get-Item $msi | Select-Object Name, Length
    }
}

# ---- the Inno Setup installer --------------------------------------------------
if ("setup" -in $formats) {
    $iscc = $env:ISCC
    if (-not $iscc) {
        $found = Get-Command iscc.exe -ErrorAction SilentlyContinue
        if ($found) { $iscc = $found.Source }
    }
    if (-not $iscc) {
        $iscc = @(
            "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
            "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
            "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
        ) | Where-Object { Test-Path $_ } | Select-Object -First 1
    }
    if (-not $iscc) { throw "Inno Setup 6 (iscc.exe) was not found; set ISCC to its path" }

    & $iscc /Qp "/DAppVersion=$version" "/DArch=$arch" "/DBaseName=$name" "/DStage=$stage" "/DOut=$out" packaging\windows\corvene.iss
    if ($LASTEXITCODE -ne 0) { throw "iscc failed" }
    $setup = "$out\$name-$version-$arch-setup.exe"
    Sign-File $setup
    Get-Item $setup | Select-Object Name, Length
}
Remove-Item -Recurse -Force $stage
