# Shared helper functions for deployment steps 5-13 (VM Host lifecycle, and Ubuntu
# Server / Server Telemetry lifecycle over SSH). This file is dot-sourced by each of
# those step scripts; it is not itself a numbered step and is not directly invoked by
# run.ps1. It intentionally duplicates (rather than imports/dot-sources) small pieces
# of logic already established in 03-resolve-config.ps1 (git-crypt magic-header check,
# credential resolution pattern) because 03 is a full step script with its own
# top-level side effects, not an importable module, and because these new scripts must
# not assume step 3 already ran in the same process (each step invocation is
# independent) and must also cover deploy-only secrets (e.g. SERVER_ROOT_SSH_KEY) that
# 03 never touches since they are not a {{PLACEHOLDER}} referenced anywhere in
# config.toml.

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

# Same git-crypt magic header bytes as 03-resolve-config.ps1.
$Script:DeployGitCryptMagic = [byte[]](0x00, 0x47, 0x49, 0x54, 0x43, 0x52, 0x59, 0x50, 0x54, 0x00)

function Test-DeployCredentialFileLocked {
    param(
        [Parameter(Mandatory)][string] $CredentialPath
    )
    $bytes = [IO.File]::ReadAllBytes($CredentialPath)
    if ($bytes.Length -lt $Script:DeployGitCryptMagic.Length) {
        return $false
    }
    for ($i = 0; $i -lt $Script:DeployGitCryptMagic.Length; $i++) {
        if ($bytes[$i] -ne $Script:DeployGitCryptMagic[$i]) {
            return $false
        }
    }
    return $true
}

function Unlock-DeployCredentialIfLocked {
    # Independently, defensively re-checks and (if needed) unlocks ONE credential
    # file's git-crypt state. Mirrors 03-resolve-config.ps1's
    # Resolve-CredentialsIfLocked in spirit, but invoked per-credential (not as a
    # single upfront batch pre-pass) since these scripts must work standalone and
    # must also cover deploy-only secrets 03 never touches (e.g.
    # SERVER_ROOT_SSH_KEY.txt, which is not a {{PLACEHOLDER}} in config.toml).
    param(
        [Parameter(Mandatory)][string] $CredentialPath,
        [Parameter(Mandatory)][string] $Name,
        [Parameter(Mandatory)][string] $RepoRoot
    )
    if (-not (Test-DeployCredentialFileLocked -CredentialPath $CredentialPath)) {
        return
    }

    Write-Host "Credential '$Name' is git-crypt locked; attempting 'git-crypt unlock'..."
    $keyPath = [IO.Path]::GetFullPath((Join-Path $RepoRoot '..\marix-git-crypt.key'))
    if (-not (Test-Path -LiteralPath $keyPath -PathType Leaf)) {
        throw "Credential '$Name' is git-crypt locked and the expected key file was not found: $keyPath"
    }
    $gitCryptCommand = Get-Command -Name 'git-crypt' -CommandType Application -ErrorAction SilentlyContinue
    if ($null -eq $gitCryptCommand) {
        throw "Credential '$Name' is git-crypt locked but the 'git-crypt' command is not available on PATH."
    }

    Push-Location -LiteralPath $RepoRoot
    try {
        & $gitCryptCommand.Source unlock $keyPath 2>$null
        $exitCode = $LASTEXITCODE
        if ($exitCode -ne 0) {
            throw "'git-crypt unlock' failed (exit code $exitCode) while resolving credential '$Name'."
        }
    }
    finally {
        Pop-Location
    }

    if (Test-DeployCredentialFileLocked -CredentialPath $CredentialPath) {
        throw "'git-crypt unlock' completed but credential '$Name' is still encrypted; unlock did not succeed."
    }
    Write-Host "'git-crypt unlock' succeeded; credential '$Name' is now decrypted."
}

function Get-DeployCredentialBytes {
    param(
        [Parameter(Mandatory)][string] $RepoRoot,
        [Parameter(Mandatory)][string] $Name
    )
    $credentialPath = Join-Path (Join-Path $RepoRoot '.credential') "$Name.txt"
    if (-not (Test-Path -LiteralPath $credentialPath -PathType Leaf)) {
        throw "Credential file for '$Name' was not found: $credentialPath"
    }
    Unlock-DeployCredentialIfLocked -CredentialPath $credentialPath -Name $Name -RepoRoot $RepoRoot
    $bytes = [IO.File]::ReadAllBytes($credentialPath)
    if ($bytes.Length -eq 0) {
        throw "Credential file for '$Name' is empty: $credentialPath"
    }
    return $bytes
}

function Get-DeployCredentialText {
    param(
        [Parameter(Mandatory)][string] $RepoRoot,
        [Parameter(Mandatory)][string] $Name
    )
    $bytes = Get-DeployCredentialBytes -RepoRoot $RepoRoot -Name $Name
    $strictUtf8 = [Text.UTF8Encoding]::new($false, $true)
    try {
        $text = $strictUtf8.GetString($bytes)
    }
    catch {
        throw "Credential file for '$Name' is not valid UTF-8 text."
    }
    $trimmed = $text.Trim()
    if ([string]::IsNullOrWhiteSpace($trimmed)) {
        throw "Credential file for '$Name' is empty after trimming whitespace."
    }
    return $trimmed
}

# ---------------------------------------------------------------------------
# SSH/SCP context (steps 7, 8, 9, 10, 11, 12 - all Ubuntu-facing steps)
# ---------------------------------------------------------------------------

function New-DeploymentSshContext {
    param(
        [Parameter(Mandatory)][string] $RepoRoot
    )
    $sshExe = Join-Path $env:WINDIR 'System32\OpenSSH\ssh.exe'
    $scpExe = Join-Path $env:WINDIR 'System32\OpenSSH\scp.exe'
    if (-not (Test-Path -LiteralPath $sshExe -PathType Leaf)) {
        throw "Windows OpenSSH client was not found: $sshExe"
    }
    if (-not (Test-Path -LiteralPath $scpExe -PathType Leaf)) {
        throw "Windows OpenSSH scp client was not found: $scpExe"
    }

    $hostIp = Get-DeployCredentialText -RepoRoot $RepoRoot -Name 'SERVER_IP'
    $keyBytes = Get-DeployCredentialBytes -RepoRoot $RepoRoot -Name 'SERVER_ROOT_SSH_KEY'

    $sshTempRoot = Join-Path $RepoRoot '.temp\deployment-ssh'
    New-Item -ItemType Directory -Path $sshTempRoot -Force | Out-Null
    $knownHostsPath = Join-Path $sshTempRoot 'known_hosts'
    if (-not (Test-Path -LiteralPath $knownHostsPath -PathType Leaf)) {
        New-Item -ItemType File -Path $knownHostsPath -Force | Out-Null
    }

    $keyPath = Join-Path $sshTempRoot "id_deploy_$([guid]::NewGuid().ToString('N')).key"
    [IO.File]::WriteAllBytes($keyPath, $keyBytes)
    try {
        icacls $keyPath /inheritance:r | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "icacls failed to strip inherited ACLs on the temporary SSH key (exit code $LASTEXITCODE)."
        }
        icacls $keyPath /grant:r "$($env:USERNAME):(F)" | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "icacls failed to grant the current user access to the temporary SSH key (exit code $LASTEXITCODE)."
        }
    }
    catch {
        Remove-Item -LiteralPath $keyPath -Force -ErrorAction SilentlyContinue
        throw
    }

    $baseArgs = @(
        '-o', 'StrictHostKeyChecking=accept-new',
        '-o', "UserKnownHostsFile=$knownHostsPath",
        '-o', 'BatchMode=yes',
        '-o', 'ConnectTimeout=10',
        '-i', $keyPath
    )

    return @{
        SshExe   = $sshExe
        ScpExe   = $scpExe
        BaseArgs = $baseArgs
        HostIp   = $hostIp
        KeyPath  = $keyPath
        TempDir  = $sshTempRoot
    }
}

function Remove-DeploymentSshContext {
    param(
        [Parameter(Mandatory)][hashtable] $Context
    )
    # Deliberately does NOT remove $Context.TempDir or the known_hosts file within it:
    # the known-hosts file is a fixed, persistent trust store by design (see
    # New-DeploymentSshContext above), so repeated runs verify against a
    # previously-trusted host key instead of re-trusting on every single run. Only the
    # decrypted private-key copy is deleted here, since it must not outlive the step
    # that decrypted it.
    if ($Context.KeyPath -and (Test-Path -LiteralPath $Context.KeyPath)) {
        Remove-Item -LiteralPath $Context.KeyPath -Force -ErrorAction SilentlyContinue
    }
}

function Invoke-DeploymentSsh {
    param(
        [Parameter(Mandatory)][hashtable] $Context,
        [Parameter(Mandatory)][string] $RemoteCommand
    )
    # PowerShell here-strings use CRLF line endings. When a multi-line command is
    # passed as a single argument to ssh and executed remotely as `bash -c "<text>"`,
    # any embedded CR becomes a literal character inside the script, which bash
    # chokes on (e.g. "do\r" is parsed as one invalid token instead of `do` followed
    # by a newline). Normalize to LF-only here - a single choke point - rather than
    # in every command-building function.
    $normalizedCommand = $RemoteCommand -replace "`r`n", "`n" -replace "`r", "`n"

    $stderrPath = Join-Path $Context.TempDir "ssh-stderr-$([guid]::NewGuid().ToString('N')).txt"
    try {
        # Deliberately redirect stderr to a FILE, not merge it via 2>&1 into the
        # success stream: merging a native command's stderr with 2>&1 wraps each
        # stderr line as a PowerShell ErrorRecord, and under
        # $ErrorActionPreference = 'Stop' the first such record becomes a
        # terminating exception immediately - regardless of the command's real exit
        # code. ssh legitimately writes to stderr on a successful run (e.g. the
        # "Warning: Permanently added ... to the list of known hosts" notice), so
        # this is not a hypothetical concern. A plain file redirect is a pure OS-level
        # redirect with no PowerShell stream/ErrorRecord involvement, so it is safe
        # here regardless.
        $stdoutLines = @(& $Context.SshExe @($Context.BaseArgs) "root@$($Context.HostIp)" $normalizedCommand 2>$stderrPath)
        $exitCode = $LASTEXITCODE
        $stderrText = ''
        if (Test-Path -LiteralPath $stderrPath) {
            $stderrText = [IO.File]::ReadAllText($stderrPath)
        }
        return [pscustomobject]@{
            ExitCode    = $exitCode
            StdOutLines = $stdoutLines
            StdErr      = $stderrText
        }
    }
    finally {
        Remove-Item -LiteralPath $stderrPath -Force -ErrorAction SilentlyContinue
    }
}

function Invoke-DeploymentScp {
    param(
        [Parameter(Mandatory)][hashtable] $Context,
        [Parameter(Mandatory)][string] $LocalPath,
        [Parameter(Mandatory)][string] $RemotePath
    )
    $stderrPath = Join-Path $Context.TempDir "scp-stderr-$([guid]::NewGuid().ToString('N')).txt"
    try {
        & $Context.ScpExe @($Context.BaseArgs) $LocalPath "root@$($Context.HostIp):$RemotePath" 2>$stderrPath | Out-Null
        $exitCode = $LASTEXITCODE
        $stderrText = ''
        if (Test-Path -LiteralPath $stderrPath) {
            $stderrText = [IO.File]::ReadAllText($stderrPath)
        }
        if ($exitCode -ne 0) {
            throw "scp upload failed (exit code $exitCode) for '$LocalPath' -> '$RemotePath': $stderrText"
        }
    }
    finally {
        Remove-Item -LiteralPath $stderrPath -Force -ErrorAction SilentlyContinue
    }
}

# ---------------------------------------------------------------------------
# VM PowerShell Direct session helper (steps 5, 6, 13)
# ---------------------------------------------------------------------------

function Connect-DeploymentVmSession {
    param(
        [Parameter(Mandatory)][string] $VmName,
        [Parameter(Mandatory)][pscredential] $Credential,
        [int] $TimeoutSeconds = 30,
        [int] $RetryDelaySeconds = 5
    )
    $stopwatch = [Diagnostics.Stopwatch]::StartNew()
    $lastError = $null
    do {
        try {
            return New-PSSession -VMName $VmName -Credential $Credential -ErrorAction Stop
        }
        catch {
            $lastError = $_
            Start-Sleep -Seconds $RetryDelaySeconds
        }
    } while ($stopwatch.Elapsed.TotalSeconds -lt $TimeoutSeconds)

    throw "Could not open a PowerShell Direct session to VM '$VmName' within $TimeoutSeconds seconds. Last error: $lastError"
}

# ---------------------------------------------------------------------------
# VM guest process query/kill by path (steps 5, 13)
#
# Get-Process's 'Path' property is an alias for '.MainModule.FileName', whose getter
# can throw "Access is denied" per-process (e.g. for other users' or protected system
# processes) - this is unrelated to whether the process we actually care about is
# accessible. Each candidate's Path access is individually try/caught so one
# inaccessible, irrelevant process never aborts the scan for the ones that matter.
# ---------------------------------------------------------------------------

function Get-VmProcessesByPath {
    <#
    Read-only query: returns simple [pscustomobject] records (Id, Path) - never live
    Process objects, which cannot usefully cross the PSSession serialization boundary
    for later actions - for every VM guest process whose Path exactly matches
    -ExactPath (case-insensitive) and/or starts with -PathPrefix (case-insensitive).
    Supply either or both; a process matching either counts as a match.
    #>
    param(
        [Parameter(Mandatory)][System.Management.Automation.Runspaces.PSSession] $Session,
        [string] $ExactPath,
        [string] $PathPrefix
    )
    # This @() wraps the Invoke-Command CALL itself (list-context capture of however
    # many objects its scriptblock emits), not a pre-assigned variable. That distinction
    # matters: when the remote scriptblock's collection has zero matches, PowerShell's
    # pipeline enumeration emits zero objects across the remoting boundary, which
    # $results = Invoke-Command ...; @($results) would then observe as a true $null
    # -- and @($null) produces a 1-ELEMENT array containing null (a well-known
    # footgun), not an empty array. Wrapping the call directly instead captures the
    # zero-object case as a genuinely empty array, with no separate null state to
    # mis-wrap. (Get-VmManifestEntries's similar-looking pattern is safe without this
    # because its RelPaths input is never empty, so it can never hit the zero case.)
    $results = @(Invoke-Command -Session $Session -ScriptBlock {
        param($Exact, $Prefix)
        $found = New-Object System.Collections.Generic.List[object]
        foreach ($proc in (Get-Process -ErrorAction SilentlyContinue)) {
            $path = $null
            try { $path = $proc.Path } catch { $path = $null }
            if ([string]::IsNullOrEmpty($path)) { continue }
            $isMatch = $false
            if ($Exact -and ($path -ieq $Exact)) { $isMatch = $true }
            if ($Prefix -and $path.StartsWith($Prefix, [StringComparison]::OrdinalIgnoreCase)) { $isMatch = $true }
            if ($isMatch) {
                $found.Add([pscustomobject]@{ Id = [int]$proc.Id; Path = [string]$path })
            }
        }
        return $found
    } -ArgumentList $ExactPath, $PathPrefix)

    $entries = foreach ($r in $results) {
        [pscustomobject]@{ Id = [int]$r.Id; Path = [string]$r.Path }
    }
    # return's own enumeration collapses a bare 0- or 1-element array back to
    # $null / a scalar at the caller UNLESS the array survives as a single intact
    # object. This function's callers always wrap the call in @(...) (list-context
    # capture), and @(...) alone is sufficient to guarantee an array of the right
    # size for every N (0, 1, many) - CONFIRMED empirically. Do NOT also return via
    # the unary comma (`return ,$entries`) here: since the caller already wraps
    # with @(), a comma-protected return would hand back a 1-element OUTER array
    # whose sole element is this whole INNER array (double-wrapped), silently
    # breaking Count and foreach for every caller. Exactly one layer of protection
    # (the caller's @()) is correct; a second layer at the callee actively
    # miscounts/mis-enumerates instead of adding safety.
    if ($null -eq $entries) { $entries = @() } else { $entries = @($entries) }
    return $entries
}

function Stop-VmProcessesByPath {
    <#
    Finds every VM guest process matching -ExactPath and/or -PathPrefix (same matching
    rules as Get-VmProcessesByPath), force-stops each, then polls -ConfirmTimeoutSeconds
    for confirmation that none remain before returning. Throws if any survive past the
    timeout. Treats "nothing found running" as success, matching the analogous Ubuntu
    Stop-RemoteProcessByPath's "nothing running is not an error" semantics.
    #>
    param(
        [Parameter(Mandatory)][System.Management.Automation.Runspaces.PSSession] $Session,
        [string] $ExactPath,
        [string] $PathPrefix,
        [Parameter(Mandatory)][string] $Label,
        [int] $ConfirmTimeoutSeconds = 10
    )
    $result = Invoke-Command -Session $Session -ScriptBlock {
        param($Exact, $Prefix, $ConfirmSeconds)

        function Get-Matches {
            $found = New-Object System.Collections.Generic.List[object]
            foreach ($proc in (Get-Process -ErrorAction SilentlyContinue)) {
                $path = $null
                try { $path = $proc.Path } catch { $path = $null }
                if ([string]::IsNullOrEmpty($path)) { continue }
                $isMatch = $false
                if ($Exact -and ($path -ieq $Exact)) { $isMatch = $true }
                if ($Prefix -and $path.StartsWith($Prefix, [StringComparison]::OrdinalIgnoreCase)) { $isMatch = $true }
                if ($isMatch) { $found.Add($proc) }
            }
            return $found
        }

        $initial = @(Get-Matches)
        if ($initial.Count -eq 0) {
            return [pscustomobject]@{ Status = 'NOTHING_WAS_RUNNING'; KilledIds = @() }
        }

        $killedIds = @($initial | ForEach-Object { $_.Id })
        foreach ($proc in $initial) {
            Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
        }

        $deadline = (Get-Date).AddSeconds($ConfirmSeconds)
        do {
            if (@(Get-Matches).Count -eq 0) {
                return [pscustomobject]@{ Status = 'CONFIRMED_STOPPED'; KilledIds = $killedIds }
            }
            Start-Sleep -Milliseconds 250
        } while ((Get-Date) -lt $deadline)

        $stillRunning = @(Get-Matches | ForEach-Object { $_.Id })
        throw "$($stillRunning.Count) process(es) did not stop within $ConfirmSeconds second(s): $($stillRunning -join ', ')"
    } -ArgumentList $ExactPath, $PathPrefix, $ConfirmTimeoutSeconds

    Write-Host "  $Label -> $($result.Status)$(if ($result.KilledIds.Count -gt 0) { " (PIDs: $($result.KilledIds -join ', '))" })"
    return $result.Status
}

# ---------------------------------------------------------------------------
# Hash-manifest build/compare (steps 6, 8, 10)
#
# The manifest's definitive relative-path set always comes from the freshly-built
# LOCAL package (guaranteed clean, since 02-build-and-package.ps1 recreates
# .temp\package from scratch every run). The remote/VM side is only ever looked up at
# exactly those same relative paths (missing -> "MISSING" sentinel); it is never
# independently, recursively enumerated. Both C:\MarixHost\ and /opt/marix/* are known
# to carry pre-existing non-package files/directories (rollback history, logs, the
# live Telemetry redb store, etc.) that must never affect - or be affected by - this
# comparison.
# ---------------------------------------------------------------------------

function Get-LocalPackageManifestEntries {
    param(
        [Parameter(Mandatory)][string] $PackageRoot
    )
    if (-not (Test-Path -LiteralPath $PackageRoot -PathType Container)) {
        throw "Local package root was not found: $PackageRoot"
    }
    $rootFull = (Resolve-Path -LiteralPath $PackageRoot).ProviderPath.TrimEnd('\', '/')
    $files = @(Get-ChildItem -LiteralPath $rootFull -Recurse -Force -File)
    $entries = foreach ($file in $files) {
        $relPath = ($file.FullName.Substring($rootFull.Length).TrimStart('\', '/')) -replace '\\', '/'
        $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        [pscustomobject]@{ RelPath = $relPath; Hash = $hash }
    }
    # return itself enumerates any array value handed to it, collapsing a 0- or
    # 1-element result to $null / a bare scalar at the caller unless the array
    # survives as a single intact object. Every call site of this function wraps
    # the call in @(...) (list-context capture), and that alone is sufficient for
    # every N (0, 1, many) - CONFIRMED empirically. Do NOT ALSO return via the
    # unary comma (`return ,$entries`) here: with the caller already using @(),
    # a comma-protected return double-wraps the result into a 1-element OUTER
    # array whose sole element is this whole INNER array, silently breaking
    # Count/foreach at every caller (this exact combination was live-observed to
    # break step 6's manifest compare - see Test-PackageManifestsMatch's own
    # ConvertTo-SortedOrdinalArray usage below for the one place in this file
    # where the callee's comma is correct instead, because that one call site
    # deliberately never wraps with an extra @()).
    if ($null -eq $entries) { $entries = @() } else { $entries = @($entries) }
    return $entries
}

function Get-VmManifestEntries {
    param(
        [Parameter(Mandatory)][System.Management.Automation.Runspaces.PSSession] $Session,
        [Parameter(Mandatory)][string] $VmDestRoot,
        [Parameter(Mandatory)][string[]] $RelPaths
    )
    $results = Invoke-Command -Session $Session -ScriptBlock {
        param($DestRoot, $Paths)
        foreach ($rel in $Paths) {
            $winRel = $rel -replace '/', '\'
            $fullPath = Join-Path $DestRoot $winRel
            if (Test-Path -LiteralPath $fullPath -PathType Leaf) {
                $hash = (Get-FileHash -LiteralPath $fullPath -Algorithm SHA256).Hash.ToLowerInvariant()
                [pscustomobject]@{ RelPath = $rel; Hash = $hash }
            }
            else {
                [pscustomobject]@{ RelPath = $rel; Hash = 'MISSING' }
            }
        }
    } -ArgumentList $VmDestRoot, $RelPaths

    $entries = foreach ($r in @($results)) {
        [pscustomobject]@{ RelPath = [string]$r.RelPath; Hash = [string]$r.Hash }
    }
    # IMPORTANT: return <array> re-enumerates the array AGAIN at this exact
    # statement, independently of whatever @()/foreach already did to build it.
    # A 0-element $entries collapses to a bare $null at the caller, and a
    # 1-element $entries collapses to that single bare object (not a 1-element
    # array) -- in both cases losing "this is a collection" information. This
    # function's caller always wraps the call in @(...), and that alone is
    # sufficient to keep the array intact for every N - CONFIRMED empirically.
    # Do NOT ALSO protect this return with the unary comma (`return ,$entries`):
    # doing so on top of the caller's @() double-wraps the result into a
    # 1-element OUTER array whose sole element is this whole INNER array,
    # silently breaking Count/foreach for the caller instead of protecting it.
    # RelPaths is never empty here (it always comes from the local package's own
    # file list), so the 0-case can't occur in practice for this function
    # specifically, but a 1-file package is entirely plausible (server_telemetry
    # has only 2 today) and would otherwise silently hand callers a scalar
    # instead of an array with a Count/foreach-able shape.
    if ($null -eq $entries) { $entries = @() } else { $entries = @($entries) }
    return $entries
}

function Get-SshManifestEntries {
    param(
        [Parameter(Mandatory)][hashtable] $Context,
        [Parameter(Mandatory)][string] $RemoteDestRoot,
        [Parameter(Mandatory)][string[]] $RelPaths
    )
    $quotedList = ($RelPaths | ForEach-Object { "'" + ($_ -replace "'", "'\''") + "'" }) -join ' '
    $remoteCmd = @"
cd '$RemoteDestRoot' 2>/dev/null
cd_rc=`$?
files=($quotedList)
for f in "`${files[@]}"; do
  if [ `$cd_rc -ne 0 ]; then
    echo "`$f:MISSING"
    continue
  fi
  if [ -f "`$f" ]; then
    h=`$(sha256sum "`$f" | awk '{print `$1}')
    echo "`$f:`$h"
  else
    echo "`$f:MISSING"
  fi
done
"@
    $result = Invoke-DeploymentSsh -Context $Context -RemoteCommand $remoteCmd
    if ($result.ExitCode -ne 0) {
        throw "Failed to read the remote manifest under '$RemoteDestRoot' (exit code $($result.ExitCode)): $($result.StdErr)"
    }

    $entries = foreach ($line in $result.StdOutLines) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        $idx = $line.LastIndexOf(':')
        if ($idx -lt 0) { continue }
        $relPath = $line.Substring(0, $idx)
        $rawHash = $line.Substring($idx + 1).Trim()
        # Normalize sha256sum's (already-lowercase, but forced defensively per the
        # design doc) hex to lowercase, while keeping the MISSING sentinel in the
        # same canonical uppercase form Get-VmManifestEntries and
        # Test-PackageManifestsMatch's own fallback both use - a blind
        # ToLowerInvariant() over the whole substring would otherwise quietly turn
        # the sentinel into "missing", which still compares as "different from any
        # real hash" either way but is needlessly inconsistent to read/debug.
        $hash = if ($rawHash -ieq 'MISSING') { 'MISSING' } else { $rawHash.ToLowerInvariant() }
        [pscustomobject]@{
            RelPath = $relPath
            Hash    = $hash
        }
    }
    # Same return-re-enumerates-arrays concern as Get-VmManifestEntries above; see
    # that comment. Applied here for the identical reason (RelPaths, hence the
    # number of lines this loop iterates over, is never empty but could be 1) -
    # and, just like there, this function's caller always wraps the call in
    # @(...), so the fix is a plain `return $entries` here, NEVER an additional
    # `return ,$entries` (which would double-wrap on top of the caller's @() and
    # silently break Count/foreach instead of protecting it).
    if ($null -eq $entries) { $entries = @() } else { $entries = @($entries) }
    return $entries
}

function ConvertTo-SortedOrdinalArray {
    param(
        [Parameter(Mandatory)][AllowEmptyCollection()][string[]] $Values
    )
    $copy = [string[]]$Values.Clone()
    [Array]::Sort($copy, [StringComparer]::Ordinal)
    # .Clone() on a typed array never yields $null (even for a 0-element source),
    # so no separate null-check is needed here unlike the manifest functions above
    # - but return would still re-enumerate a 0- or 1-element array into $null /
    # a bare scalar unless protected with the unary comma, so it's still required
    # HERE. This is the one function in this file that keeps `return ,$copy`,
    # because its one and only call site (Test-PackageManifestsMatch's own
    # `foreach ($rel in (ConvertTo-SortedOrdinalArray ...))` below) deliberately
    # consumes it directly via a bare foreach, with NO additional @() wrap -
    # CONFIRMED empirically correct for N=0/1/many in that shape. Do not "fix"
    # this by adding an @() around that call site: combined with this function's
    # own comma-protected return, that would double-wrap the result into a
    # 1-element array whose sole element is the whole real array (the exact bug
    # class the other manifest functions above deliberately avoid by relying on
    # their callers' @() alone instead of also using the comma here).
    return ,$copy
}

function Test-PackageManifestsMatch {
    param(
        [Parameter(Mandatory)][AllowEmptyCollection()][object[]] $LocalEntries,
        [Parameter(Mandatory)][AllowEmptyCollection()][object[]] $RemoteEntries
    )
    $localByPath = @{}
    foreach ($e in $LocalEntries) { $localByPath[$e.RelPath] = $e.Hash }
    $remoteByPath = @{}
    foreach ($e in $RemoteEntries) { $remoteByPath[$e.RelPath] = $e.Hash }

    $differing = New-Object System.Collections.Generic.List[string]
    foreach ($rel in (ConvertTo-SortedOrdinalArray -Values @($localByPath.Keys))) {
        $localHash = $localByPath[$rel]
        $remoteHash = if ($remoteByPath.ContainsKey($rel)) { $remoteByPath[$rel] } else { 'MISSING' }
        if ($localHash -cne $remoteHash) {
            $differing.Add($rel) | Out-Null
        }
    }

    return [pscustomobject]@{
        Matches          = ($differing.Count -eq 0)
        DifferingRelPaths = @($differing)
    }
}

# ---------------------------------------------------------------------------
# Atomic per-file replace (steps 6, 8, 10)
#
# Files are replaced INDIVIDUALLY, never via a whole-directory swap. This matters
# critically for Server Telemetry: its redb telemetry database lives at
# <deployment-directory>/log/*.redb, i.e. inside the very same directory as the
# executable and config.toml (see src\common\logging\store.rs). A whole-directory
# rename-swap would strand or lose that live data directory on every redeploy;
# per-file replace never touches sibling files/directories it doesn't explicitly know
# about, so this risk does not exist. The same per-file pattern is applied to Host in
# the VM and to Server on Ubuntu too, for consistency and because it also avoids
# clobbering anything unexpected under C:\MarixHost\ or /opt/marix/server/ outside the
# known package file set.
# ---------------------------------------------------------------------------

function Sync-FileToVmAtomic {
    param(
        [Parameter(Mandatory)][System.Management.Automation.Runspaces.PSSession] $Session,
        [Parameter(Mandatory)][string] $LocalPath,
        [Parameter(Mandatory)][string] $DestPath,
        [Parameter(Mandatory)][string] $ExpectedHash
    )
    $destDir = Split-Path -Parent $DestPath
    $newPath = "$DestPath.new"
    $oldPath = "$DestPath.old"

    Invoke-Command -Session $Session -ScriptBlock {
        param($Dir)
        if (-not (Test-Path -LiteralPath $Dir -PathType Container)) {
            New-Item -ItemType Directory -Path $Dir -Force | Out-Null
        }
    } -ArgumentList $destDir | Out-Null

    Copy-Item -Path $LocalPath -Destination $newPath -ToSession $Session -Force

    $remoteHash = Invoke-Command -Session $Session -ScriptBlock {
        param($Path)
        (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    } -ArgumentList $newPath

    if ([string]$remoteHash -cne $ExpectedHash) {
        throw "Hash mismatch after staging '$DestPath' in the VM: expected $ExpectedHash, got $remoteHash."
    }

    Invoke-Command -Session $Session -ScriptBlock {
        param($Final, $Old, $New)
        if (Test-Path -LiteralPath $Final -PathType Leaf) {
            if (Test-Path -LiteralPath $Old) {
                Remove-Item -LiteralPath $Old -Force
            }
            Rename-Item -LiteralPath $Final -NewName (Split-Path -Leaf $Old) -Force
        }
        Rename-Item -LiteralPath $New -NewName (Split-Path -Leaf $Final) -Force
    } -ArgumentList $DestPath, $oldPath, $newPath | Out-Null
}

function Sync-FileToRemoteAtomic {
    param(
        [Parameter(Mandatory)][hashtable] $Context,
        [Parameter(Mandatory)][string] $LocalPath,
        [Parameter(Mandatory)][string] $RemoteDestPath,
        [Parameter(Mandatory)][string] $ExpectedHash,
        # Linux, unlike Windows, requires an explicit execute permission bit on a
        # regular file before it can be run at all - even by root: root's usual
        # "bypass owner/group/other checks" privilege does NOT extend to skipping
        # the execute-bit check entirely; at least one x bit must be set, or every
        # execve() attempt fails with EACCES ("Permission denied"), CONFIRMED
        # empirically against this exact server (`timeout 3 ./marix-server-telemetry`
        # as root against a freshly scp'd, chmod-untouched file failed with exactly
        # that error). scp never invents an execute bit for a new destination file
        # copied from Windows (there is no Unix execute-bit concept on the NTFS
        # source side to preserve), so every package's main executable must be
        # explicitly marked executable here after staging, before the final rename.
        # Left plain data (config.toml, *.prompt) is deliberately NOT marked
        # executable, matching this Ubuntu deployment's established convention.
        [switch] $MakeExecutable
    )
    $remoteDir = $RemoteDestPath -replace '/[^/]*$', ''
    $remoteNew = "$RemoteDestPath.new"
    $remoteOld = "$RemoteDestPath.old"

    $mkdirResult = Invoke-DeploymentSsh -Context $Context -RemoteCommand "mkdir -p '$remoteDir'"
    if ($mkdirResult.ExitCode -ne 0) {
        throw "Failed to ensure remote directory '$remoteDir' exists (exit code $($mkdirResult.ExitCode)): $($mkdirResult.StdErr)"
    }

    Invoke-DeploymentScp -Context $Context -LocalPath $LocalPath -RemotePath $remoteNew

    $chmodLine = if ($MakeExecutable) { "chmod 755 '$remoteNew'" } else { '' }
    $verifyRenameCmd = @"
h=`$(sha256sum '$remoteNew' | awk '{print `$1}')
if [ "`$h" != "$ExpectedHash" ]; then
  echo "HASH_MISMATCH `$h"
  exit 1
fi
$chmodLine
if [ -f '$RemoteDestPath' ]; then
  rm -f '$remoteOld'
  mv '$RemoteDestPath' '$remoteOld'
fi
mv '$remoteNew' '$RemoteDestPath'
echo REPLACED
"@
    $result = Invoke-DeploymentSsh -Context $Context -RemoteCommand $verifyRenameCmd
    if ($result.ExitCode -ne 0) {
        throw "Failed to verify/replace remote file '$RemoteDestPath' (exit code $($result.ExitCode)): $($result.StdOutLines -join ' ') $($result.StdErr)"
    }
}

# ---------------------------------------------------------------------------
# Config.toml scalar value reading (steps 11, 12)
# ---------------------------------------------------------------------------

function Get-ConfigTomlValue {
    <#
    Extracts one scalar value from an already-resolved config.toml by key name,
    via a plain line-oriented regex rather than a full TOML parser. Sufficient
    because every config.toml this deployment tooling reads is flat (a handful of
    single-level [section] tables, no arrays, no multi-line strings), and the key
    names steps 11/12 read (host_port, telemetry_port) are unique across the whole
    file, so no [section]-scoping is needed to disambiguate them. Strips one layer
    of surrounding double quotes for quoted string values; numeric values (the
    only kind currently read by callers) are returned as their raw literal text.
    #>
    param(
        [Parameter(Mandatory)][string] $Path,
        [Parameter(Mandatory)][string] $Key
    )
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Config file was not found: $Path"
    }
    $pattern = "^\s*$([regex]::Escape($Key))\s*=\s*(.+?)\s*$"
    foreach ($line in [IO.File]::ReadAllLines($Path)) {
        $m = [regex]::Match($line, $pattern)
        if ($m.Success) {
            $value = $m.Groups[1].Value.Trim()
            if ($value.Length -ge 2 -and $value.StartsWith('"') -and $value.EndsWith('"')) {
                $value = $value.Substring(1, $value.Length - 2)
            }
            return $value
        }
    }
    throw "Key '$Key' was not found in config file: $Path"
}

# ---------------------------------------------------------------------------
# Remote process kill/confirm and detached start (steps 7, 9, 11, 12)
#
# Every remote command below is passed as a single argument to `ssh`, which the
# server executes as `bash -c "<entire script text>"`. Because the script text itself
# contains the literal target path as a substring, an UNANCHORED `pkill -f` / `pgrep
# -f` pattern risks matching the wrapping shell's own cmdline (which also contains
# that substring) instead of - or in addition to - the intended target. Every pattern
# here is therefore anchored as ^<path>$ (exact full-string match), and every target
# process is always launched via its bare absolute path with zero CLI arguments (cwd
# is still set with `cd` first, for `marix_path = "."` resolution) so the real
# process's cmdline is exactly equal to the anchored pattern while the much longer
# wrapping shell's cmdline never matches. `set -e` is deliberately never used in these
# snippets: pkill's "no matching process" exit code (1) must be treated as success,
# and mixing that with `set -e` is a classic footgun; explicit exit-code checks are
# used instead throughout for full auditability.
# ---------------------------------------------------------------------------

function Stop-RemoteProcessByPath {
    param(
        [Parameter(Mandatory)][hashtable] $Context,
        [Parameter(Mandatory)][string] $ExecutablePath,
        [int] $ConfirmTimeoutSeconds = 10
    )
    $pattern = "^$ExecutablePath`$"
    $remoteCmd = @"
set +e
pkill -f '$pattern' 2>/dev/null
rc=`$?
set -e
if [ `$rc -gt 1 ]; then
  echo "PKILL_ERROR `$rc"
  exit 1
fi
deadline=`$((SECONDS + $ConfirmTimeoutSeconds))
while pgrep -f '$pattern' >/dev/null 2>&1; do
  if [ `$SECONDS -ge `$deadline ]; then
    echo STILL_RUNNING_AFTER_TIMEOUT
    exit 1
  fi
  sleep 0.25
done
if [ `$rc -eq 0 ]; then
  echo CONFIRMED_STOPPED
else
  echo NOTHING_WAS_RUNNING
fi
"@
    $result = Invoke-DeploymentSsh -Context $Context -RemoteCommand $remoteCmd
    if ($result.ExitCode -ne 0) {
        throw "Failed to stop and confirm remote process '$ExecutablePath' (exit code $($result.ExitCode)): $($result.StdOutLines -join ' ') $($result.StdErr)"
    }
    return ($result.StdOutLines -join ' ').Trim()
}

function Start-RemoteProcessDetached {
    param(
        [Parameter(Mandatory)][hashtable] $Context,
        [Parameter(Mandatory)][string] $RemoteDir,
        [Parameter(Mandatory)][string] $ExecutablePath,
        [Parameter(Mandatory)][string] $OutputLogName
    )
    $pattern = "^$ExecutablePath`$"
    $remoteCmd = @"
cd '$RemoteDir' || { echo CD_FAILED; exit 1; }
nohup '$ExecutablePath' > '$OutputLogName' 2>&1 < /dev/null &
disown
sleep 0.3
if pgrep -f '$pattern' >/dev/null 2>&1; then
  echo STARTED
else
  echo START_FAILED_IMMEDIATE
  exit 1
fi
"@
    $result = Invoke-DeploymentSsh -Context $Context -RemoteCommand $remoteCmd
    if ($result.ExitCode -ne 0) {
        throw "Failed to start detached remote process '$ExecutablePath' (exit code $($result.ExitCode)): $($result.StdOutLines -join ' ') $($result.StdErr)"
    }
}

function Wait-RemoteTcpReady {
    # Bounded TCP readiness probe, adapted from the shape documented in
    # .github\agents\engineer-of-deployment.agent.md's "Startup order and readiness"
    # section - with the systemd `systemctl is-active` liveness fallback replaced by
    # an anchored `pgrep -f` liveness check, since these processes are no longer
    # managed by systemd.
    param(
        [Parameter(Mandatory)][hashtable] $Context,
        [Parameter(Mandatory)][string] $ProbeHost,
        [Parameter(Mandatory)][string] $Port,
        [Parameter(Mandatory)][string] $LivenessExecutablePath,
        [int] $TotalTimeoutSeconds = 30,
        [double] $PerAttemptTimeoutSeconds = 1,
        [double] $DelaySeconds = 0.25
    )
    $pattern = "^$LivenessExecutablePath`$"
    $remoteCmd = @"
probe_host='$ProbeHost'
probe_port='$Port'
deadline=`$((SECONDS + $TotalTimeoutSeconds))
until timeout $PerAttemptTimeoutSeconds bash -c 'exec 3<>/dev/tcp/`$1/`$2' _ "`$probe_host" "`$probe_port" 2>/dev/null; do
  if ! pgrep -f '$pattern' >/dev/null 2>&1; then
    echo "Process stopped before its TCP listener became ready" >&2
    exit 1
  fi
  if [ `$SECONDS -ge `$deadline ]; then
    echo "Timed out waiting for the TCP listener" >&2
    exit 1
  fi
  sleep $DelaySeconds
done
echo TCP_READY
"@
    $result = Invoke-DeploymentSsh -Context $Context -RemoteCommand $remoteCmd
    if ($result.ExitCode -ne 0) {
        throw "TCP readiness probe failed for port $Port (exit code $($result.ExitCode)): $($result.StdOutLines -join ' ') $($result.StdErr)"
    }
}
