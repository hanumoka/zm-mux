<#
  Clone reference repositories for zm-mux research (read-only study material).

  reference/ is gitignored - NOT part of the zm-mux repo. The tracked inventory
  (URL / license / reuse-class / pinned SHA) lives in
  docs/research/05-reference-inventory.md.

  Reuse policy (clean-room): MIT/Apache repos are SAFE (learn + reuse). GPL/AGPL
  repos are STUDY-ONLY - read for understanding, never copy code/text into zm-mux.

  Usage:  pwsh scripts/clone-references.ps1
#>
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
$ref  = Join-Path $root 'reference'
New-Item -ItemType Directory -Force -Path $ref | Out-Null

# name = directory under reference/
$repos = @(
  @{ name='cmux';              url='https://github.com/manaflow-ai/cmux' }              # STUDY (GPL-3.0) reproduction target
  @{ name='wezterm';           url='https://github.com/wezterm/wezterm' }              # SAFE  (MIT)
  @{ name='zellij';            url='https://github.com/zellij-org/zellij' }            # SAFE  (MIT)
  @{ name='alacritty';         url='https://github.com/alacritty/alacritty' }          # SAFE  (Apache/MIT)
  @{ name='vte';               url='https://github.com/alacritty/vte' }                # SAFE  (Apache/MIT)
  @{ name='cosmic-term';       url='https://github.com/pop-os/cosmic-term' }           # STUDY (GPL-3.0)
  @{ name='psmux';             url='https://github.com/psmux/psmux' }                  # ? verify
  @{ name='wmux-amirlehmam';   url='https://github.com/amirlehmam/wmux' }              # STUDY (AGPL?)
  @{ name='wmux-openwong2kim'; url='https://github.com/openwong2kim/wmux' }            # ? verify
  @{ name='cmux-for-linux';    url='https://github.com/cai0baa/cmux-for-linux' }       # STUDY (GPL)
  @{ name='cmux-linux';        url='https://github.com/bradwilson331/cmux-linux' }     # STUDY (GPL?)
)

Write-Host "=== zm-mux reference clone (shallow) ==="
foreach ($r in $repos) {
  $dest = Join-Path $ref $r.name
  if (Test-Path (Join-Path $dest '.git')) { Write-Host "SKIP  $($r.name) (exists)"; continue }
  Write-Host "CLONE $($r.name)  <-  $($r.url)"
  git clone --depth 1 $r.url $dest *> $null
  if (Test-Path (Join-Path $dest '.git')) {
    Write-Host "  OK   $($r.name) @ $(git -C $dest rev-parse HEAD)"
  } else {
    Write-Host "  FAIL $($r.name)  ($($r.url))  -- record as non-existent / private in inventory"
  }
}

# microsoft/terminal is huge: sparse partial clone, only ConPTY sample + docs.
$mt = Join-Path $ref 'microsoft-terminal'
if (-not (Test-Path (Join-Path $mt '.git'))) {
  Write-Host "CLONE microsoft-terminal (sparse: samples + doc)  <-  https://github.com/microsoft/terminal"
  git clone --depth 1 --filter=blob:none --sparse https://github.com/microsoft/terminal $mt *> $null
  if (Test-Path (Join-Path $mt '.git')) {
    git -C $mt sparse-checkout set samples doc *> $null
    Write-Host "  OK   microsoft-terminal @ $(git -C $mt rev-parse HEAD) (sparse)"
  } else {
    Write-Host "  FAIL microsoft-terminal"
  }
}

Write-Host ""
Write-Host "=== summary: repo / short-sha / license-file ==="
Get-ChildItem -Directory $ref | ForEach-Object {
  $d = $_.FullName
  if (-not (Test-Path (Join-Path $d '.git'))) { return }
  $lic = Get-ChildItem $d -File | Where-Object { $_.Name -match '^(?i)(LICENSE|COPYING|LICENCE)' } | Select-Object -First 1
  $sha = git -C $d rev-parse --short HEAD 2>$null
  "  {0,-20} sha={1,-10} license_file={2}" -f $_.Name, $sha, ($(if ($lic) { $lic.Name } else { 'NONE' }))
}
Write-Host "=== done ==="
