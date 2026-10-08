<#
.SYNOPSIS
Installs HoploDex's machine-wide Windows build prerequisites.

.DESCRIPTION
The machine-wide half of a Windows development setup (DEVELOPMENT.md,
"Windows"). Run it once per computer, from an elevated PowerShell, then run
setup-user.ps1 as each account that builds:

  powershell -ExecutionPolicy Bypass -File setup-system.ps1 -User alice [-AutoLogon]

It installs, skipping whatever is already there:
  - Visual Studio Build Tools 2026 with the "Desktop development with C++"
    workload and its recommended parts (the MSVC compiler and linker, and a
    Windows SDK). A Visual Studio that already has the C++ tools counts.
  - Git, machine-wide.
  - Node.js 24 LTS, pinned in winget so an upgrade stays on 24.
  - Python 3.13 (the dev container's), machine-wide, on PATH, with the py
    launcher.
  - The WebView2 runtime, if Windows doesn't already have it.
  - vcpkg in -VcpkgRoot, with OpenSSL 3 (openssl:x64-windows-static-md) for
    SQLCipher, and the machine-wide OPENSSL_DIR and OPENSSL_STATIC=1 the
    build reads. OpenSSL 1.1 isn't GPLv3-compatible, so it checks for 3.
  - Win32 long paths, which deep node_modules and target paths can need.
  - Remote access for -User: Remote Desktop (with Network Level
    Authentication) and the OpenSSH server, both on and open in the
    firewall, and -User in the "Remote Desktop Users" and "OpenSSH Users"
    groups. SSH is restricted to "OpenSSH Users" (AllowGroups in
    sshd_config), so administrators can't sign in over SSH unless they're
    in it too.
  - On Windows Server, Server Manager no longer opens at sign-in.
  - With -AutoLogon, a test machine's unattended desktop session (E2E and
    screenshots need one): Windows signs -User in at startup, its password
    kept as an LSA secret rather than in the registry, and the session
    never locks, blanks or sleeps when idle. Windows' animations are on in
    it: with them off, WebView2 reports reduced motion and the E2E tests
    of the app's animations fail.
  - With -GitHubCli, the GitHub CLI.

It can be run again: it adds only what's missing. For x64 Windows 10 or 11
Pro, Enterprise or Education, or Windows Server 2025 with Desktop
Experience. Not Home, which has no Remote Desktop host.

.PARAMETER User
The account that will build and connect remotely, usually not an
administrator: a local account (alice) or a domain one (DOMAIN\alice).

.PARAMETER VcpkgRoot
Where to clone vcpkg. Defaults to C:\vcpkg.

.PARAMETER AutoLogon
Sign -User in automatically at startup, turn off the idle lock, screen
saver, display timeout and sleep, and turn on -User's Windows animations
("Show animations in Windows"). For a test machine only: anyone who can
reach its console gets -User's session.

.PARAMETER Password
-User's password for -AutoLogon. Asked for when it's left out.

.PARAMETER GitHubCli
Also install the GitHub CLI (gh).
#>
#Requires -Version 5.1
#Requires -RunAsAdministrator
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$User,
    [string]$VcpkgRoot = 'C:\vcpkg',
    [switch]$AutoLogon,
    [SecureString]$Password,
    [switch]$GitHubCli
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$NodeMajor = 24
# The dev container's Python (Debian trixie's python3).
$PythonVersion = '3.13'
$OpenSslTriplet = 'x64-windows-static-md'
$VcWorkload = 'Microsoft.VisualStudio.Workload.VCTools'
$VcTools = 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64'
# Builtin\Remote Desktop Users, whose name depends on the Windows language.
$RemoteDesktopUsersSid = 'S-1-5-32-555'
$OpenSshUsers = 'OpenSSH Users'
$Winlogon = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon'
# winget's "reboot needed to finish installation" (0x8A150109).
$WingetRebootNeeded = -1978334967
$script:RebootNeeded = $false

function Write-Step([string]$Message) {
    Write-Host "==> $Message" -ForegroundColor Cyan
}

# Runs a program, shows its output, and returns its exit code, throwing if
# that isn't one of OkCodes.
function Invoke-Checked {
    param([string]$File, [string[]]$Arguments, [int[]]$OkCodes = @(0))
    # Progress on stderr (git, winget) is not an error.
    $ErrorActionPreference = 'Continue'
    & $File @Arguments | Out-Host
    if ($OkCodes -notcontains $LASTEXITCODE) {
        throw "$File $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
    return $LASTEXITCODE
}

# Installers change PATH in the registry, not in this session.
function Update-SessionPath {
    $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
        [Environment]::GetEnvironmentVariable('Path', 'User')
}

function Test-Command([string]$Name) {
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Install-WingetPackage {
    param([string]$Id, [string[]]$Extra = @())
    $arguments = @('install', '--id', $Id, '--exact', '--source', 'winget',
        '--accept-source-agreements', '--accept-package-agreements',
        '--disable-interactivity') + $Extra
    $code = Invoke-Checked winget $arguments -OkCodes @(0, $WingetRebootNeeded)
    if ($code -eq $WingetRebootNeeded) { $script:RebootNeeded = $true }
    Update-SessionPath
}

function Get-VsWhere {
    return Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
}

# The newest Visual Studio or Build Tools that has the x64 C++ tools.
function Get-VcInstallation {
    $vswhere = Get-VsWhere
    if (-not (Test-Path $vswhere)) { return $null }
    $path = & $vswhere -latest -products * -requires $VcTools -property installationPath
    if ($path) { return $path }
    return $null
}

# Adds a member to a local group unless it's already in it.
function Add-GroupMember {
    param([string]$Description, [hashtable]$Group, [string]$Sid)
    try {
        Add-LocalGroupMember @Group -Member $Sid
        Write-Host "Added $User to $Description."
    } catch {
        if ($_.FullyQualifiedErrorId -notlike 'MemberExists*') { throw }
        Write-Host "$User is already in $Description."
    }
}

# Whether Password is Domain\Name's password.
function Test-Password {
    param([string]$Domain, [string]$Name, [SecureString]$Password)
    Add-Type -AssemblyName System.DirectoryServices.AccountManagement
    $contextType = [System.DirectoryServices.AccountManagement.ContextType]::Domain
    if ($Domain -eq $env:COMPUTERNAME) {
        $contextType = [System.DirectoryServices.AccountManagement.ContextType]::Machine
    }
    $context = New-Object System.DirectoryServices.AccountManagement.PrincipalContext($contextType, $Domain)
    try {
        $plain = (New-Object System.Net.NetworkCredential('', $Password)).Password
        return $context.ValidateCredentials($Name, $plain)
    } finally {
        $context.Dispose()
    }
}

# Stores Password as the LSA secret Winlogon reads for auto-logon
# (DefaultPassword), as Sysinternals Autologon does, so it isn't left in the
# registry, where any local user can read Winlogon's values.
function Set-AutoLogonSecret([SecureString]$Password) {
    if (-not ('HoploDexLsaSecret' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

public static class HoploDexLsaSecret {
    [StructLayout(LayoutKind.Sequential)]
    private struct LsaUnicodeString {
        public ushort Length;
        public ushort MaximumLength;
        public IntPtr Buffer;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct LsaObjectAttributes {
        public int Length;
        public IntPtr RootDirectory;
        public IntPtr ObjectName;
        public uint Attributes;
        public IntPtr SecurityDescriptor;
        public IntPtr SecurityQualityOfService;
    }

    [DllImport("advapi32.dll")]
    private static extern uint LsaOpenPolicy(IntPtr systemName, ref LsaObjectAttributes attributes,
        uint access, out IntPtr policy);

    [DllImport("advapi32.dll")]
    private static extern uint LsaStorePrivateData(IntPtr policy, ref LsaUnicodeString keyName,
        ref LsaUnicodeString privateData);

    [DllImport("advapi32.dll")]
    private static extern uint LsaClose(IntPtr policy);

    [DllImport("advapi32.dll")]
    private static extern int LsaNtStatusToWinError(uint status);

    private const uint PolicyCreateSecret = 0x20;

    // Stores the UTF-16 buffer secret, length characters long, under keyName.
    public static void Store(string keyName, IntPtr secret, int length) {
        LsaObjectAttributes attributes = new LsaObjectAttributes();
        attributes.Length = Marshal.SizeOf(typeof(LsaObjectAttributes));
        IntPtr policy;
        Check(LsaOpenPolicy(IntPtr.Zero, ref attributes, PolicyCreateSecret, out policy));
        IntPtr keyBuffer = Marshal.StringToHGlobalUni(keyName);
        try {
            LsaUnicodeString key = new LsaUnicodeString();
            key.Length = (ushort)(keyName.Length * 2);
            key.MaximumLength = (ushort)(keyName.Length * 2 + 2);
            key.Buffer = keyBuffer;
            LsaUnicodeString data = new LsaUnicodeString();
            data.Length = (ushort)(length * 2);
            data.MaximumLength = (ushort)(length * 2);
            data.Buffer = secret;
            Check(LsaStorePrivateData(policy, ref key, ref data));
        } finally {
            Marshal.FreeHGlobal(keyBuffer);
            LsaClose(policy);
        }
    }

    private static void Check(uint status) {
        if (status != 0) {
            throw new Win32Exception(LsaNtStatusToWinError(status));
        }
    }
}
'@
    }
    $buffer = [Runtime.InteropServices.Marshal]::SecureStringToGlobalAllocUnicode($Password)
    try {
        [HoploDexLsaSecret]::Store('DefaultPassword', $buffer, $Password.Length)
    } finally {
        [Runtime.InteropServices.Marshal]::ZeroFreeGlobalAllocUnicode($buffer)
    }
}

# Runs Action with the reg.exe path of -User's own registry hive, loading the
# hive while they're signed out. Returns $false if they've never signed in,
# so they have no hive yet.
function Invoke-InUserHive([scriptblock]$Action) {
    $hive = "HKU\$UserSid"
    $loaded = $false
    if (-not (Test-Path "Registry::HKEY_USERS\$UserSid")) {
        $profileKey = "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\$UserSid"
        $profileDir = (Get-ItemProperty $profileKey -ErrorAction SilentlyContinue).ProfileImagePath
        if (-not $profileDir) { return $false }
        $hive = 'HKU\HoploDexSetup'
        Invoke-Checked reg @('load', $hive, (Join-Path $profileDir 'NTUSER.DAT')) | Out-Null
        $loaded = $true
    }
    try {
        & $Action $hive | Out-Null
    } finally {
        if ($loaded) { Invoke-Checked reg @('unload', $hive) | Out-Null }
    }
    return $true
}

# Sets REG_SZ values under a key in -User's own registry hive.
function Set-UserRegistryValue {
    param([string]$Key, [hashtable]$Values)
    return Invoke-InUserHive {
        param($hive)
        foreach ($name in $Values.Keys) {
            Invoke-Checked reg @('add', "$hive\$Key", '/v', $name, '/t', 'REG_SZ',
                '/d', $Values[$name], '/f') | Out-Null
        }
    }
}

# Turns on "Show animations in Windows" (Settings > Accessibility > Visual
# effects) for -User, from their next sign-in. It's one bit of
# UserPreferencesMask, the one SPI_SETCLIENTAREAANIMATION sets; the rest of
# the mask stays as it is.
function Enable-UserAnimations {
    return Invoke-InUserHive {
        param($hive)
        $key = "$hive\Control Panel\Desktop"
        $ErrorActionPreference = 'Continue'
        $query = reg query $key /v UserPreferencesMask
        $ErrorActionPreference = 'Stop'
        if ($LASTEXITCODE -ne 0 -or "$query" -notmatch 'REG_BINARY\s+([0-9A-Fa-f]+)') {
            throw "No UserPreferencesMask in $key."
        }
        $hex = $Matches[1]
        $mask = [byte[]](0..($hex.Length / 2 - 1) | ForEach-Object { [Convert]::ToByte($hex.Substring($_ * 2, 2), 16) })
        if ($mask.Length -lt 5) { throw "UserPreferencesMask in $key is only $($mask.Length) bytes." }
        $mask[4] = $mask[4] -bor 0x02
        Invoke-Checked reg @('add', $key, '/v', 'UserPreferencesMask', '/t', 'REG_BINARY',
            '/d', (($mask | ForEach-Object { $_.ToString('X2') }) -join ''), '/f') | Out-Null
    }
}

function Test-WindowsSdk {
    $lib = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\Lib\*\um\x64\kernel32.Lib'
    return [bool](Get-ChildItem $lib -ErrorAction SilentlyContinue)
}

if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {
    throw "This setup is for x64 Windows; this computer is $env:PROCESSOR_ARCHITECTURE."
}
$edition = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion').EditionID
if ($edition -like 'Core*') {
    throw "Windows Home ($edition) has no Remote Desktop host; use Pro, Enterprise, Education or Server."
}
try {
    $UserSid = (New-Object System.Security.Principal.NTAccount($User)).Translate(
        [System.Security.Principal.SecurityIdentifier]).Value
} catch [System.Security.Principal.IdentityNotMappedException] {
    throw "There's no account named $User on this computer."
}
# The account's own spelling, DOMAIN\name or COMPUTER\name, for Winlogon.
$LogonDomain, $LogonName = (New-Object System.Security.Principal.SecurityIdentifier($UserSid)).Translate(
    [System.Security.Principal.NTAccount]).Value.Split('\', 2)
if ($AutoLogon) {
    # Ask now, so the rest runs unattended.
    if (-not $Password) { $Password = Read-Host "$User's password, for auto-logon" -AsSecureString }
    if (-not (Test-Password -Domain $LogonDomain -Name $LogonName -Password $Password)) {
        throw "That isn't $User's password."
    }
}
if (-not (Test-Command winget)) {
    throw 'winget is missing. Install "App Installer" from the Microsoft Store, then run this again.'
}

Write-Step 'C++ build tools'
if ((Get-VcInstallation) -and (Test-WindowsSdk)) {
    Write-Host "Found the C++ tools in $(Get-VcInstallation)."
} else {
    $vswhere = Get-VsWhere
    $buildTools = $null
    if (Test-Path $vswhere) {
        $buildTools = & $vswhere -latest -products Microsoft.VisualStudio.Product.BuildTools -property installationPath
    }
    if ($buildTools) {
        # winget won't add a workload to an existing install, so ask the
        # Visual Studio Installer to.
        Write-Host "Adding the C++ workload to $buildTools."
        $installer = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\setup.exe'
        $process = Start-Process $installer -Wait -PassThru -ArgumentList @(
            'modify', '--installPath', "`"$buildTools`"", '--add', $VcWorkload,
            '--includeRecommended', '--passive', '--norestart')
        if ($process.ExitCode -eq 3010) {
            $script:RebootNeeded = $true
        } elseif ($process.ExitCode -ne 0) {
            throw "The Visual Studio Installer failed with exit code $($process.ExitCode)."
        }
    } else {
        Install-WingetPackage 'Microsoft.VisualStudio.BuildTools' @(
            '--override', "--wait --passive --norestart --add $VcWorkload --includeRecommended")
    }
    if (-not (Get-VcInstallation)) { throw 'The C++ tools are still missing after installing them.' }
    if (-not (Test-WindowsSdk)) { throw 'The Windows SDK is still missing after installing the C++ tools.' }
}

Write-Step 'Git'
if (Test-Command git) {
    Write-Host "Found $(git --version)."
} else {
    Install-WingetPackage 'Git.Git' @('--scope', 'machine')
}

Write-Step "Node.js $NodeMajor LTS"
if (Test-Command node) {
    $found = (node --version).TrimStart('v')
    if (([version]$found).Major -ne $NodeMajor) {
        throw "Node.js $found is installed; HoploDex needs $NodeMajor. Uninstall it, then run this again."
    }
    Write-Host "Found Node.js $found."
} else {
    # OpenJS.NodeJS.LTS follows whichever release is LTS, so pick the newest 24.x.
    $versions = @(winget show --id OpenJS.NodeJS.LTS --exact --versions --source winget `
        --accept-source-agreements --disable-interactivity |
        Where-Object { $_ -match "^\s*$NodeMajor\.\d+\.\d+\s*$" } |
        ForEach-Object { [version]$_.Trim() } |
        Sort-Object -Descending)
    if (-not $versions) { throw "winget lists no Node.js $NodeMajor.x release." }
    Install-WingetPackage 'OpenJS.NodeJS.LTS' @('--version', $versions[0].ToString())
}
# Keep "winget upgrade --all" from moving to the next LTS.
Invoke-Checked winget @('pin', 'add', '--id', 'OpenJS.NodeJS.LTS', '--exact',
    '--version', "$NodeMajor.*", '--force', '--accept-source-agreements',
    '--disable-interactivity') | Out-Null

Write-Step "Python $PythonVersion"
# Not `python --version`: without Python, that finds the Microsoft Store's
# python.exe alias, which opens the Store. The py launcher comes with the
# python.org installer.
$foundPython = $null
if (Test-Command py) {
    $ErrorActionPreference = 'Continue'
    $foundPython = "$(& py "-$PythonVersion" --version 2>$null)".Trim()
    $ErrorActionPreference = 'Stop'
}
if ($foundPython -match '^Python \d') {
    Write-Host "Found $foundPython."
} else {
    # Python.Python.3.13 stays on 3.13 when upgraded, so no pin. --override
    # replaces winget's silent switches with these: for all users, on the
    # machine PATH (ahead of the user's WindowsApps aliases), with the launcher.
    Install-WingetPackage "Python.Python.$PythonVersion" @('--scope', 'machine', '--override',
        '/quiet InstallAllUsers=1 PrependPath=1 Include_launcher=1 InstallLauncherAllUsers=1 Include_test=0')
}

Write-Step 'WebView2 runtime'
$webView2 = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
$webView2Keys = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$webView2",
    "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$webView2")
$webView2Version = $webView2Keys |
    ForEach-Object { (Get-ItemProperty $_ -ErrorAction SilentlyContinue).pv } |
    Where-Object { $_ -and $_ -ne '0.0.0.0' } |
    Select-Object -First 1
if ($webView2Version) {
    Write-Host "Found WebView2 $webView2Version."
} else {
    Install-WingetPackage 'Microsoft.EdgeWebView2Runtime' @('--scope', 'machine')
}

Write-Step "vcpkg and OpenSSL ($VcpkgRoot)"
if (-not (Test-Path (Join-Path $VcpkgRoot '.git'))) {
    Invoke-Checked git @('clone', 'https://github.com/microsoft/vcpkg', $VcpkgRoot) | Out-Null
}
$vcpkg = Join-Path $VcpkgRoot 'vcpkg.exe'
if (-not (Test-Path $vcpkg)) {
    Invoke-Checked (Join-Path $VcpkgRoot 'bootstrap-vcpkg.bat') @('-disableMetrics') | Out-Null
}
Invoke-Checked $vcpkg @('install', "openssl:$OpenSslTriplet") | Out-Null
$opensslDir = Join-Path $VcpkgRoot "installed\$OpenSslTriplet"
$header = Get-Content (Join-Path $opensslDir 'include\openssl\opensslv.h') -Raw
if ($header -notmatch 'OPENSSL_VERSION_MAJOR\s+(\d+)' -or [int]$Matches[1] -lt 3) {
    throw "vcpkg installed an OpenSSL older than 3 in $opensslDir; HoploDex's license needs OpenSSL 3."
}
[Environment]::SetEnvironmentVariable('OPENSSL_DIR', $opensslDir, 'Machine')
[Environment]::SetEnvironmentVariable('OPENSSL_STATIC', '1', 'Machine')
Write-Host "OPENSSL_DIR=$opensslDir, OPENSSL_STATIC=1 (machine-wide)."

Write-Step 'Long paths'
Set-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' -Name LongPathsEnabled -Value 1 -Type DWord
Invoke-Checked git @('config', '--system', 'core.longpaths', 'true') | Out-Null

if ($GitHubCli) {
    Write-Step 'GitHub CLI'
    if (Test-Command gh) {
        Write-Host "Found $((gh --version)[0])."
    } else {
        Install-WingetPackage 'GitHub.cli' @('--scope', 'machine')
    }
}

Write-Step 'Remote Desktop'
$terminalServer = 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server'
Set-ItemProperty $terminalServer -Name fDenyTSConnections -Value 0 -Type DWord
# Network Level Authentication: sign in before a session is created.
Set-ItemProperty "$terminalServer\WinStations\RDP-Tcp" -Name UserAuthentication -Value 1 -Type DWord
# The "Remote Desktop" rule group, by an ID that doesn't depend on the language.
Enable-NetFirewallRule -Group '@FirewallAPI.dll,-28752'
Add-GroupMember -Description 'Remote Desktop Users' -Group @{ SID = $RemoteDesktopUsersSid } -Sid $UserSid

Write-Step 'OpenSSH server'
# Windows Server 2025 has sshd built in; elsewhere it's an optional capability.
if (-not (Get-Service sshd -ErrorAction SilentlyContinue)) {
    $capability = Get-WindowsCapability -Online -Name 'OpenSSH.Server*' | Select-Object -First 1
    if (-not $capability) { throw 'This Windows has no OpenSSH server to install.' }
    Add-WindowsCapability -Online -Name $capability.Name | Out-Null
}
Set-Service sshd -StartupType Automatic
Start-Service sshd
# Installing the capability normally adds this rule; make sure it's there and on.
$sshRule = Get-NetFirewallRule -Name 'OpenSSH-Server-In-TCP' -ErrorAction SilentlyContinue
if ($sshRule) {
    Enable-NetFirewallRule -Name 'OpenSSH-Server-In-TCP'
} else {
    New-NetFirewallRule -Name 'OpenSSH-Server-In-TCP' -DisplayName 'OpenSSH Server (sshd)' `
        -Enabled True -Direction Inbound -Protocol TCP -LocalPort 22 -Action Allow | Out-Null
}
if (-not (Get-LocalGroup -Name $OpenSshUsers -ErrorAction SilentlyContinue)) {
    New-LocalGroup -Name $OpenSshUsers -Description 'Members may sign in through OpenSSH.' | Out-Null
}
Add-GroupMember -Description $OpenSshUsers -Group @{ Name = $OpenSshUsers } -Sid $UserSid
# Only "OpenSSH Users" may sign in. Windows' sshd wants account names in lower
# case, and AllowGroups has to come before the first Match block, or it
# applies only inside it. sshd wrote the default config when it first started.
$sshdConfig = Join-Path $env:ProgramData 'ssh\sshd_config'
$allowGroups = "AllowGroups `"$($OpenSshUsers.ToLowerInvariant())`""
$current = @(Get-Content $sshdConfig)
$updated = New-Object System.Collections.Generic.List[string]
$inMatch = $false
foreach ($line in $current) {
    if (-not $inMatch -and $line -match '^\s*Match\s') {
        $updated.Add($allowGroups)
        $inMatch = $true
    }
    if (-not $inMatch -and $line -match '^\s*AllowGroups\s') { continue }
    $updated.Add($line)
}
if (-not $inMatch) { $updated.Add($allowGroups) }
if (($updated -join "`n") -ne ($current -join "`n")) {
    $backup = "$sshdConfig.before-hoplodex"
    if (-not (Test-Path $backup)) { Copy-Item $sshdConfig $backup }
    # Without a byte order mark, which sshd would read as part of the first line.
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [IO.File]::WriteAllLines($sshdConfig, [string[]]$updated, $utf8)
    $sshd = (Get-CimInstance Win32_Service -Filter "Name='sshd'").PathName.Trim('"')
    if ((Invoke-Checked $sshd @('-t') -OkCodes (0..255)) -ne 0) {
        [IO.File]::WriteAllLines($sshdConfig, [string[]]$current, $utf8)
        throw "sshd rejected the new $sshdConfig, so it's been put back."
    }
    Restart-Service sshd
    Write-Host "SSH is restricted to $OpenSshUsers (the old config is $backup)."
} else {
    Write-Host "SSH is already restricted to $OpenSshUsers."
}

$serverManager = Get-ScheduledTask -TaskName ServerManager -ErrorAction SilentlyContinue
if ($serverManager) {
    Write-Step 'Server Manager'
    $serverManager | Disable-ScheduledTask | Out-Null
    # The "Do not display Server Manager automatically at logon" policy.
    $policy = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Server\ServerManager'
    New-Item $policy -Force | Out-Null
    Set-ItemProperty $policy -Name DoNotOpenAtLogon -Value 1 -Type DWord
    Write-Host "Server Manager won't open at sign-in."
}

$script:SignInFirst = $false
if ($AutoLogon) {
    Write-Step "Auto-logon as $LogonDomain\$LogonName"
    Set-AutoLogonSecret $Password
    Set-ItemProperty $Winlogon -Name AutoAdminLogon -Value '1' -Type String
    Set-ItemProperty $Winlogon -Name DefaultUserName -Value $LogonName -Type String
    Set-ItemProperty $Winlogon -Name DefaultDomainName -Value $LogonDomain -Type String
    # A password here would win over the LSA secret, and a count would end
    # auto-logon after that many sign-ins.
    Remove-ItemProperty $Winlogon -Name DefaultPassword, AutoLogonCount -ErrorAction SilentlyContinue

    Write-Step 'No idle lock, screen saver, display timeout or sleep'
    foreach ($setting in 'monitor', 'standby', 'hibernate') {
        foreach ($power in 'ac', 'dc') {
            Invoke-Checked powercfg @('/change', "$setting-timeout-$power", '0') | Out-Null
        }
    }
    # "Require a password on wakeup".
    Invoke-Checked powercfg @('/setacvalueindex', 'SCHEME_CURRENT', 'SUB_NONE', 'CONSOLELOCK', '0') | Out-Null
    Invoke-Checked powercfg @('/setdcvalueindex', 'SCHEME_CURRENT', 'SUB_NONE', 'CONSOLELOCK', '0') | Out-Null
    Invoke-Checked powercfg @('/setactive', 'SCHEME_CURRENT') | Out-Null
    # "Interactive logon: Machine inactivity limit".
    Set-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System' `
        -Name InactivityTimeoutSecs -Value 0 -Type DWord
    # The screen saver is a per-user setting, so it's set as a policy in the
    # user's hive, which exists once they've signed in.
    $screenSaver = Set-UserRegistryValue -Key 'Software\Policies\Microsoft\Windows\Control Panel\Desktop' `
        -Values @{ ScreenSaveActive = '0'; ScreenSaverIsSecure = '0' }
    if (-not $screenSaver) { $script:SignInFirst = $true }

    Write-Step 'Windows animations on'
    # Windows Server starts with them off, and WebView2 then reports
    # prefers-reduced-motion, so the app skips its animations and the E2E
    # tests that check them fail.
    if (-not (Enable-UserAnimations)) { $script:SignInFirst = $true }
}

Write-Host ''
Write-Host "Machine-wide setup is done. Next, signed in as $User, open a new" -ForegroundColor Green
Write-Host 'PowerShell (not elevated) and run setup-user.ps1.' -ForegroundColor Green
if ($script:SignInFirst) {
    Write-Warning ("$User has never signed in, so their screen saver and animations can't be set yet. " +
        'Restart (auto-logon signs them in), then run this script again.')
} elseif ($AutoLogon) {
    Write-Host "Restart Windows, and it signs in as $User." -ForegroundColor Green
}
if ($script:RebootNeeded) {
    Write-Warning 'An installer asked for a restart. Restart Windows before building.'
}
