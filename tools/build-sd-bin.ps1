<#
.SYNOPSIS
    Build an EdgeTX SD bootloader binary with 32 KiB padding on Windows PowerShell.
.DESCRIPTION
    Links the application at 0x08008000 and prepends the 32 KiB (32,768-byte)
    padding required by EdgeTX Bootloader's 'Write Firmware' validation routine.
.EXAMPLE
    .\tools\build-sd-bin.ps1 zorro
    .\tools\build-sd-bin.ps1 pocket
#>
[CmdletBinding()]
param (
    [Parameter(Mandatory=$true, Position=0)]
    [string]$Board,
    [Parameter(Position=1)]
    [string]$OutFile
)

$ValidBoards = @("pocket", "boxer", "zorro", "tx12", "qx7", "tprov2", "t12", "x9d", "x9e", "at9spro", "gx12")

if ($ValidBoards -notcontains $Board.ToLower()) {
    Write-Error "Error: unsupported board '$Board'. Valid boards: $($ValidBoards -join ', ')"
    exit 1
}

$Board = $Board.ToLower()
if (-not $OutFile) {
    $OutFile = "ferrox-$Board-sd.bin"
}

Write-Host "Building ferrox-$Board (edgetx-bootloader @ 0x08008000)..."

$tempApp = [System.IO.Path]::GetTempFileName()

try {
    & cargo objcopy `
        -p ferrox-pocket `
        --bin ferrox-pocket `
        --release `
        --target thumbv7em-none-eabihf `
        --no-default-features `
        --features "$Board,edgetx-bootloader" `
        -- -O binary $tempApp

    if ($LASTEXITCODE -ne 0) {
        Write-Error "cargo objcopy failed with exit code $LASTEXITCODE"
        exit $LASTEXITCODE
    }

    $appBytes = [System.IO.File]::ReadAllBytes($tempApp)
    $padSize = 32768
    $totalSize = $padSize + $appBytes.Length
    $outBytes = New-Object byte[] $totalSize

    for ($i = 0; $i -lt $padSize; $i++) {
        $outBytes[$i] = 0xFF
    }

    [System.Array]::Copy($appBytes, 0, $outBytes, $padSize, $appBytes.Length)
    [System.IO.File]::WriteAllBytes($OutFile, $outBytes)

    Write-Host "Created $OutFile : $totalSize bytes ($padSize B bootloader pad + $($appBytes.Length) B app)"
}
finally {
    if (Test-Path $tempApp) {
        Remove-Item -Force $tempApp
    }
}
