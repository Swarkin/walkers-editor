try {
    # Validate environment variable exists
    if (-not $env:TRUNK_STAGING_DIR) {
        throw "TRUNK_STAGING_DIR environment variable is not set"
    }

    # Normalize and validate staging directory
    $stagingDir = $env:TRUNK_STAGING_DIR -replace '\\\\\?\\',''
    if (-not (Test-Path $stagingDir -PathType Container)) {
        throw "Staging directory does not exist: $stagingDir"
    }

    $swPath = Join-Path $stagingDir "sw.js"
    $tempPath = Join-Path $stagingDir ".sw.js"

    # Validate source file exists
    if (-not (Test-Path $swPath -PathType Leaf)) {
        throw "Source file does not exist: $swPath"
    }

    # Create temporary copy with error handling
    try {
        Copy-Item $swPath $tempPath -ErrorAction Stop
    } catch {
        throw "Failed to create temporary file: $_"
    }

    # Generate timestamp and write new content
    try {
        $timestamp = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
        "const BUILD_TIME = $timestamp;" | Out-File $swPath -Encoding UTF8 -ErrorAction Stop
        Get-Content $tempPath -ErrorAction Stop | Add-Content $swPath -Encoding UTF8 -ErrorAction Stop
    }
    catch {
        # Restore original file if write failed
        if (Test-Path $tempPath) {
            Move-Item $tempPath $swPath -Force
        }
        throw "Failed to update service worker file: $_"
    }

    # Clean up temporary file with error handling
    if (Test-Path $tempPath) {
        Remove-Item $tempPath -ErrorAction SilentlyContinue
    }
} catch {
    Write-Error "Script failed: $($_.Exception.Message)"
    exit 1
}
