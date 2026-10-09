#requires -Version 7
<#
.SYNOPSIS
  Spawn an isolated task worktree for the pipelined worktree workflow.

.DESCRIPTION
  Creates <worktreeRoot>\<slug> on branch task/<slug> from the base branch, copies the
  configured untracked secret files (worktree.config.json -> secretFiles), optionally
  reserves the next app version (and optionally the next ADR number) in
  .parallel\ledger.json, applies the reserved version to the configured version file, and
  writes the task inbox (memory-bank\inbox\<task-id>.md). By default (-Open opencode) it
  then opens a new Windows Terminal window in the worktree running
  `opencode --prompt '<kickoff>'`; -Open shell opens a plain terminal, -Open none skips.

  The project config lives at <repo>\worktree.config.json (tracked). Without a version
  block, versioning is disabled (no reservations, no version-tagged commit). The repo is
  resolved from the current working directory, so master copies of this skill work from
  anywhere inside the project.

.EXAMPLE
  .\New-TaskWorktree.ps1 -Slug fix-home-mom -Title "Fix Home MoM basis" -VersionKind sub -NeedsAdr

.EXAMPLE
  .\New-TaskWorktree.ps1 -Slug fix-home-mom -Title "Fix Home MoM basis" -Open none

.EXAMPLE
  .\New-TaskWorktree.ps1 -ReserveAdr -Slug fix-home-mom
#>
[CmdletBinding()]
param(
    [string]$Slug,
    [string]$Title,
    [string]$Goal = "",
    [string]$Scope = "",
    [switch]$NeedsAdr,
    [switch]$ReserveAdr,
    [switch]$NoVersionBump,
    [ValidateSet('sub', 'step', 'milestone')][string]$VersionKind = 'sub',
    [ValidateSet('opencode', 'shell', 'none')][string]$Open = 'opencode',
    [string]$WorktreeRoot = "",
    [string]$BaseBranch = "main",
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

function Write-Utf8([string]$Path, [string]$Text) {
    [System.IO.File]::WriteAllText($Path, $Text, [System.Text.UTF8Encoding]::new($false))
}

function Quote-Arg([string]$s) {
    if ($s -match '\s') { return '"' + $s + '"' }
    return $s
}

function Get-MainWorktree([string]$From) {
    foreach ($line in (git -C $From worktree list --porcelain)) {
        if ($line.StartsWith('worktree ')) { return (Resolve-Path $line.Substring(9).Trim()).Path }
    }
    throw "Cannot locate a git worktree from '$From'."
}

function Assert-GitOk([string]$What) {
    if ($LASTEXITCODE -ne 0) { throw "$What failed (git exit $LASTEXITCODE)." }
}

function Get-ProjectConfig([string]$MainRepo) {
    $path = Join-Path $MainRepo 'worktree.config.json'
    if (-not (Test-Path $path)) { return $null }
    try { return Get-Content -Raw $path | ConvertFrom-Json }
    catch { throw "Invalid worktree.config.json: $($_.Exception.Message)" }
}

function Get-VersionScheme($Config) {
    if ($null -eq $Config -or $null -eq $Config.version) { return $null }
    if ($Config.version.scheme) { return [string]$Config.version.scheme }
    return 'milestone'
}

function Parse-Version([string]$Name, [string]$Scheme) {
    $core = ($Name -split '[-+]')[0]
    $p = $core.Split('.')
    if ($Scheme -eq 'semver') {
        return @{
            m   = if ($p.Length -ge 1 -and $p[0] -match '^\d+$') { [int]$p[0] } else { 0 }
            s   = if ($p.Length -ge 2) { [int]$p[1] } else { 0 }
            sub = if ($p.Length -ge 3) { [int]$p[2] } else { 0 }
        }
    }
    # milestone scheme: 0.{milestone}.{step}[.{sub}]
    return @{
        m   = if ($p.Length -ge 2) { [int]$p[1] } else { 0 }
        s   = if ($p.Length -ge 3) { [int]$p[2] } else { 0 }
        sub = if ($p.Length -ge 4) { [int]$p[3] } else { 0 }
    }
}

function Format-Version($v, [string]$Scheme) {
    if ($Scheme -eq 'semver') { return "$($v.m).$($v.s).$($v.sub)" }
    if ($v.sub -gt 0) { return "0.$($v.m).$($v.s).$($v.sub)" }
    return "0.$($v.m).$($v.s)"
}

function Next-Version([string]$FromName, [string]$Kind, [string]$Scheme) {
    $v = Parse-Version $FromName $Scheme
    switch ($Kind) {
        'sub' { $v.sub += 1 }
        'step' { $v.s += 1; $v.sub = 0 }
        'milestone' { $v.m += 1; $v.s = 0; $v.sub = 0 }
    }
    return [pscustomobject]@{
        versionName = Format-Version $v $Scheme
        versionCode = $v.m * 10000 + $v.s * 100 + $v.sub
        versionKind = $Kind
    }
}

function Read-Version([string]$Path, $Config) {
    $text = [IO.File]::ReadAllText($Path)
    $name = [regex]::Match($text, [string]$Config.version.nameRegex).Groups[1].Value
    if (-not $name) { throw "version.nameRegex did not match anything in $Path" }
    $scheme = Get-VersionScheme $Config
    $code = 0
    if ($Config.version.codeRegex) {
        $code = [int]([regex]::Match($text, [string]$Config.version.codeRegex).Groups[1].Value)
    }
    else {
        $v = Parse-Version $name $scheme
        $code = $v.m * 10000 + $v.s * 100 + $v.sub
    }
    return [pscustomobject]@{ versionName = $name; versionCode = $code }
}

function Set-VersionFile([string]$Path, $Version, $Config) {
    $text = [IO.File]::ReadAllText($Path)
    $replName = ([string]$Config.version.nameReplace).Replace('{name}', $Version.versionName).Replace('$', '$$')
    $text = [regex]::Replace($text, [string]$Config.version.nameRegex, $replName)
    if ($Config.version.codeReplace -and $Config.version.codeRegex) {
        $replCode = ([string]$Config.version.codeReplace).Replace('{code}', [string]$Version.versionCode).Replace('$', '$$')
        $text = [regex]::Replace($text, [string]$Config.version.codeRegex, $replCode)
    }
    Write-Utf8 $Path $text
}

function Load-Ledger([string]$MainRepo, $Config) {
    $path = Join-Path $MainRepo '.parallel\ledger.json'
    if (Test-Path $path) {
        $ledger = Get-Content -Raw $path | ConvertFrom-Json
        if ($null -eq $ledger.reservations) { $ledger | Add-Member -NotePropertyName reservations -NotePropertyValue @() -Force }
        return $ledger
    }
    $shipped = $null
    if ($null -ne $Config -and $null -ne $Config.version) {
        $v = Read-Version (Join-Path $MainRepo $Config.version.file) $Config
        $shipped = [pscustomobject]@{ versionName = $v.versionName; versionCode = $v.versionCode }
    }
    return [pscustomobject]@{
        worktreeRoot = ""
        shipped      = $shipped
        reservations = @()
    }
}

function Save-Ledger([string]$MainRepo, $Ledger) {
    $dir = Join-Path $MainRepo '.parallel'
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
    $Ledger.reservations = @($Ledger.reservations)
    Write-Utf8 (Join-Path $dir 'ledger.json') ($Ledger | ConvertTo-Json -Depth 12)
}

function Get-NextAdr([string]$MainRepo, $Ledger) {
    $max = 0
    $adrDir = Join-Path $MainRepo 'docs\adr'
    if (Test-Path $adrDir) {
        foreach ($f in (Get-ChildItem $adrDir -Filter '*.md')) {
            if ($f.Name -match '^(\d{4})-') {
                $n = [int]$Matches[1]
                if ($n -gt $max) { $max = $n }
            }
        }
    }
    foreach ($r in @($Ledger.reservations)) {
        if ($null -ne $r -and $r.adr -and $r.adr -match '^\d{4}$') {
            $n = [int]$r.adr
            if ($n -gt $max) { $max = $n }
        }
    }
    return ('{0:D4}' -f ($max + 1))
}

# ---------------------------------------------------------------- resolve repos + config
$cwd = (Get-Location).Path
$repoRoot = (git -C $cwd rev-parse --show-toplevel).Trim()
Assert-GitOk "git rev-parse"
$main = Get-MainWorktree $repoRoot
$config = Get-ProjectConfig $main
if ($config -and $config.mainBranch -and -not $PSBoundParameters.ContainsKey('BaseBranch')) {
    $BaseBranch = [string]$config.mainBranch
}
$scheme = Get-VersionScheme $config
$hasVersionConfig = ($null -ne $scheme)

$ledger = Load-Ledger $main $config

# Reconcile the ledger's shipped version with the actual version file (an out-of-band
# commit or pull can bump it without going through finish).
if ($hasVersionConfig) {
    $current = Read-Version (Join-Path $main $config.version.file) $config
    if (-not $ledger.shipped -or $null -eq $ledger.shipped.versionCode -or [int]$current.versionCode -gt [int]$ledger.shipped.versionCode) {
        Add-Member -InputObject $ledger -NotePropertyName shipped -NotePropertyValue ([pscustomobject]@{ versionName = $current.versionName; versionCode = $current.versionCode }) -Force
    }
}

if ($ReserveAdr) {
    if (-not $Slug) { throw "-ReserveAdr requires -Slug." }
    $branch = "task/$Slug"
    $reservation = @($ledger.reservations | Where-Object { $_.branch -eq $branch })[0]
    if (-not $reservation) { throw "No spawn reservation found for '$branch' in the ledger." }
    if ($reservation.adr) { Write-Host "ADR $($reservation.adr) is already reserved for $branch."; exit 0 }
    $adr = Get-NextAdr $main $ledger
    if ($DryRun) { Write-Host "DRY RUN: would reserve ADR $adr for $branch."; exit 0 }
    $reservation.adr = $adr
    Save-Ledger $main $ledger
    $inboxPath = Join-Path $reservation.worktree ("memory-bank\inbox\{0}.md" -f $reservation.taskId)
    if (Test-Path $inboxPath) {
        $t = [IO.File]::ReadAllText($inboxPath)
        $t = [regex]::Replace($t, '(?m)^- \*\*Reserved ADR:\*\*.*$', "- **Reserved ADR:** $adr")
        Write-Utf8 $inboxPath $t
    }
    Write-Host "Reserved ADR $adr for $branch. Create docs/adr/$adr-<slug>.md on the branch."
    exit 0
}

if (-not $Slug) { throw "Usage: New-TaskWorktree.ps1 -Slug <slug> -Title <title> [-Goal <>] [-Scope <>] [-VersionKind sub|step|milestone] [-NeedsAdr] [-NoVersionBump] [-Open opencode|shell|none]" }
if (-not $Title) { throw "-Title is required." }
if ($Slug -notmatch '^[a-z0-9][a-z0-9-]*$') { throw "Slug must match ^[a-z0-9][a-z0-9-]*$ (lowercase, dashes ok)." }

$branch = "task/$Slug"
$taskId = "$(Get-Date -Format 'yyyy-MM-dd')-$Slug"

# ---------------------------------------------------------------- preconditions
$mainBranchNow = (git -C $main rev-parse --abbrev-ref HEAD).Trim()
Assert-GitOk "git rev-parse main branch"
if ($mainBranchNow -ne $BaseBranch) { throw "Main repo is on '$mainBranchNow', expected '$BaseBranch'. Finish or park other work first." }
$dirty = @(git -C $main status --porcelain)
Assert-GitOk "git status"
if ($dirty.Count -gt 0) { throw "Main repo is not clean:`n$($dirty -join "`n")" }

$root = if ($WorktreeRoot) { $WorktreeRoot }
elseif ($config -and $config.worktreeRoot) { [string]$config.worktreeRoot }
elseif ($ledger.worktreeRoot) { $ledger.worktreeRoot }
else { Join-Path (Split-Path $main -Parent) ((Split-Path $main -Leaf) + '-wt') }
$wtPath = Join-Path $root $Slug

if (Test-Path $wtPath) { throw "Worktree path already exists: $wtPath" }
git -C $main rev-parse --verify --quiet "refs/heads/$branch" | Out-Null
if ($LASTEXITCODE -eq 0) { throw "Branch '$branch' already exists. Use -Mode abandon/teardown first, or pick another slug." }

# ---------------------------------------------------------------- reservations
$adr = $null
if ($NeedsAdr) { $adr = Get-NextAdr $main $ledger }

$res = $null
$reserveVersion = $hasVersionConfig -and -not $NoVersionBump
if ($reserveVersion) {
    if ($ledger.shipped -and $ledger.shipped.versionCode) {
        $maxName = $ledger.shipped.versionName
        $maxCode = [int]$ledger.shipped.versionCode
    }
    else {
        $v0 = Read-Version (Join-Path $main $config.version.file) $config
        $maxName = $v0.versionName
        $maxCode = [int]$v0.versionCode
    }
    foreach ($r in @($ledger.reservations)) {
        if ($null -ne $r -and $null -ne $r.versionCode -and [int]$r.versionCode -gt $maxCode) {
            $maxCode = [int]$r.versionCode
            $maxName = $r.versionName
        }
    }
    $res = Next-Version $maxName $VersionKind $scheme
}

$versionText = if ($reserveVersion) { "v$($res.versionName) ($($res.versionCode))" } else { "none (versioning off)" }
$adrText = if ($adr) { $adr } else { "none" }

Write-Host "Plan for task $taskId"
Write-Host "  worktree : $wtPath"
Write-Host "  branch   : $branch (from $BaseBranch)"
Write-Host "  version  : $versionText"
Write-Host "  ADR      : $adrText"
if ($DryRun) { Write-Host "DRY RUN: no changes made."; exit 0 }

# ---------------------------------------------------------------- create worktree
if (-not (Test-Path $root)) { New-Item -ItemType Directory -Path $root -Force | Out-Null }
git -C $main worktree add $wtPath -b $branch $BaseBranch
Assert-GitOk "git worktree add"

if ($config -and $config.secretFiles) {
    foreach ($rel in @($config.secretFiles)) {
        $src = Join-Path $main $rel
        $dst = Join-Path $wtPath $rel
        if (Test-Path $src) {
            $dir = Split-Path $dst -Parent
            if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
            Copy-Item $src $dst -Force
        }
        else {
            Write-Warning "Configured secret file not found in $main : $rel"
        }
    }
}

# ---------------------------------------------------------------- apply reserved version
if ($reserveVersion) {
    $file = Join-Path $wtPath $config.version.file
    Set-VersionFile $file $res $config
    git -C $wtPath add $config.version.file
    Assert-GitOk "git add version"
    git -C $wtPath commit -q -m "chore: reserve v$($res.versionName) ($($res.versionCode)) for $taskId"
    Assert-GitOk "git commit version reservation"
}

# ---------------------------------------------------------------- inbox
$inboxDir = Join-Path $wtPath 'memory-bank\inbox'
if (-not (Test-Path $inboxDir)) { New-Item -ItemType Directory -Path $inboxDir -Force | Out-Null }
$templatePath = Join-Path $PSScriptRoot '..\references\inbox-template.md'
$template = if (Test-Path $templatePath) { [IO.File]::ReadAllText($templatePath) } else { "# Task inbox - $taskId`n" }
$inbox = $template.
    Replace('{{TASK_ID}}', $taskId).
    Replace('{{TITLE}}', $Title).
    Replace('{{BRANCH}}', $branch).
    Replace('{{WORKTREE}}', $wtPath).
    Replace('{{VERSION}}', $versionText).
    Replace('{{ADR}}', $adrText).
    Replace('{{GOAL}}', $Goal).
    Replace('{{SCOPE}}', $Scope).
    Replace('{{CREATED}}', (Get-Date -Format 'yyyy-MM-dd HH:mm'))
Write-Utf8 (Join-Path $inboxDir "$taskId.md") $inbox

# ---------------------------------------------------------------- ledger
$reservation = [pscustomobject]@{
    taskId      = $taskId
    slug        = $Slug
    title       = $Title
    branch      = $branch
    worktree    = $wtPath
    versionName = $(if ($reserveVersion) { $res.versionName } else { $null })
    versionCode = $(if ($reserveVersion) { $res.versionCode } else { $null })
    versionKind = $(if ($reserveVersion) { $VersionKind } else { $null })
    adr         = $adr
    state       = 'active'
    created     = (Get-Date -Format 'o')
}
$ledger.worktreeRoot = $root
$ledger.reservations = @($ledger.reservations) + $reservation
Save-Ledger $main $ledger

# ---------------------------------------------------------------- summary + launch
Write-Host ""
Write-Host "Spawned task $taskId" -ForegroundColor Green
Write-Host "  worktree : $wtPath"
Write-Host "  branch   : $branch"
Write-Host "  inbox    : $inboxDir\$taskId.md"
Write-Host "  ledger   : $(Join-Path $main '.parallel\ledger.json')"

$kickoff = "Read all memory-bank core files plus the task inbox in memory-bank/inbox/ (exactly one file). Plan the task it describes, write the plan and the Planned files list into the inbox, then run the worktree-overlap-check skill before Build Mode."

if ($Open -ne 'none') {
    $launcherDir = Join-Path $root '.launchers'
    if (-not (Test-Path $launcherDir)) { New-Item -ItemType Directory -Path $launcherDir -Force | Out-Null }
    $launcherPath = Join-Path $launcherDir "$Slug.ps1"
    $launchBody = "Set-Location -LiteralPath '$wtPath'`n"
    if ($Open -eq 'opencode') {
        $launchBody += "opencode --prompt '$kickoff'`n"
    }
    else {
        $launchBody += "Write-Host 'Task worktree: $wtPath'`n"
    }
    Write-Utf8 $launcherPath $launchBody
    try {
        if (Get-Command wt.exe -ErrorAction SilentlyContinue) {
            Start-Process -FilePath 'wt.exe' -ArgumentList @('-d', (Quote-Arg $wtPath), 'pwsh', '-NoExit', '-File', (Quote-Arg $launcherPath)) | Out-Null
        }
        else {
            Start-Process -FilePath 'pwsh' -WorkingDirectory $wtPath -ArgumentList @('-NoExit', '-File', (Quote-Arg $launcherPath)) | Out-Null
        }
        $launched = if ($Open -eq 'opencode') { "a new terminal running opencode (kickoff prompt queued)" } else { "a new terminal at the worktree" }
        Write-Host "  launched : $launched" -ForegroundColor Green
        Write-Host "  launcher : $launcherPath"
    }
    catch {
        Write-Warning "Could not open a terminal automatically ($($_.Exception.Message))."
        Write-Host "Open one manually and run: pwsh -NoExit -File `"$launcherPath`""
    }
}
else {
    Write-Host ""
    Write-Host "Open an AI session in the worktree, then start with something like:"
    Write-Host "  Read memory-bank/ and the inbox at memory-bank/inbox/$taskId.md, plan '$Title',"
    Write-Host "  write the plan and the Planned files list into the inbox, then run the"
    Write-Host "  worktree-overlap-check skill before starting Build Mode."
}
