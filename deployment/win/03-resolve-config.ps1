param(
    [Parameter(Mandatory)][string] $RepoRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

# Never rely on the current working directory: every path below is built from the
# $RepoRoot value passed explicitly by run.ps1, never re-derived from $PSScriptRoot.

function Invoke-Native {
    param(
        [Parameter(Mandatory)][string] $Command,
        [Parameter(Mandatory)][string[]] $Arguments,
        [Parameter(Mandatory)][string] $Target
    )

    Write-Host "> $Command $($Arguments -join ' ')"
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed for target '$Target' (exit code $LASTEXITCODE): $Command $($Arguments -join ' ')"
    }
}

# git-crypt magic header (\0GITCRYPT\0): a file starting with these bytes is still
# git-crypt ciphertext, not a usable plaintext credential.
$gitCryptMagic = [byte[]](0x00, 0x47, 0x49, 0x54, 0x43, 0x52, 0x59, 0x50, 0x54, 0x00)

function Test-CredentialFileLocked {
    param(
        [Parameter(Mandatory)][string] $CredentialPath
    )

    $bytes = [IO.File]::ReadAllBytes($CredentialPath)
    if ($bytes.Length -lt $gitCryptMagic.Length) {
        return $false
    }
    for ($index = 0; $index -lt $gitCryptMagic.Length; $index++) {
        if ($bytes[$index] -ne $gitCryptMagic[$index]) {
            return $false
        }
    }
    return $true
}

function Resolve-CredentialsIfLocked {
    <#
    One-time pre-pass (runs once, not once per destination): discover every
    required credential file, check each for the git-crypt magic header, and if
    ANY are still locked, attempt 'git-crypt unlock' once before falling back to
    a clear failure. If none are locked, the repo is already decrypted and this
    is a no-op.
    #>
    param(
        [Parameter(Mandatory)][AllowEmptyCollection()][string[]] $PlaceholderNames,
        [Parameter(Mandatory)][string] $CredentialDirectory,
        [Parameter(Mandatory)][string] $RepoRoot
    )

    $credentialPaths = @()
    foreach ($name in $PlaceholderNames) {
        $credentialPath = Join-Path $CredentialDirectory "$name.txt"
        if (-not (Test-Path -LiteralPath $credentialPath -PathType Leaf)) {
            throw "Credential file for placeholder '$name' was not found: $credentialPath"
        }
        $credentialPaths += $credentialPath
    }

    $lockedPaths = @($credentialPaths | Where-Object { Test-CredentialFileLocked -CredentialPath $_ })
    if ($lockedPaths.Count -eq 0) {
        Write-Host 'Required credential files are already decrypted; no git-crypt unlock needed.'
        return
    }

    Write-Host "Detected $($lockedPaths.Count) git-crypt locked credential file(s); attempting 'git-crypt unlock'..."

    $keyPath = [IO.Path]::GetFullPath((Join-Path $RepoRoot '..\marix-git-crypt.key'))
    if (-not (Test-Path -LiteralPath $keyPath -PathType Leaf)) {
        throw "Required credentials are git-crypt locked and the expected key file was not found: $keyPath"
    }

    $gitCryptCommand = Get-Command git-crypt -CommandType Application -ErrorAction SilentlyContinue
    if ($null -eq $gitCryptCommand) {
        throw "Required credentials are git-crypt locked but the 'git-crypt' command is not available on PATH."
    }

    Push-Location $RepoRoot
    try {
        Invoke-Native -Command $gitCryptCommand.Source -Arguments @('unlock', $keyPath) -Target 'git-crypt unlock'
    }
    finally {
        Pop-Location
    }

    $stillLockedPaths = @($lockedPaths | Where-Object { Test-CredentialFileLocked -CredentialPath $_ })
    if ($stillLockedPaths.Count -gt 0) {
        throw "'git-crypt unlock' completed but $($stillLockedPaths.Count) required credential file(s) are still encrypted; unlock did not succeed."
    }

    Write-Host "'git-crypt unlock' succeeded; required credential files are now decrypted."
}

function Write-ResolvedConfig {
    <#
    Independently resolves the template for ONE destination: re-reads the template
    and every credential file fresh from disk, so calling this multiple times for
    different destinations never reuses another destination's resolved text.
    #>
    param(
        [Parameter(Mandatory)][string] $TemplatePath,
        [Parameter(Mandatory)][string] $CredentialDirectory,
        [Parameter(Mandatory)][string] $Destination
    )

    if (-not (Test-Path -LiteralPath $TemplatePath -PathType Leaf)) {
        throw "Config template was not found: $TemplatePath"
    }
    if (-not (Test-Path -LiteralPath $CredentialDirectory -PathType Container)) {
        throw "Credential directory was not found: $CredentialDirectory"
    }

    $template = [IO.File]::ReadAllText($TemplatePath, [Text.Encoding]::UTF8)
    $placeholderMatches = [regex]::Matches($template, '\{\{([^{}\r\n]+)\}\}')
    $placeholderNames = @($placeholderMatches |
        ForEach-Object { $_.Groups[1].Value } |
        Sort-Object -Unique)
    $strictUtf8 = [Text.UTF8Encoding]::new($false, $true)

    foreach ($name in $placeholderNames) {
        $credentialPath = Join-Path $CredentialDirectory "$name.txt"
        if (-not (Test-Path -LiteralPath $credentialPath -PathType Leaf)) {
            throw "Credential file for placeholder '$name' was not found: $credentialPath"
        }

        $credentialBytes = [IO.File]::ReadAllBytes($credentialPath)
        if ($credentialBytes.Length -eq 0) {
            throw "Credential file for placeholder '$name' is empty: $credentialPath"
        }

        # Safety net: by this point Resolve-CredentialsIfLocked has already ensured
        # every required credential is decrypted, but re-check per file in case this
        # function is ever invoked on its own without that pre-pass having run.
        if (Test-CredentialFileLocked -CredentialPath $credentialPath) {
            throw "Credential file for placeholder '$name' is still encrypted by git-crypt: $credentialPath"
        }

        try {
            $credentialValue = $strictUtf8.GetString($credentialBytes).Trim()
        }
        catch {
            throw "Credential file for placeholder '$name' is not valid UTF-8: $credentialPath"
        }
        if ([string]::IsNullOrWhiteSpace($credentialValue)) {
            throw "Credential file for placeholder '$name' is empty after trimming whitespace: $credentialPath"
        }

        $template = $template.Replace("{{$name}}", $credentialValue)
    }

    if ([regex]::IsMatch($template, '\{\{.*?\}\}', [Text.RegularExpressions.RegexOptions]::Singleline)) {
        throw "Resolved config still contains unresolved placeholder syntax: $Destination"
    }

    [IO.File]::WriteAllText($Destination, $template, [Text.UTF8Encoding]::new($false))
}

$configTemplate = Join-Path $RepoRoot 'config.toml'
$credentialDirectory = Join-Path $RepoRoot '.credential'
$packageRoot = Join-Path $RepoRoot '.temp\package'

if (-not (Test-Path -LiteralPath $configTemplate -PathType Leaf)) {
    throw "Config template was not found: $configTemplate"
}
if (-not (Test-Path -LiteralPath $credentialDirectory -PathType Container)) {
    throw "Credential directory was not found: $credentialDirectory"
}

$templateTextForDiscovery = [IO.File]::ReadAllText($configTemplate, [Text.Encoding]::UTF8)
$placeholderNames = @([regex]::Matches($templateTextForDiscovery, '\{\{([^{}\r\n]+)\}\}') |
    ForEach-Object { $_.Groups[1].Value } |
    Sort-Object -Unique)
if ($placeholderNames.Count -eq 0) {
    Write-Host 'Warning: config.toml template contains no {{PLACEHOLDER}} tokens; nothing to resolve from credentials.'
}

Resolve-CredentialsIfLocked -PlaceholderNames $placeholderNames -CredentialDirectory $credentialDirectory -RepoRoot $RepoRoot

$destinations = @(
    (Join-Path $packageRoot 'server\config.toml'),
    (Join-Path $packageRoot 'server_telemetry\config.toml'),
    (Join-Path $packageRoot 'client\App\config.toml'),
    (Join-Path $packageRoot 'client\Cli\config.toml'),
    (Join-Path $packageRoot 'host\config.toml')
)

foreach ($destination in $destinations) {
    $destinationDirectory = Split-Path -Parent $destination
    if (-not (Test-Path -LiteralPath $destinationDirectory -PathType Container)) {
        throw "Expected package output directory was not found (run the build-and-package step first): $destinationDirectory"
    }
    # Resolved independently per destination from the root template and credentials,
    # even though the resulting values are identical across all five destinations.
    Write-ResolvedConfig -TemplatePath $configTemplate -CredentialDirectory $credentialDirectory -Destination $destination
}

Write-Host ''
Write-Host 'Config resolution complete (each destination resolved independently):'
foreach ($destination in $destinations) {
    Write-Host "  $destination"
}
