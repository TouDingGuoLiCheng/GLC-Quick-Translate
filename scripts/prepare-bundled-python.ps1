# 下载 Windows 嵌入式 Python，仅安装取词所需 pyperclip，输出到 src-tauri/bundled-python
# 供 Tauri 打包为安装目录下的 python/ 文件夹（Rust 已用 --clipboard-only，无需 pyautogui 等）
param(
    [string]$PythonVersion = "3.12.10",
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path $PSScriptRoot -Parent
$OutDir = Join-Path $ProjectRoot "src-tauri\bundled-python"
$Marker = Join-Path $OutDir ".bundle-ready"

if ((Test-Path $Marker) -and -not $Force) {
    Write-Host "bundled-python ready (use -Force to rebuild)"
    exit 0
}

if ($Force -and (Test-Path $OutDir)) {
    Remove-Item -Recurse -Force $OutDir
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$ZipName = "python-$PythonVersion-embed-amd64.zip"
$ZipUrl = "https://www.python.org/ftp/python/$PythonVersion/$ZipName"
$ZipPath = Join-Path $env:TEMP $ZipName

if (-not (Test-Path $ZipPath)) {
    Write-Host "Downloading $ZipUrl ..."
    Invoke-WebRequest -Uri $ZipUrl -OutFile $ZipPath -UseBasicParsing
}

Write-Host "Extracting to $OutDir ..."
Expand-Archive -Path $ZipPath -DestinationPath $OutDir -Force

$PthFile = Get-ChildItem -Path $OutDir -Filter "*._pth" | Select-Object -First 1
if (-not $PthFile) {
    throw "python*._pth not found"
}
$SiteDir = Join-Path $OutDir "Lib\site-packages"
New-Item -ItemType Directory -Force -Path $SiteDir | Out-Null

$PthBase = Get-Content $PthFile.FullName | Where-Object { $_ -match '\.zip$' -or $_ -eq '.' } | Select-Object -First 2
if ($PthBase.Count -lt 2) {
    $zipLine = (Get-ChildItem $OutDir -Filter "python*.zip" | Select-Object -First 1).Name
    $PthBase = @($zipLine, ".")
}
$PthLines = $PthBase + @("Lib\site-packages", "", "import site")
Set-Content -Path $PthFile.FullName -Value $PthLines -Encoding ascii

$PythonExe = Join-Path $OutDir "python.exe"
$PythonwExe = Join-Path $OutDir "pythonw.exe"
if (-not (Test-Path $PythonExe)) {
    throw "python.exe not found after extract"
}
if (-not (Test-Path $PythonwExe)) {
    throw "pythonw.exe not found in embed bundle (required for headless capture)"
}
Write-Host "Using python.exe + pythonw.exe from embed bundle"

$GetPip = Join-Path $env:TEMP "get-pip.py"
if (-not (Test-Path $GetPip)) {
    Write-Host "Downloading get-pip.py ..."
    Invoke-WebRequest -Uri "https://bootstrap.pypa.io/get-pip.py" -OutFile $GetPip -UseBasicParsing
}

Write-Host "Installing pip (build only)..."
& $PythonExe $GetPip --no-warn-script-location
if ($LASTEXITCODE -ne 0) { throw "get-pip failed" }

Write-Host "Installing pyperclip + pyautogui (capture hotkey)..."
& $PythonExe -m pip install --no-warn-script-location "pyperclip>=1.8.2" "pyautogui>=0.9.54"
if ($LASTEXITCODE -ne 0) { throw "pip install failed" }

Write-Host "Verify imports..."
& $PythonExe -c "import pyperclip, pyautogui; print('ok', pyperclip.__version__)"
if ($LASTEXITCODE -ne 0) { throw "import verify failed" }

Write-Host "Pruning pip/setuptools/wheel..."
$Site = Join-Path $OutDir "Lib\site-packages"
$PruneNames = @(
    "pip", "pip-*",
    "setuptools", "setuptools-*",
    "wheel", "wheel-*",
    "_distutils_hack",
    "pkg_resources", "pkg_resources-*",
    "distutils-precedence.pth"
)
foreach ($pattern in $PruneNames) {
    Get-ChildItem -Path $Site -Filter $pattern -ErrorAction SilentlyContinue | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}
Get-ChildItem -Path $Site -Recurse -Directory -Filter "__pycache__" -ErrorAction SilentlyContinue |
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue

$ScriptsDir = Join-Path $OutDir "Scripts"
if (Test-Path $ScriptsDir) {
    Remove-Item -Recurse -Force $ScriptsDir
}

$SizeMb = [math]::Round((Get-ChildItem $OutDir -Recurse -File | Measure-Object Length -Sum).Sum / 1MB, 1)
Write-Host "Bundle size about ${SizeMb} MB (embed Python + pyperclip)"

$MarkerContent = @"
version=$PythonVersion
packages=pyperclip,pyautogui
built=$(Get-Date -Format o)
"@
Set-Content -Path $Marker -Value $MarkerContent -Encoding utf8

Write-Host "Done: $OutDir"
