#requires -Version 7
<#
.SYNOPSIS
  Compare this worktree's planned files against every other active worktree.

.DESCRIPTION
  Reads the "Planned files" section of this worktree's inbox (plus -PlannedFiles), then
  scans every other worktree's declared files (its inbox), dirty files (git status) and
  committed diff vs the base branch. Main-only paths are excluded from hard conflicts.
  Appends the result to the inbox and exits 0 (GREEN) or 1 (RED).

.EXAMPLE
  .\Test-Overlap.ps1
.EXAMPLE
  .\Test-Overlap.ps1 -PlannedFiles ui/home/HomeScreen.kt,ui/home/BalanceHeroCard.kt
#>
[CmdletBinding()]
param(
    [string]$WorktreePath = (Get-Location).Path,
    [string[]]$PlannedFiles = @(),
    [string]$BaseBranch = "main",
    [switch]$NoInboxWrite
)

$ErrorActionPreference = 'Stop'

function Get-MainWorktree([string]$From) {
    foreach ($line in (git -C $From worktree list --porcelain)) {
        if ($line.StartsWith('worktree ')) { return (Resolve-Path $line.Substring(9).Trim()).Path }
    }
    throw "Cannot locate a git worktree from '$From'."
}

function Get-WorktreeBlocks([string]$From) {
    $blocks = @(); $cur = @{}
    foreach ($line in (git -C $From worktree list --porcelain)) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            if ($cur.Count -gt 0) { $blocks += , $cur; $cur = @{} }
            continue
        }
        $sp = $line.IndexOf(' ')
        if ($sp -lt 0) { continue }
        $cur[$line.Substring(0, $sp)] = $line.Substring($sp + 1)
    }
    if ($cur.Count -gt 0) { $blocks += , $cur }
    return $blocks
}

function Normalize([string]$p) {
    if ([string]::IsNullOrWhiteSpace($p)) { return '' }
    $t = $p.Trim().Replace('\', '/')
    while ($t.StartsWith('./')) { $t = $t.Substring(2) }
    return $t.ToLowerInvariant()
}

function Test-Excluded([string]$f) {
    if ($script:ExtraExcludes -and ($script:ExtraExcludes -contains $f)) { return $true }
    if ($f -match '^memory-bank/') { return $true }
    if ($f -match '^\.parallel/') { return $true }
    if ($f -eq 'local.properties' -or $f -eq '.env' -or $f -eq '.env.local') { return $true }
    if ($f -match '^\.agents/') { return $true }
    if ($f -match '^\.claude/') { return $true }
    if ($f -match '^\.clinerules/') { return $true }
    if ($f -match '^docs/adr/') { return $true }
    return $false
}

function Get-OwnInbox([string]$wt) {
    $dir = Join-Path $wt 'memory-bank\inbox'
    if (-not (Test-Path $dir)) { return $null }
    $file = Get-ChildItem $dir -Filter *.md -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($file) { return $file.FullName }
    return $null
}

function Get-InboxPlannedFiles([string]$wt) {
    $file = Get-OwnInbox $wt
    if (-not $file) { return @() }
    $lines = Get-Content $file
    $in = $false; $out = @()
    foreach ($l in $lines) {
        if ($l -match '^##\s+Planned files') { $in = $true; continue }
        if ($in -and $l -match '^##\s') { break }
        if (-not $in) { continue }
        $t = $l.Trim()
        if ($t -eq '' -or $t.StartsWith('<!--')) { continue }
        $t = $t -replace '^[-*]\s+', ''
        if ($t -match '[A-Za-z0-9]' -and -not $t.StartsWith('<')) { $out += $t }
    }
    return $out
}

function Get-DirtyFiles([string]$wt) {
    $out = @()
    foreach ($l in (git -C $wt status --porcelain)) {
        if ($l.Length -lt 4) { continue }
        $t = $l.Substring(3).Trim()
        if ($t -match ' -> ') { $t = ($t -split ' -> ')[-1].Trim() }
        $t = $t.Trim('"')
        if ($t) { $out += $t }
    }
    return $out
}

# ---------------------------------------------------------------- gather
$self = (Resolve-Path $WorktreePath).Path
$main = Get-MainWorktree $self
$selfBranch = (git -C $self rev-parse --abbrev-ref HEAD).Trim()

# Project-managed files come from worktree.config.json (version file) plus the config itself.
$script:ExtraExcludes = @('worktree.config.json')
$cfgPath = Join-Path $main 'worktree.config.json'
if (Test-Path $cfgPath) {
    try {
        $cfg = Get-Content -Raw $cfgPath | ConvertFrom-Json
        if ($cfg.version -and $cfg.version.file) { $script:ExtraExcludes += (Normalize ([string]$cfg.version.file)) }
    }
    catch { }
}

$planned = @()
$inboxPath = Get-OwnInbox $self
$planned += @(Get-InboxPlannedFiles $self)
$planned += $PlannedFiles
$planned = @($planned | ForEach-Object { Normalize $_ } | Where-Object { $_ } | Sort-Object -Unique)
$plannedHard = @($planned | Where-Object { -not (Test-Excluded $_) })

$records = @()
$otherCount = 0
foreach ($b in (Get-WorktreeBlocks $self)) {
    $wtRaw = $b['worktree']
    if (-not $wtRaw) { continue }
    $wt = $wtRaw
    try { $wt = (Resolve-Path $wtRaw).Path } catch { }
    if ($wt -eq $self -or $wt -eq $main) { continue }
    $otherCount++
    $branch = if ($b['branch']) { $b['branch'] -replace '^refs/heads/', '' } else { '(detached)' }

    foreach ($f in @(Get-DirtyFiles $wt)) {
        $records += [pscustomobject]@{ file = (Normalize $f); wt = $branch; kind = 'dirty' }
    }
    if ($b['branch']) {
        $committed = @(git -C $wt diff --name-only "$BaseBranch...$branch" 2>$null)
        if ($LASTEXITCODE -eq 0) {
            foreach ($f in $committed) {
                $records += [pscustomobject]@{ file = (Normalize $f); wt = $branch; kind = 'committed' }
            }
        }
    }
    foreach ($f in @(Get-InboxPlannedFiles $wt)) {
        $records += [pscustomobject]@{ file = (Normalize $f); wt = $branch; kind = 'declared' }
    }
}

$records = @($records | Where-Object { $_.file -and -not (Test-Excluded $_.file) })

$conflicts = @($records | Where-Object { $plannedHard -contains $_.file } | Sort-Object file, wt)

$plannedDirs = @($plannedHard | ForEach-Object { $i = $_.LastIndexOf('/'); if ($i -gt 0) { $_.Substring(0, $i) } } | Sort-Object -Unique)
$advisories = @($records | Where-Object {
        $_.file -notin $conflicts.file
    } | Where-Object {
        $i = $_.file.LastIndexOf('/')
        $i -gt 0 -and ($plannedDirs -contains $_.file.Substring(0, $i))
    } | Sort-Object file, wt -Unique)

$mainAhead = [int](git -C $self rev-list --count "HEAD..$BaseBranch")
if ($LASTEXITCODE -ne 0) { $mainAhead = -1 }

$result = if ($conflicts.Count -gt 0) { 'RED' } else { 'GREEN' }

# ---------------------------------------------------------------- report
Write-Host ""
Write-Host "Overlap check - $selfBranch" -ForegroundColor Cyan
Write-Host "  worktree : $self"
Write-Host "  planned  : $($planned.Count) declared / $($plannedHard.Count) considered"
Write-Host "  others   : $otherCount worktree(s)"
if ($mainAhead -gt 0) {
    Write-Host "  NOTE     : $BaseBranch is $mainAhead commit(s) ahead of this branch; finish will rebase." -ForegroundColor Yellow
}
Write-Host ""

if ($conflicts.Count -eq 0) {
    Write-Host "RESULT: GREEN - no file conflicts with other active worktrees." -ForegroundColor Green
}
else {
    Write-Host "RESULT: RED - $($conflicts.Count) overlapping file(s):" -ForegroundColor Red
    foreach ($c in $conflicts) {
        Write-Host ("  x {0}  [{1} in {2}]" -f $c.file, $c.kind, $c.wt)
    }
    Write-Host "Stop: wait for the other task to finish or re-scope the plan."
}
if ($advisories.Count -gt 0) {
    Write-Host ""
    Write-Host "Advisory - same directory, different files (merge-review worth a look):" -ForegroundColor Yellow
    foreach ($a in $advisories) {
        Write-Host ("  ~ {0}  [{1} in {2}]" -f $a.file, $a.kind, $a.wt)
    }
}
Write-Host ""

# ---------------------------------------------------------------- record in inbox
if ($inboxPath -and -not $NoInboxWrite) {
    $stamp = Get-Date -Format 'yyyy-MM-dd HH:mm'
    $conflictText = if ($conflicts.Count -eq 0) { 'none' } else { ($conflicts | ForEach-Object { "$($_.file) [$($_.wt)]" }) -join '; ' }
    $block = @(
        ""
        "## Overlap check $stamp"
        "result: $result"
        "planned: $($plannedHard -join ', ')"
        "conflicts: $conflictText"
        "advisories: $(($advisories | ForEach-Object { $_.file }) -join ', ')"
        "main-ahead: $mainAhead"
    )
    Add-Content -Path $inboxPath -Value $block -Encoding utf8
}

if ($conflicts.Count -gt 0) { exit 1 }
exit 0
