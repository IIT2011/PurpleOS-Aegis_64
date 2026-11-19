param(
    [Parameter(Mandatory = $true)] [string] $FirmwareCode,
    [Parameter(Mandatory = $true)] [string] $FirmwareVars,
    [string] $Disk = 'build/aegis64-uefi.img',
    [switch] $Headless,
    [switch] $WritableDisk
)

$ErrorActionPreference = 'Stop'
$code = (Resolve-Path -LiteralPath $FirmwareCode).Path
$template = (Resolve-Path -LiteralPath $FirmwareVars).Path
Push-Location -LiteralPath $PSScriptRoot
try {
    $image = (Resolve-Path -LiteralPath $Disk).Path
    $scratch = (New-Item -ItemType Directory -Path (Join-Path 'target' ('uefi-' + [guid]::NewGuid())) -Force).FullName
    $vars = Join-Path $scratch 'vars.fd'
    Copy-Item -LiteralPath $template -Destination $vars
    $display = if ($Headless) { 'none' } else { 'sdl,show-cursor=off' }
    $snapshot = if ($WritableDisk) { 'off' } else { 'on' }

    qemu-system-x86_64 -machine q35,smm=on -m 256M -smp 4 `
        -global driver=cfi.pflash01,property=secure,value=on `
        -drive "if=pflash,format=raw,readonly=on,file=$code" `
        -drive "if=pflash,format=raw,file=$vars" `
        -device piix3-ide,id=aegis-ata `
        -drive "if=none,id=aegis-system,format=raw,snapshot=$snapshot,file=$image" `
        -device ide-hd,drive=aegis-system,bus=aegis-ata.0,bootindex=1 `
        -display $display -serial stdio -nic none
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
