#Requires -Version 7
<#
.SYNOPSIS
    Compila o Maré em modo release, copia para dist\Mare.exe e assina com o certificado da Certum.

.DESCRIPTION
    Antes de rodar, deixe o SimplySign Desktop conectado (o certificado da Certum precisa
    aparecer em "Certificados - Usuário atual > Pessoal"). O script acha sozinho o certificado
    "Open Source Developer" que tem chave privada e ainda está válido.

    A assinatura leva carimbo de data/hora da Certum, para continuar válida mesmo depois que
    o certificado vencer.

.PARAMETER Thumbprint
    Impressão digital (SHA-1) de um certificado específico, se houver mais de um.

.PARAMETER Exe
    Assina este executável em vez de compilar (ele é copiado para dist\Mare.exe).

.PARAMETER Teste
    Aceita um certificado de teste, que o Windows não reconhece. Serve só para conferir
    o fluxo; um executável assinado assim NÃO passa no Smart App Control.

.EXAMPLE
    .\scripts\compilar-e-assinar.ps1
#>
param(
    [string]$Thumbprint,
    [string]$Exe,
    [switch]$Teste,
    [string]$Carimbo = "http://time.certum.pl"
)

$ErrorActionPreference = "Stop"
$raiz = Split-Path -Parent $PSScriptRoot
Set-Location $raiz

function Passo($texto) { Write-Host "`n==> $texto" -ForegroundColor Cyan }
function Falha($texto) { Write-Host "`nERRO: $texto" -ForegroundColor Red; exit 1 }

# 1. Compilar -----------------------------------------------------------------
if (-not $Exe) {
    Passo "Compilando (release)..."
    cargo build --release
    if ($LASTEXITCODE -ne 0) { Falha "a compilação falhou." }
    $meta = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
    $Exe = Join-Path $meta.target_directory "release\mare.exe"
}
if (-not (Test-Path $Exe)) { Falha "não achei o executável $Exe." }

New-Item -ItemType Directory -Force (Join-Path $raiz "dist") | Out-Null
$saida = Join-Path $raiz "dist\Mare.exe"
if ((Resolve-Path $Exe).Path -ne $saida) { Copy-Item $Exe $saida -Force }
Write-Host "Executável: $saida"

# 2. Achar o signtool ---------------------------------------------------------
Passo "Procurando o signtool..."
$kits = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
$signtool = Get-ChildItem $kits -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match '\\x64\\' } |
    Sort-Object { [version]($_.Directory.Parent.Name -replace '[^\d\.]', '') } -Descending |
    Select-Object -First 1
if (-not $signtool) {
    Falha "signtool.exe não encontrado. Instale o 'Windows SDK' (componente 'Signing Tools')."
}
Write-Host $signtool.FullName

# 3. Escolher o certificado ---------------------------------------------------
Passo "Procurando o certificado de assinatura de código..."
$certs = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
    Where-Object { $_.HasPrivateKey -and $_.NotAfter -gt (Get-Date) }
if ($Thumbprint) {
    $cert = $certs | Where-Object Thumbprint -eq ($Thumbprint -replace '\s', '').ToUpper()
} else {
    $cert = $certs | Where-Object Subject -match 'Open Source Developer' |
        Sort-Object NotAfter -Descending | Select-Object -First 1
}
if (-not $cert) {
    Falha ("nenhum certificado da Certum disponível. Abra o SimplySign Desktop, " +
           "conecte com o código do app SimplySign no celular e rode de novo.")
}
Write-Host "Certificado: $($cert.Subject)"
Write-Host "Válido até:  $($cert.NotAfter.ToString('dd/MM/yyyy'))"

# 4. Assinar -------------------------------------------------------------------
Passo "Assinando (com carimbo de $Carimbo)..."
$argumentos = @(
    "sign", "/fd", "sha256", "/sha1", $cert.Thumbprint,
    "/tr", $Carimbo, "/td", "sha256",
    "/d", "Maré", "/du", "https://github.com/neonio-it/mare-torrent",
    $saida
)
& $signtool.FullName @argumentos
if ($LASTEXITCODE -ne 0) { Falha "o signtool não conseguiu assinar (código $LASTEXITCODE)." }

# 5. Conferir -----------------------------------------------------------------
Passo "Conferindo a assinatura..."
$sig = Get-AuthenticodeSignature $saida
Write-Host "Situação:   $($sig.Status)"
Write-Host "Assinante:  $($sig.SignerCertificate.Subject)"
if ($sig.TimeStamperCertificate) {
    Write-Host "Carimbo:    $($sig.TimeStamperCertificate.Subject)"
} else {
    Write-Host "Carimbo:    (nenhum)" -ForegroundColor Yellow
}
Write-Host "SHA-256:    $((Get-FileHash $saida -Algorithm SHA256).Hash)"

& $signtool.FullName verify /pa /q $saida *> $null
$cadeiaOk = ($LASTEXITCODE -eq 0)

if ($sig.Status -eq "Valid" -and $cadeiaOk -and $sig.TimeStamperCertificate) {
    Write-Host "`nPronto: dist\Mare.exe assinado, com carimbo, e verificado pelo Windows." -ForegroundColor Green
} elseif ($Teste) {
    Write-Host ("`nAssinado com certificado de teste. O Windows não confia nele (esperado); " +
                "isso só prova que o fluxo funciona.") -ForegroundColor Yellow
} else {
    Falha "a assinatura não foi validada pelo Windows: $($sig.StatusMessage)"
}
