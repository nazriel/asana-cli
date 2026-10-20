# Install the asana CLI and its agent skill on Windows from a GitHub release.
#
#   irm https://raw.githubusercontent.com/nazriel/asana-cli/main/install.ps1 | iex
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/nazriel/asana-cli/main/install.ps1))) -Version v0.7.0
#
# Parameters (or the matching environment variables):
#   -Version TAG       release to install (ASANA_VERSION, default: latest)
#   -BinDir DIR        where asana.exe goes (ASANA_BIN_DIR, default: ~\.local\bin)
#   -SkillsDir DIR     where the skill goes (ASANA_SKILLS_DIR, default: ~\.agents\skills)
#   -NoBin             install only the skill
#   -NoSkill           install only the binary
#   -NoModifyPath      do not add BinDir to the user PATH
#
# The skill is linked into ~\.claude\skills when ~\.claude exists. An existing
# skill that is a link (for example to a development checkout) is left alone.
# ASANA_DOWNLOAD_URL replaces the release download URL (a mirror, or a local
# server in CI); it must serve the archive and SHA256SUMS for the version.
param(
    [string]$Version = $(if ($env:ASANA_VERSION) { $env:ASANA_VERSION } else { 'latest' }),
    [string]$BinDir = $(if ($env:ASANA_BIN_DIR) { $env:ASANA_BIN_DIR } else { Join-Path $HOME '.local\bin' }),
    [string]$SkillsDir = $(if ($env:ASANA_SKILLS_DIR) { $env:ASANA_SKILLS_DIR } else { Join-Path $HOME '.agents\skills' }),
    [switch]$NoBin,
    [switch]$NoSkill,
    [switch]$NoModifyPath
)

$ErrorActionPreference = 'Stop'
# Windows PowerShell renders download progress slowly enough to dominate the install.
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$repo = 'nazriel/asana-cli'

function Say([string]$Message) { Write-Host "asana-install: $Message" }
function Fail([string]$Message) { throw "asana-install: error: $Message" }

function Test-Link([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    return $item -and ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)
}

if ($PSVersionTable.PSEdition -eq 'Core' -and -not $IsWindows) {
    Fail 'install.ps1 is for Windows; use install.sh on macOS and Linux'
}

$arch = $null
try {
    $arch = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
} catch {
    $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
}
switch -Regex ($arch) {
    '^(X64|AMD64)$' { $target = 'x86_64-pc-windows-msvc' }
    '^(Arm64|ARM64)$' { $target = 'aarch64-pc-windows-msvc' }
    default { Fail "unsupported architecture: $arch" }
}

if ($Version -eq 'latest') {
    try {
        $Version = (Invoke-RestMethod -UseBasicParsing "https://api.github.com/repos/$repo/releases/latest").tag_name
    } catch {
        Fail "could not resolve the latest release: $($_.Exception.Message)"
    }
}

$archive = "asana-$Version-$target"
$base = if ($env:ASANA_DOWNLOAD_URL) { $env:ASANA_DOWNLOAD_URL } else { "https://github.com/$repo/releases/download/$Version" }
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("asana-install-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
    $zip = Join-Path $tmp "$archive.zip"
    $sums = Join-Path $tmp 'SHA256SUMS'
    Say "downloading $archive.zip"
    try {
        Invoke-WebRequest -UseBasicParsing -Uri "$base/$archive.zip" -OutFile $zip
        Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile $sums
    } catch {
        Fail "download failed from ${base}: $($_.Exception.Message)"
    }

    $expected = $null
    foreach ($line in Get-Content -LiteralPath $sums) {
        $fields = $line -split '\s+', 2
        if ($fields.Count -eq 2 -and $fields[1].TrimStart('*') -eq "$archive.zip") { $expected = $fields[0] }
    }
    if (-not $expected) { Fail "$archive.zip is not listed in SHA256SUMS" }
    $actual = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash
    if ($actual -ne $expected) { Fail "checksum mismatch for $archive.zip" }

    Expand-Archive -LiteralPath $zip -DestinationPath $tmp -Force
    $src = Join-Path $tmp $archive

    if (-not $NoBin) {
        New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
        $exe = Join-Path $BinDir 'asana.exe'
        # A running asana.exe cannot be overwritten, but it can be renamed.
        if (Test-Path -LiteralPath $exe) { Move-Item -LiteralPath $exe -Destination "$exe.old" -Force }
        Copy-Item -LiteralPath (Join-Path $src 'asana.exe') -Destination $exe
        Remove-Item -LiteralPath "$exe.old" -Force -ErrorAction SilentlyContinue
        Say "installed $exe ($Version)"

        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        $entries = @($userPath -split ';' | Where-Object { $_ })
        if ($entries -notcontains $BinDir) {
            if ($NoModifyPath) {
                Say "note: $BinDir is not on PATH"
            } else {
                [Environment]::SetEnvironmentVariable('Path', (($entries + $BinDir) -join ';'), 'User')
                $env:Path = "$env:Path;$BinDir"
                Say "added $BinDir to the user PATH; open a new terminal to use it"
            }
        }
    }

    if (-not $NoSkill) {
        $skill = Join-Path $SkillsDir 'asana-cli'
        if (Test-Link $skill) {
            Say "skipped skill: $skill is a link"
        } else {
            New-Item -ItemType Directory -Force -Path $SkillsDir | Out-Null
            if (Test-Path -LiteralPath $skill) { Remove-Item -LiteralPath $skill -Recurse -Force }
            Copy-Item -LiteralPath (Join-Path $src 'skills\asana-cli') -Destination $skill -Recurse
            Say "installed skill $skill"
        }

        $claudeSkill = Join-Path $HOME '.claude\skills\asana-cli'
        if ((Test-Path -LiteralPath (Join-Path $HOME '.claude')) -and ($claudeSkill -ne $skill)) {
            if (Test-Link $claudeSkill) {
                # Already linked, possibly to a development checkout.
            } elseif (Test-Path -LiteralPath $claudeSkill) {
                Say "skipped Claude Code link: $claudeSkill already exists"
            } else {
                New-Item -ItemType Directory -Force -Path (Split-Path $claudeSkill) | Out-Null
                # A junction needs neither administrator rights nor developer mode.
                New-Item -ItemType Junction -Path $claudeSkill -Target $skill | Out-Null
                Say "linked $claudeSkill -> $skill"
            }
        }
    }

    if (-not $NoBin) {
        Say 'next: run `asana auth login`'
    }
} finally {
    Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
