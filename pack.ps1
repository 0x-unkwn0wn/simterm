# Empaqueta una campaña en un .exe autónomo con el contenido CIFRADO dentro.
#
# El binario resultante ignora `--campaign`: siempre juega la campaña embebida.
# La campaña (campaign.ron + assets: música, etc.) se comprime (zlib) y se cifra
# (ChaCha20-Poly1305) en tiempo de compilación por `build.rs`, y se incrusta con
# `include_bytes!`. Sin la clave no se puede leer el contenido del .exe.
#
# USO:
#   ./pack.ps1 -Campaign <ruta-campaña> [-Key <secreto>] [-Out <fichero.exe>] [-NoMusic] [-NoAutoplay]
#
# EJEMPLOS:
#   ./pack.ps1 -Campaign C:\campañas\sigint -Key "mi-secreto" -Out sigint.exe
#   ./pack.ps1 -Campaign .\examples\sample_campaign            # clave aleatoria
#   ./pack.ps1 -Campaign .\examples\sample_campaign -NoAutoplay
#
# NOTAS:
#   - Usa un directorio target aparte (target-pack/) para no tocar tu
#     `target/…/simterm.exe` normal de desarrollo.
#   - Si -Key se omite, `build.rs` deriva una clave del contenido + reloj: el
#     .exe funciona, pero la clave no es reproducible entre compilaciones.
#   - -NoMusic empaqueta una copia de la campaña SIN la carpeta music/ (útil si
#     los WAV pesan demasiado; el .exe queda pequeño y se juega en silencio).
#   - -NoAutoplay hornea en el .exe la desactivación del autoplay: el binario
#     ignora cualquier --autoplay* para que el jugador no pueda spoilear la
#     campaña (no se puede saltar por línea de comandos).

param(
    [Parameter(Mandatory = $true)] [string] $Campaign,
    [string] $Key = "",
    [string] $Out = "simterm-campaign.exe",
    [switch] $NoMusic,
    [switch] $NoAutoplay
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $MyInvocation.MyCommand.Path

if (-not (Test-Path -LiteralPath $Campaign -PathType Container)) {
    throw "La ruta de campaña no existe o no es un directorio: $Campaign"
}
$campaignPath = (Resolve-Path -LiteralPath $Campaign).Path

# Con -NoMusic se empaqueta una copia temporal sin la carpeta music/.
$tempCampaign = $null
if ($NoMusic) {
    $tempCampaign = Join-Path $env:TEMP ("simterm-pack-" + [System.Guid]::NewGuid().ToString("N"))
    Copy-Item -LiteralPath $campaignPath -Destination $tempCampaign -Recurse
    $music = Join-Path $tempCampaign "music"
    if (Test-Path -LiteralPath $music) { Remove-Item -LiteralPath $music -Recurse -Force }
    $campaignPath = $tempCampaign
    Write-Host "[pack] -NoMusic: empaquetando sin la carpeta music/." -ForegroundColor Yellow
}

try {
    $env:SIMTERM_EMBED_CAMPAIGN = $campaignPath
    $env:SIMTERM_EMBED_KEY = $Key
    if ($NoAutoplay) {
        $env:SIMTERM_DISABLE_AUTOPLAY = "1"
        Write-Host "[pack] -NoAutoplay: el autoplay quedará desactivado en el .exe." -ForegroundColor Yellow
    }
    $targetDir = Join-Path $repo "target-pack"

    Write-Host "[pack] Compilando (release) con la campaña embebida..." -ForegroundColor Cyan
    Write-Host "[pack] Campaña: $campaignPath"

    & cargo build --release -p simterm --features embed-campaign --target-dir $targetDir
    if ($LASTEXITCODE -ne 0) { throw "cargo build falló (código $LASTEXITCODE)" }

    $built = Join-Path $targetDir "release\simterm.exe"
    if (-not (Test-Path -LiteralPath $built)) { throw "No se encontró el binario compilado: $built" }

    Copy-Item -LiteralPath $built -Destination $Out -Force
    $size = "{0:N1} MB" -f ((Get-Item -LiteralPath $Out).Length / 1MB)
    Write-Host "[pack] Listo: $Out ($size)" -ForegroundColor Green
    Write-Host "[pack] Pruébalo con:  ./$Out"
}
finally {
    Remove-Item Env:\SIMTERM_EMBED_CAMPAIGN -ErrorAction SilentlyContinue
    Remove-Item Env:\SIMTERM_EMBED_KEY -ErrorAction SilentlyContinue
    Remove-Item Env:\SIMTERM_DISABLE_AUTOPLAY -ErrorAction SilentlyContinue
    if ($tempCampaign -and (Test-Path -LiteralPath $tempCampaign)) {
        Remove-Item -LiteralPath $tempCampaign -Recurse -Force -ErrorAction SilentlyContinue
    }
}
