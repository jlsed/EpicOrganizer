#requires -Version 7
<#
.SYNOPSIS
  Finish (rebase + squash + version + commit + fast-forward main), tear down, clean up, or
  abandon a task worktree in the pipelined worktree workflow.

.DESCRIPTION
  Mode finish  (default, run inside the task worktree):
    - acquires the finish lock, checks both trees are clean
    - rebases on the base branch (auto-resolves a version-only conflict in the configured
      version file)
    - applies the final version, if the project configures one (renumbered above main's
      when a sibling finished first)
    - runs the configured test command (commands.test) and, with -ReleaseBuild,
      commands.release
    - squashes all WIP commits into one: [vX.Y.Z (code): ]type: summary
    - fast-forwards main (linear history), updates the ledger

  Mode teardown (run from outside the worktree, e.g. main):
    - removes the worktree, deletes the branch, clears the ledger reservation

  Mode cleanup (run from the main repo):
    - tears down every shipped worktree (worktree, branch, launcher, ledger entry)

  Mode abandon (run from outside the worktree):
    - removes the worktree without merging, releases the reservation, deletes the branch
      unless -KeepBranch

  Project settings come from <repo>\worktree.config.json (tracked): version file + regexes,
  commands (test / release / stop), secret files, worktree root. Without a version block,
  versioning is off and commits are plain `type: summary`.

.EXAMPLE
  # inside the task worktree, after the inbox record is copied into memory-bank/
  .\Complete-TaskWorktree.ps1 -Type fix -Summary "keep Home MoM label on the same-day basis" -Body "..."

.EXAMPLE
  # from the main repo
  .\Complete-TaskWorktree.ps1 -Mode teardown -Slug fix-home-mom
  .\Complete-TaskWorktree.ps1 -Mode cleanup
#>
[CmdletBinding()]
param(
    [string]$WorktreePath = (Get-Location).Path,
    [string]$BaseBranch = "main",
    [ValidateSet('finish', 'teardown', 'abandon', 'cleanup')][string]$Mode = 'finish',
    [string]$Slug = "",
    [ValidateSet('feat', 'fix', 'refactor', 'test', 'docs', 'chore')][string]$Type,
    [string]$Summary = "",
    [string]$Body = "",
    [ValidateSet('sub', 'step', 'milestone')][string]$VersionKind = "",
    [switch]$NoVersionBump,
    [switch]$ReleaseBuild,
    [switch]$SkipTests,
    [switch]$KeepBranch,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

function Write-Utf8([string]$Path, [string]$Text) {
    [System.IO.File]::WriteAllText($Path, $Text, [System.Text.UTF8Encoding]::new($false))
}

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

function Remove-TaskWorktree([string]$MainRepo, [string]$Path, $StopCommand) {
    if (-not (Test-Path $Path)) { return $true }
    git -C $MainRepo worktree remove $Path --force 2>$null
    if ($LASTEXITCODE -eq 0 -and -not (Test-Path $Path)) { return $true }
    # git can drop the registration while leaving the directory (locked session), or the
    # registration may already be gone ("not a working tree"); fall back to a direct delete.
    Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction SilentlyContinue
    if (Test-Path $Path) {
        Write-Warning "Removal blocked (running session or file lock?); stopping daemons and retrying."
        $cmd = @()
        if ($StopCommand) { $cmd = @($StopCommand) }
        elseif (Test-Path (Join-Path $Path 'gradlew.bat')) { $cmd = @('./gradlew.bat', '--stop') }
        if ($cmd.Count -gt 0) {
            Push-Location $Path
            try { & $cmd[0] @($cmd | Select-Object -Skip 1) 2>$null | Out-Null } catch { } finally { Pop-Location }
        }
        Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction SilentlyContinue
    }
    git -C $MainRepo worktree prune 2>$null | Out-Null
    return (-not (Test-Path $Path))
}

function Invoke-ConfiguredCommand([string]$Label, $Command, [string]$WorkingDir) {
    $arr = @($Command)
    if ($arr.Count -eq 0) { return }
    Write-Host "Running $Label ($($arr -join ' ')) ..."
    Push-Location $WorkingDir
    try { & $arr[0] @($arr | Select-Object -Skip 1) } finally { Pop-Location }
    if ($LASTEXITCODE -ne 0) { throw "$Label failed (exit $LASTEXITCODE)." }
}

function Load-Ledger([string]$MainRepo) {
    $path = Join-Path $MainRepo '.parallel\ledger.json'
    if (Test-Path $path) {
        $ledger = Get-Content -Raw $path | ConvertFrom-Json
        if ($null -eq $ledger.reservations) { $ledger | Add-Member -NotePropertyName reservations -NotePropertyValue @() -Force }
        return $ledger
    }
    return [pscustomobject]@{
        worktreeRoot = ""
        shipped      = $null
        reservations = @()
    }
}

function Save-Ledger([string]$MainRepo, $Ledger) {
    $dir = Join-Path $MainRepo '.parallel'
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
    $Ledger.reservations = @($Ledger.reservations)
    Write-Utf8 (Join-Path $dir 'ledger.json') ($Ledger | ConvertTo-Json -Depth 12)
}

# ================================================================ setup
$self = (Resolve-Path $WorktreePath).Path
$main = Get-MainWorktree $self
$config = Get-ProjectConfig $main
if ($config -and $config.mainBranch -and -not $PSBoundParameters.ContainsKey('BaseBranch')) {
    $BaseBranch = [string]$config.mainBranch
}
$scheme = Get-VersionScheme $config
$ledger = Load-Ledger $main
$stopCommand = if ($config -and $config.commands) { $config.commands.stop } else { $null }

# Reconcile the ledger's shipped version with the actual version file (an out-of-band
# commit or pull can bump it without going through finish).
if ($null -ne $scheme) {
    $current = Read-Version (Join-Path $main $config.version.file) $config
    if (-not $ledger.shipped -or $null -eq $ledger.shipped.versionCode -or [int]$current.versionCode -gt [int]$ledger.shipped.versionCode) {
        Add-Member -InputObject $ledger -NotePropertyName shipped -NotePropertyValue ([pscustomobject]@{ versionName = $current.versionName; versionCode = $current.versionCode }) -Force
    }
}

if ($Mode -eq 'cleanup') {
    # ------------------------------------------------------------ cleanup all shipped
    $root = if ($config -and $config.worktreeRoot) { [string]$config.worktreeRoot }
    elseif ($ledger.worktreeRoot) { $ledger.worktreeRoot }
    else { Join-Path (Split-Path $main -Parent) ((Split-Path $main -Leaf) + '-wt') }
    $targets = @($ledger.reservations | Where-Object { $_.state -eq 'shipped' })
    if ($targets.Count -eq 0) { Write-Host "Nothing to clean: no shipped worktrees in the ledger."; exit 0 }
    if ($DryRun) {
        Write-Host "DRY RUN: would clean up:"
        foreach ($r in $targets) { Write-Host "  - $($r.branch)" }
        exit 0
    }
    $here = (Get-Location).Path
    $cleaned = 0
    foreach ($r in $targets) {
        $wt = $r.worktree
        if (-not $wt -or -not (Test-Path $wt)) { $wt = Join-Path $root $r.slug }
        if ($here.StartsWith($wt, [StringComparison]::OrdinalIgnoreCase)) {
            Write-Warning "Skipping $($r.branch): the current directory is inside $wt (close that session first)."
            continue
        }
        Write-Host "CLEANUP $($r.branch)"
        if (-not (Remove-TaskWorktree $main $wt $stopCommand)) {
            Write-Warning "Still locked; leaving $wt and its reservation for a later cleanup."
            continue
        }
        git -C $main branch -D $r.branch 2>$null | Out-Null
        if ($LASTEXITCODE -ne 0) { Write-Warning "Branch '$($r.branch)' was not deleted (already gone or checked out elsewhere)." }
        $launcher = Join-Path (Join-Path $root '.launchers') "$($r.slug).ps1"
        if (Test-Path $launcher) { Remove-Item $launcher -Force }
        $ledger.reservations = @($ledger.reservations | Where-Object { $_.branch -ne $r.branch })
        Save-Ledger $main $ledger
        $cleaned++
    }
    Write-Host "Cleaned $cleaned worktree(s)." -ForegroundColor Green
    if ($cleaned -lt $targets.Count) { Write-Host "Some were skipped; close their sessions and rerun cleanup." -ForegroundColor Yellow }
    exit 0
}

if ($Mode -ne 'finish') {
    # ------------------------------------------------------------ teardown / abandon
    $targetPath = $null
    $targetBranch = $null
    foreach ($b in (Get-WorktreeBlocks $main)) {
        $wtRaw = $b['worktree']
        if (-not $wtRaw) { continue }
        $wt = $wtRaw
        try { $wt = (Resolve-Path $wtRaw).Path } catch { }
        if ($wt -eq $main) { continue }
        $br = if ($b['branch']) { $b['branch'] -replace '^refs/heads/', '' } else { '' }
        if (($Slug -and $br -eq "task/$Slug") -or ((-not $Slug) -and $wt -eq $self -and $self -ne $main)) {
            $targetPath = $wt; $targetBranch = $br; break
        }
    }
    $root = if ($config -and $config.worktreeRoot) { [string]$config.worktreeRoot }
    elseif ($ledger.worktreeRoot) { $ledger.worktreeRoot }
    else { Join-Path (Split-Path $main -Parent) ((Split-Path $main -Leaf) + '-wt') }
    if (-not $targetPath -and $Slug) { $targetPath = Join-Path $root $Slug }
    if (-not $targetPath) { throw "Nothing to clean: pass -Slug <slug> and run from the main repo." }
    if (-not $targetBranch) { $targetBranch = "task/$Slug" }

    $here = (Get-Location).Path
    if ($here.StartsWith($targetPath, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Run $Mode from outside the worktree (e.g. from $main): Windows locks the worktree while this session holds it."
    }

    Write-Host "$($Mode.ToUpper()) task worktree"
    Write-Host "  worktree : $targetPath"
    Write-Host "  branch   : $targetBranch"
    if ($DryRun) { Write-Host "DRY RUN: no changes made."; exit 0 }

    if (-not (Remove-TaskWorktree $main $targetPath $stopCommand)) {
        throw "Worktree removal failed; close the session using $targetPath and retry."
    }
    if (-not $KeepBranch) {
        git -C $main branch -D $targetBranch 2>$null
        if ($LASTEXITCODE -ne 0) { Write-Warning "Branch '$targetBranch' was not deleted (already gone or checked out elsewhere)." }
    }
    if (-not $Slug) { $Slug = $targetBranch -replace '^task/', '' }
    $launcher = Join-Path (Join-Path $root '.launchers') "$Slug.ps1"
    if (Test-Path $launcher) { Remove-Item $launcher -Force }
    $ledger.reservations = @($ledger.reservations | Where-Object { $_.branch -ne $targetBranch })
    Save-Ledger $main $ledger
    Write-Host "Done. Ledger reservation cleared." -ForegroundColor Green
    exit 0
}

# ================================================================ finish
if (-not $Type -or -not $Summary) {
    throw "finish requires -Type <feat|fix|refactor|test|docs|chore> and -Summary `"<text>`"."
}
$branch = (git -C $self rev-parse --abbrev-ref HEAD).Trim()
if ($branch -notmatch '^task/') { throw "Finish must run inside a task worktree (branch is '$branch')." }
$slugFromBranch = $branch -replace '^task/', ''
if ($Slug -and $Slug -ne $slugFromBranch) { throw "-Slug '$Slug' does not match branch '$branch'." }
$Slug = $slugFromBranch

$reservation = @($ledger.reservations | Where-Object { $_.branch -eq $branch })[0]
$doBump = ($null -ne $scheme) -and (-not $NoVersionBump) -and (-not ($reservation -and -not $reservation.versionCode))

$final = $null
if ($doBump) {
    if ($reservation -and $reservation.versionCode) {
        $final = [pscustomobject]@{ versionName = $reservation.versionName; versionCode = [int]$reservation.versionCode; versionKind = $reservation.versionKind }
    }
    else {
        if ($ledger.shipped -and $ledger.shipped.versionCode) {
            $maxName = $ledger.shipped.versionName
        }
        else {
            $maxName = (Read-Version (Join-Path $main $config.version.file) $config).versionName
        }
        $kind = if ($VersionKind) { $VersionKind } else { 'sub' }
        $final = Next-Version $maxName $kind $scheme
    }
    $shippedCode = if ($ledger.shipped -and $ledger.shipped.versionCode) { [int]$ledger.shipped.versionCode } else { 0 }
    if ($final.versionCode -le $shippedCode) {
        $kind = if ($VersionKind) { $VersionKind } else { if ($final.versionKind) { $final.versionKind } else { 'sub' } }
        $final = Next-Version $ledger.shipped.versionName $kind $scheme
        Write-Host "NOTE: a sibling finished first; renumbering to v$($final.versionName) ($($final.versionCode))." -ForegroundColor Yellow
    }
}

Write-Host "Finish $branch -> $BaseBranch"
Write-Host "  worktree : $self"
Write-Host "  commit   : $(if ($doBump) { "v$($final.versionName) ($($final.versionCode)): " })$Type`: $Summary"
if (-not $doBump) { Write-Host "  version  : (versioning off)" }
if ($DryRun) { Write-Host "DRY RUN: no changes made."; exit 0 }

# ---------------------------------------------------------------- lock
$parallelDir = Join-Path $main '.parallel'
if (-not (Test-Path $parallelDir)) { New-Item -ItemType Directory -Path $parallelDir | Out-Null }
$lockPath = Join-Path $parallelDir 'finish.lock'
if (Test-Path $lockPath) {
    $age = (Get-Date) - (Get-Item $lockPath).LastWriteTime
    if ($age.TotalHours -lt 2) {
        throw "Another finish appears to be running (lock: $lockPath, age $([int]$age.TotalMinutes) min). Remove the lock if it is stale."
    }
    Write-Warning "Removing stale finish lock ($([int]$age.TotalHours) h old)."
    Remove-Item $lockPath -Force
}
Write-Utf8 $lockPath ("task=$branch pid=$PID at=$(Get-Date -Format o)")

try {
    # ------------------------------------------------------------ preconditions
    $selfDirty = @(git -C $self status --porcelain)
    if ($selfDirty.Count -gt 0) { throw "Worktree not clean - commit or discard first (the inbox is gitignored):`n$($selfDirty -join "`n")" }
    $mainBranchNow = (git -C $main rev-parse --abbrev-ref HEAD).Trim()
    if ($mainBranchNow -ne $BaseBranch) { throw "Main repo is on '$mainBranchNow', expected '$BaseBranch'." }
    $mainDirty = @(git -C $main status --porcelain)
    if ($mainDirty.Count -gt 0) { throw "Main repo not clean:`n$($mainDirty -join "`n")" }

    # ------------------------------------------------------------ rebase
    git -C $self rebase $BaseBranch
    if ($LASTEXITCODE -ne 0) {
        $unmerged = @(git -C $self diff --name-only --diff-filter=U)
        $versionFile = if ($doBump) { ($config.version.file -replace '\\', '/') } else { $null }
        $isVersionOnly = ($doBump -and $unmerged.Count -eq 1 -and ($unmerged[0] -replace '\\', '/') -eq $versionFile)
        if ($isVersionOnly) {
            Write-Host "Auto-resolving the expected version-line conflict in $versionFile (keeping $BaseBranch's file, re-applying the new version)." -ForegroundColor Yellow
            git -C $self checkout --ours -- $config.version.file
            if ($LASTEXITCODE -ne 0) { throw "auto-resolve (checkout --ours) failed." }
            Set-VersionFile (Join-Path $self $config.version.file) $final $config
            git -C $self add $config.version.file
            git -C $self -c core.editor=true rebase --continue
            if ($LASTEXITCODE -ne 0) { throw "Rebase still conflicts after the version auto-resolve. Resolve manually, then rerun finish." }
        }
        else {
            throw "Rebase conflicts in:`n$($unmerged -join "`n")`nResolve them in the worktree, git add, git rebase --continue, then rerun finish."
        }
    }

    # ------------------------------------------------------------ final version + tests
    if ($doBump) {
        Set-VersionFile (Join-Path $self $config.version.file) $final $config
    }

    if (-not $SkipTests) {
        $cmds = if ($config) { $config.commands } else { $null }
        if ($cmds -and $cmds.test) {
            try { Invoke-ConfiguredCommand "tests" $cmds.test $self }
            catch { throw "$($_.Exception.Message) Fix, commit (git add -A; git commit -m wip), and rerun finish." }
        }
        else {
            Write-Warning "No commands.test configured in worktree.config.json; skipping the test gate."
        }
        if ($ReleaseBuild) {
            if ($cmds -and $cmds.release) {
                try { Invoke-ConfiguredCommand "release build" $cmds.release $self }
                catch { throw "$($_.Exception.Message) Fix, commit (git add -A; git commit -m wip), and rerun finish." }
            }
            else {
                Write-Warning "No commands.release configured in worktree.config.json; skipping the release build."
            }
        }
    }

    # ------------------------------------------------------------ squash + commit
    git -C $self reset --soft $BaseBranch
    if ($LASTEXITCODE -ne 0) { throw "git reset --soft failed." }
    git -C $self add -A
    if ($LASTEXITCODE -ne 0) { throw "git add failed." }
    $staged = @(git -C $self diff --cached --name-only)
    if ($staged.Count -eq 0) { throw "Nothing to commit - the branch has no changes relative to $BaseBranch." }

    $firstLine = if ($doBump) { "v$($final.versionName) ($($final.versionCode)): $Type`: $Summary" } else { "$Type`: $Summary" }
    if ($Body) {
        git -C $self commit -q -m $firstLine -m $Body
    }
    else {
        git -C $self commit -q -m $firstLine
    }
    if ($LASTEXITCODE -ne 0) { throw "git commit failed." }

    # ------------------------------------------------------------ fast-forward main
    $attempt = 0
    while ($true) {
        $attempt++
        git -C $main merge --ff-only $branch
        if ($LASTEXITCODE -eq 0) { break }
        if ($attempt -ge 3) { throw "Could not fast-forward $BaseBranch (moved repeatedly). Rerun finish; the commit is safe on '$branch'." }
        Write-Warning "$BaseBranch moved during finish; rebasing the task commit and retrying."
        git -C $self rebase $BaseBranch
        if ($LASTEXITCODE -ne 0) { throw "Rebase against the moved $BaseBranch conflicted. Resolve in the worktree, git rebase --continue, then rerun finish." }
    }
    $sha = (git -C $main rev-parse --short HEAD).Trim()

    # ------------------------------------------------------------ ledger
    if ($doBump) {
        Add-Member -InputObject $ledger -NotePropertyName shipped -NotePropertyValue ([pscustomobject]@{ versionName = $final.versionName; versionCode = $final.versionCode }) -Force
    }
    if ($reservation) {
        $reservation.state = 'shipped'
        if ($doBump) {
            Add-Member -InputObject $reservation -NotePropertyName finalVersionName -NotePropertyValue $final.versionName -Force
            Add-Member -InputObject $reservation -NotePropertyName finalVersionCode -NotePropertyValue $final.versionCode -Force
        }
    }
    Save-Ledger $main $ledger

    Write-Host ""
    Write-Host "Merged $branch into $BaseBranch at $sha" -ForegroundColor Green
    Write-Host "  commit : $firstLine"
    Write-Host ""
    Write-Host "Teardown (run from outside the worktree, e.g. the main repo):"
    Write-Host "  pwsh .\.agents\skills\worktree-finish\scripts\Complete-TaskWorktree.ps1 -Mode teardown -Slug $Slug"
}
finally {
    if (Test-Path $lockPath) { Remove-Item $lockPath -Force }
}
