#requires -Version 7
<#
.SYNOPSIS
  Read-only status of the pipelined worktrees and reservation ledger.

.DESCRIPTION
  Lists every task reservation (slug, state, branch, reserved version, ADR, worktree
  presence, dirty-file count, last commit) plus the ledger's shipped version. Resolves the
  repo from the current working directory, so it works from main or any worktree, and from
  a master copy of the skill. Changes nothing.

.EXAMPLE
  .\Get-TaskStatus.ps1
#>
[CmdletBinding()]
param()

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

$main = Get-MainWorktree (Get-Location).Path
$ledgerPath = Join-Path $main '.parallel\ledger.json'

Write-Host "Worktree workflow status"
Write-Host "  main      : $main"
if (-not (Test-Path $ledgerPath)) {
    Write-Host "  ledger    : (none yet - no tasks spawned)"
    exit 0
}
$ledger = Get-Content -Raw $ledgerPath | ConvertFrom-Json
Write-Host "  ledger    : $ledgerPath"
if ($ledger.shipped -and $ledger.shipped.versionName) {
    Write-Host "  shipped   : v$($ledger.shipped.versionName) ($($ledger.shipped.versionCode))"
}
else {
    Write-Host "  shipped   : (versioning off)"
}
$cfgPath = Join-Path $main 'worktree.config.json'
if (Test-Path $cfgPath) {
    try {
        $cfg = Get-Content -Raw $cfgPath | ConvertFrom-Json
        if ($cfg.version -and $cfg.version.file) {
            $text = [IO.File]::ReadAllText((Join-Path $main ([string]$cfg.version.file)))
            $name = [regex]::Match($text, [string]$cfg.version.nameRegex).Groups[1].Value
            if ($name) { Write-Host "  version file: v$name" }
        }
    }
    catch { }
}
Write-Host ""

$wtByBranch = @{}
foreach ($b in (Get-WorktreeBlocks $main)) {
    if ($b['branch']) { $wtByBranch[($b['branch'] -replace '^refs/heads/', '')] = $b['worktree'] }
}

$rows = @()
foreach ($r in @($ledger.reservations)) {
    if ($null -eq $r) { continue }
    $wt = $wtByBranch[$r.branch]
    $dirty = ''
    $last = ''
    if ($wt) {
        $dirty = @(git -C $wt status --porcelain).Count
        $last = (git -C $wt log -1 '--format=%h %s' 2>$null)
    }
    $rows += [pscustomobject]@{
        slug    = $r.slug
        state   = $r.state
        branch  = $r.branch
        version = $r.versionName
        adr     = $r.adr
        wt      = $(if ($wt) { 'yes' } else { 'no' })
        dirty   = $dirty
        last    = $last
    }
}
$rows | Format-Table -AutoSize

$shipped = @($ledger.reservations | Where-Object { $_.state -eq 'shipped' }).Count
$active = @($ledger.reservations | Where-Object { $_.state -eq 'active' }).Count
Write-Host "active: $active   shipped (awaiting teardown): $shipped"
if ($shipped -gt 0) {
    Write-Host "Tear down the shipped ones from main after closing their sessions:"
    Write-Host "  pwsh .agents/skills/worktree-finish/scripts/Complete-TaskWorktree.ps1 -Mode cleanup"
}
