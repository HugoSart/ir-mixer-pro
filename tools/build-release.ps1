param(
    [switch]$SkipChecks
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    if (-not $SkipChecks) {
        cargo fmt --all -- --check
        if ($LASTEXITCODE -ne 0) { throw "Formatting check failed." }
        cargo clippy --workspace --all-targets -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw "Clippy failed." }
        cargo test --workspace
        if ($LASTEXITCODE -ne 0) { throw "Tests failed." }
    }

    cargo build --release --locked -p ir-mixer-pro
    if ($LASTEXITCODE -ne 0) { throw "Release build failed." }

    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw "Could not read Cargo package metadata." }
    $package = $metadata.packages | Where-Object { $_.name -eq "ir-mixer-pro" } | Select-Object -First 1
    if (-not $package) { throw "Could not find the ir-mixer-pro package." }

    $releaseName = "IR-Mixer-Pro-$($package.version)-windows-x64"
    $distRoot = Join-Path $repoRoot "dist"
    $staging = Join-Path $distRoot $releaseName
    $archive = Join-Path $distRoot "$releaseName.zip"
    $checksum = "$archive.sha256"

    New-Item -ItemType Directory -Force -Path $distRoot | Out-Null
    if (Test-Path -LiteralPath $staging) { Remove-Item -LiteralPath $staging -Recurse -Force }
    if (Test-Path -LiteralPath $archive) { Remove-Item -LiteralPath $archive -Force }
    if (Test-Path -LiteralPath $checksum) { Remove-Item -LiteralPath $checksum -Force }
    New-Item -ItemType Directory -Path $staging | Out-Null

    Copy-Item -LiteralPath (Join-Path $repoRoot "target\release\ir-mixer-pro.exe") -Destination (Join-Path $staging "IR Mixer Pro.exe")
    Copy-Item -LiteralPath (Join-Path $repoRoot "README.md") -Destination $staging
    $license = Join-Path $repoRoot "LICENSE"
    if (-not (Test-Path -LiteralPath $license)) { throw "LICENSE is required for release packaging." }
    Copy-Item -LiteralPath $license -Destination $staging

    Compress-Archive -LiteralPath $staging -DestinationPath $archive -CompressionLevel Optimal
    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    Set-Content -LiteralPath $checksum -Value "$hash  $releaseName.zip" -Encoding ascii

    Write-Host "Created $archive"
    Write-Host "Created $checksum"
}
finally {
    Pop-Location
}
