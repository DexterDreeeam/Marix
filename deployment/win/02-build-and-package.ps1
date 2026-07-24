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

    # Cargo re-parses the whole workspace manifest on every invocation and reprints the
    # same harmless warnings each time; streaming that straight to the console produces
    # hundreds of near-duplicate lines across the ~20 Invoke-Native calls in this script.
    # Instead: print exactly one line per target, capture all stdout+stderr together,
    # and only ever dump the captured output when the command actually failed.
    Write-Host -NoNewline "Building $Target... "
    $stopwatch = [Diagnostics.Stopwatch]::StartNew()

    # Merging a native command's stderr into the success stream via '2>&1' wraps each
    # stderr line as an ErrorRecord. With this script's '$ErrorActionPreference = Stop'
    # in effect, PowerShell promotes the FIRST such ErrorRecord into a terminating
    # exception the instant it appears -- regardless of the command's actual exit code.
    # This applies on BOTH Windows PowerShell 5.1 and PowerShell 7 (it predates and is
    # unrelated to PS7.3+'s separate $PSNativeCommandUseErrorActionPreference feature,
    # which only governs promoting a non-zero EXIT CODE, not merged stderr content, to
    # a terminating error). Cargo always writes its harmless workspace-manifest warning
    # to stderr on every invocation, so without this guard every single build call
    # would immediately "fail" on that warning text alone, even on real success.
    # Temporarily relax $ErrorActionPreference to SilentlyContinue for just this one
    # call so merged stderr is captured as plain data instead of promoted to an
    # exception; restore it immediately afterward regardless of outcome.
    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = 'SilentlyContinue'
    try {
        $capturedOutput = & $Command @Arguments 2>&1 | Out-String
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }

    $stopwatch.Stop()
    $elapsedSeconds = '{0:N1}' -f $stopwatch.Elapsed.TotalSeconds

    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAILED (${elapsedSeconds}s)" -ForegroundColor Red
        Write-Host $capturedOutput
        throw "Command failed for target '$Target' (exit code $LASTEXITCODE): $Command $($Arguments -join ' ')"
    }

    $changeLabel = if ($capturedOutput -clike '*Compiling*') { '[compiled]' } else { '[no change]' }
    Write-Host "OK (${elapsedSeconds}s) $changeLabel"
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

$workspaceRoot = Join-Path $RepoRoot 'src'
$outputRoot = Join-Path $RepoRoot '.temp\package'
$telemetryOutput = Join-Path $outputRoot 'server_telemetry'
$serverOutput = Join-Path $outputRoot 'server'
$clientOutput = Join-Path $outputRoot 'client'
$clientAppOutput = Join-Path $clientOutput 'App'
$clientCliOutput = Join-Path $clientOutput 'Cli'
$hostOutput = Join-Path $outputRoot 'host'
$hostToolOutput = Join-Path $hostOutput 'tool'
$serverPromptSource = Join-Path $RepoRoot 'src\server\prompt\template'
$serverPromptOutput = Join-Path $serverOutput 'prompt'

if (-not (Test-Path -LiteralPath $workspaceRoot -PathType Container)) {
    throw "Cargo workspace was not found: $workspaceRoot"
}

# Recreate only the packaging output from scratch; Cargo's own build cache (the
# 'target' directories) is left untouched so incremental compilation still applies.
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
        # Each Tool is built in its own isolated invocation. Building more than one
        # Tool feature per invocation silently collapses all of them into whichever
        # module is declared first (a known Cargo feature-unification hazard across
        # the shared-source [[bin]] targets in marix-tool) -- never combine these.
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

    Write-Host ''
    Write-Host 'Build and package complete (config.toml is resolved separately in step 3):'
    Write-Host "  Server Telemetry bundle: $telemetryOutput (executable)"
    Write-Host "  Server bundle:           $serverOutput (executable and dynamic prompts)"
    Write-Host "  Client bundle:           $clientOutput"
    Write-Host "    App:                   $clientAppOutput (executable)"
    Write-Host "    Cli:                   $clientCliOutput (executable)"
    Write-Host "  Host bundle:             $hostOutput (executable and $($tools.Count) dynamic tools)"
}
finally {
    Pop-Location
}
