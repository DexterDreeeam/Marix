$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

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

function Invoke-NativeCapture {
    param(
        [Parameter(Mandatory)][string] $Command,
        [Parameter(Mandatory)][string[]] $Arguments,
        [Parameter(Mandatory)][string] $Target
    )

    $output = & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed for target '$Target' (exit code $LASTEXITCODE): $Command $($Arguments -join ' ')"
    }
    return ($output | Out-String).Trim()
}

function Write-ResolvedConfig {
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
    $gitCryptMagic = [byte[]](0x00, 0x47, 0x49, 0x54, 0x43, 0x52, 0x59, 0x50, 0x54, 0x00)
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

        $isEncrypted = $credentialBytes.Length -ge $gitCryptMagic.Length
        if ($isEncrypted) {
            for ($index = 0; $index -lt $gitCryptMagic.Length; $index++) {
                if ($credentialBytes[$index] -ne $gitCryptMagic[$index]) {
                    $isEncrypted = $false
                    break
                }
            }
        }
        if ($isEncrypted) {
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
        throw "Resolved config still contains unresolved placeholder syntax."
    }

    [IO.File]::WriteAllText($Destination, $template, [Text.UTF8Encoding]::new($false))
}

function Copy-ServerPromptTemplates {
    param(
        [Parameter(Mandatory)][string] $Source,
        [Parameter(Mandatory)][string] $Destination
    )

    if (-not (Test-Path -LiteralPath $Source -PathType Container)) {
        throw "Server prompt template directory was not found: $Source"
    }

    $sourceFiles = @(Get-ChildItem -LiteralPath $Source -File -Recurse -Force)
    if ($sourceFiles.Count -eq 0) {
        throw "Server prompt template directory contains no files: $Source"
    }

    foreach ($sourceFile in $sourceFiles) {
        $relativePath = $sourceFile.FullName.Substring($Source.Length + 1)
        $destinationFile = Join-Path $Destination $relativePath
        New-Item -ItemType Directory -Path (Split-Path -Parent $destinationFile) -Force | Out-Null
        Copy-Item -LiteralPath $sourceFile.FullName -Destination $destinationFile
    }

    $destinationFileCount = @(Get-ChildItem -LiteralPath $Destination -File -Recurse -Force).Count
    if ($destinationFileCount -ne $sourceFiles.Count) {
        throw "Server prompt copy count mismatch: expected $($sourceFiles.Count), found $destinationFileCount."
    }
}

function Enable-Zig {
    $zigCommand = Get-Command zig -CommandType Application -ErrorAction SilentlyContinue
    if ($null -ne $zigCommand) {
        if (-not (Test-Path -LiteralPath $zigCommand.Source -PathType Leaf)) {
            throw "Zig command does not resolve to a file: $($zigCommand.Source)"
        }
        return
    }

    $pythonCommand = Get-Command py -CommandType Application -ErrorAction SilentlyContinue
    if ($null -eq $pythonCommand) {
        $pythonCommand = Get-Command python -CommandType Application -ErrorAction SilentlyContinue
    }
    if ($null -eq $pythonCommand) {
        throw "Zig is not on PATH and neither 'py' nor 'python' is available to locate Python ziglang."
    }

    $pythonCode = "import pathlib, ziglang; print(pathlib.Path(ziglang.__file__).resolve().parent / 'zig.exe')"
    $zigPathText = Invoke-NativeCapture -Command $pythonCommand.Source -Arguments @('-c', $pythonCode) -Target 'Zig discovery'
    if ([string]::IsNullOrWhiteSpace($zigPathText)) {
        throw "Python ziglang returned an empty Zig path."
    }

    $zigPath = [IO.Path]::GetFullPath($zigPathText)
    if (-not (Test-Path -LiteralPath $zigPath -PathType Leaf)) {
        throw "Python ziglang Zig executable was not found: $zigPath"
    }

    $env:PATH = "$(Split-Path -Parent $zigPath)$([IO.Path]::PathSeparator)$env:PATH"
    if ($null -eq (Get-Command zig -CommandType Application -ErrorAction SilentlyContinue)) {
        throw "Zig was found at '$zigPath' but could not be added to this process PATH."
    }
}

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\..'))
$workspaceRoot = Join-Path $repoRoot 'src'
$outputRoot = Join-Path $repoRoot '.temp\project-build'
$telemetryOutput = Join-Path $outputRoot 'server-telemetry'
$serverOutput = Join-Path $outputRoot 'server'
$clientOutput = Join-Path $outputRoot 'client'
$clientAppOutput = Join-Path $clientOutput 'App'
$clientCliOutput = Join-Path $clientOutput 'Cli'
$hostOutput = Join-Path $outputRoot 'host'
$hostToolOutput = Join-Path $hostOutput 'tool'
$serverPromptSource = Join-Path $repoRoot 'src\server\prompt\template'
$serverPromptOutput = Join-Path $serverOutput 'src\server\prompt\template'
$configTemplate = Join-Path $repoRoot 'config.toml'
$credentialDirectory = Join-Path $repoRoot '.credential'

if (-not (Test-Path -LiteralPath $workspaceRoot -PathType Container)) {
    throw "Cargo workspace was not found: $workspaceRoot"
}
if (Test-Path -LiteralPath $outputRoot) {
    Remove-Item -LiteralPath $outputRoot -Recurse -Force
}
@(
    $telemetryOutput,
    $serverOutput,
    $serverPromptOutput,
    $clientAppOutput,
    $clientCliOutput,
    $hostToolOutput
) |
    ForEach-Object { New-Item -ItemType Directory -Path $_ -Force | Out-Null }

Write-ResolvedConfig -TemplatePath $configTemplate -CredentialDirectory $credentialDirectory -Destination (Join-Path $telemetryOutput 'config.toml')
Write-ResolvedConfig -TemplatePath $configTemplate -CredentialDirectory $credentialDirectory -Destination (Join-Path $serverOutput 'config.toml')
Write-ResolvedConfig -TemplatePath $configTemplate -CredentialDirectory $credentialDirectory -Destination (Join-Path $clientAppOutput 'config.toml')
Write-ResolvedConfig -TemplatePath $configTemplate -CredentialDirectory $credentialDirectory -Destination (Join-Path $clientCliOutput 'config.toml')
Write-ResolvedConfig -TemplatePath $configTemplate -CredentialDirectory $credentialDirectory -Destination (Join-Path $hostOutput 'config.toml')
Copy-ServerPromptTemplates -Source $serverPromptSource -Destination $serverPromptOutput

$cargoCommand = Get-Command cargo -CommandType Application -ErrorAction SilentlyContinue
if ($null -eq $cargoCommand) {
    throw "Required command 'cargo' was not found."
}
if ($null -eq (Get-Command cargo-zigbuild -CommandType Application -ErrorAction SilentlyContinue)) {
    throw "Required command 'cargo-zigbuild' was not found."
}
$rustupCommand = Get-Command rustup -CommandType Application -ErrorAction SilentlyContinue
if ($null -eq $rustupCommand) {
    throw "Required command 'rustup' was not found; installed Rust targets cannot be checked."
}

$installedTargetsText = Invoke-NativeCapture -Command $rustupCommand.Source -Arguments @('target', 'list', '--installed') -Target 'Rust target check'
$installedTargets = @($installedTargetsText -split '\r?\n')
foreach ($requiredTarget in @('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu')) {
    if ($installedTargets -notcontains $requiredTarget) {
        throw "Required Rust target '$requiredTarget' is not installed."
    }
}
Enable-Zig

Push-Location $workspaceRoot
try {
    Invoke-Native -Command $cargoCommand.Source -Arguments @('fetch', '--locked') -Target 'workspace dependencies'

    $metadataJson = Invoke-NativeCapture -Command $cargoCommand.Source -Arguments @('metadata', '--no-deps', '--format-version', '1') -Target 'Cargo metadata'
    try {
        $metadata = $metadataJson | ConvertFrom-Json -ErrorAction Stop
    }
    catch {
        throw "Cargo metadata returned invalid JSON: $_"
    }

    $targetDirectory = [string]$metadata.target_directory
    if ([string]::IsNullOrWhiteSpace($targetDirectory)) {
        throw "Cargo metadata did not provide target_directory."
    }
    $linuxRelease = Join-Path $targetDirectory 'x86_64-unknown-linux-gnu\release'
    $windowsRelease = Join-Path $targetDirectory 'release'

    Invoke-Native -Command $cargoCommand.Source -Arguments @('zigbuild', '--release', '--locked', '--target', 'x86_64-unknown-linux-gnu', '-p', 'marix-server-telemetry') -Target 'marix-server-telemetry (x86_64-unknown-linux-gnu)'
    Invoke-Native -Command $cargoCommand.Source -Arguments @('zigbuild', '--release', '--locked', '--target', 'x86_64-unknown-linux-gnu', '-p', 'marix-server') -Target 'marix-server (x86_64-unknown-linux-gnu)'
    Invoke-Native -Command $cargoCommand.Source -Arguments @('build', '--release', '--locked', '-p', 'marix-client') -Target 'marix-client (x86_64-pc-windows-msvc)'
    Invoke-Native -Command $cargoCommand.Source -Arguments @('build', '--release', '--locked', '-p', 'marix-host') -Target 'marix-host (x86_64-pc-windows-msvc)'

    $artifacts = @(
        [pscustomobject]@{ Source = Join-Path $linuxRelease 'marix-server-telemetry'; Destination = Join-Path $telemetryOutput 'marix-server-telemetry'; Target = 'marix-server-telemetry' }
        [pscustomobject]@{ Source = Join-Path $linuxRelease 'marix-server'; Destination = Join-Path $serverOutput 'marix-server'; Target = 'marix-server' }
        [pscustomobject]@{ Source = Join-Path $windowsRelease 'marix-client-cli.exe'; Destination = Join-Path $clientCliOutput 'marix-client-cli.exe'; Target = 'marix-client-cli' }
        [pscustomobject]@{ Source = Join-Path $windowsRelease 'marix-client-app.exe'; Destination = Join-Path $clientAppOutput 'marix-client-app.exe'; Target = 'marix-client-app' }
        [pscustomobject]@{ Source = Join-Path $windowsRelease 'marix-host.exe'; Destination = Join-Path $hostOutput 'marix-host.exe'; Target = 'marix-host' }
    )
    foreach ($artifact in $artifacts) {
        if (-not (Test-Path -LiteralPath $artifact.Source -PathType Leaf)) {
            throw "Expected artifact for target '$($artifact.Target)' was not found: $($artifact.Source)"
        }
        Copy-Item -LiteralPath $artifact.Source -Destination $artifact.Destination
    }

    $toolPackages = @($metadata.packages | Where-Object { $_.name -eq 'marix-tool' })
    if ($toolPackages.Count -ne 1) {
        throw "Expected exactly one 'marix-tool' package; found $($toolPackages.Count)."
    }
    $tools = @($toolPackages[0].targets |
        Where-Object { $_.kind -contains 'bin' } |
        ForEach-Object {
            $features = @($_.'required-features')
            if ($features.Count -ne 1) {
                throw "Tool target '$($_.name)' must declare exactly one required feature; found $($features.Count)."
            }
            [pscustomobject]@{ Target = [string]$_.name; Feature = [string]$features[0] }
        } |
        Sort-Object Target)
    if ($tools.Count -eq 0) {
        throw "'marix-tool' declares no binary targets."
    }

    foreach ($tool in $tools) {
        Invoke-Native -Command $cargoCommand.Source -Arguments @('build', '--release', '--locked', '-p', 'marix-tool', '--bin', $tool.Target, '--features', $tool.Feature) -Target "marix-tool/$($tool.Target)"

        $toolExe = Join-Path $windowsRelease "$($tool.Target).exe"
        if (-not (Test-Path -LiteralPath $toolExe -PathType Leaf)) {
            throw "Expected executable for tool target '$($tool.Target)' was not found: $toolExe"
        }
        $previewText = Invoke-NativeCapture -Command $toolExe -Arguments @('--preview') -Target "marix-tool/$($tool.Target) preview"
        try {
            $preview = $previewText | ConvertFrom-Json -ErrorAction Stop
        }
        catch {
            throw "Tool target '$($tool.Target)' returned invalid preview JSON: $_"
        }
        if ($null -eq $preview.PSObject.Properties['name'] -or
            $preview.name -isnot [string] -or
            $preview.name -cne $tool.Feature) {
            throw "Tool target '$($tool.Target)' preview.name '$($preview.name)' does not exactly match required feature '$($tool.Feature)'."
        }
        Copy-Item -LiteralPath $toolExe -Destination (Join-Path $hostToolOutput "$($tool.Target).exe")
    }

    $copiedToolCount = @(Get-ChildItem -LiteralPath $hostToolOutput -File -Force).Count
    if ($copiedToolCount -ne $tools.Count) {
        throw "Tool copy count mismatch in '$hostToolOutput': expected $($tools.Count), found $copiedToolCount."
    }
}
finally {
    Pop-Location
}

Write-Host ''
Write-Host 'Build complete:'
Write-Host "  Server Telemetry bundle: $telemetryOutput (executable and config)"
Write-Host "  Server bundle:           $serverOutput (executable, config, and dynamic prompts)"
Write-Host "  Client bundle:           $clientOutput"
Write-Host "    App:                   $clientAppOutput (executable and config)"
Write-Host "    Cli:                   $clientCliOutput (executable and config)"
Write-Host "  Host bundle:             $hostOutput (executable, config, and $($tools.Count) dynamic tools)"
