@echo off
chcp 65001 >nul 2>&1
setlocal

:: 定位 reallyzip.exe：优先与脚本同目录，其次 dist\ 与 build\
set "EXE=%~dp0reallyzip.exe"
if not exist "%EXE%" set "EXE=%~dp0dist\reallyzip.exe"
if not exist "%EXE%" set "EXE=%~dp0build\reallyzip.exe"
if not exist "%EXE%" set "EXE=%~dp0target\release\reallyzip.exe"

if not exist "%EXE%" (
    echo [错误] 未找到 reallyzip.exe。
    echo 请将本脚本与 reallyzip.exe 放在同一目录下再运行。
    echo 也可从 Releases 页面下载 install.bat 与 reallyzip.exe 配套使用。
    pause
    exit /b 1
)

echo 正在注册 ReallyZip 右键菜单（写入当前用户注册表，无需管理员）...
"%EXE%" --register-shell
if errorlevel 1 (
    echo [失败] 注册未成功，请重试或检查杀毒软件拦截。
    pause
    exit /b 1
)

echo.
echo [完成] 右键菜单已注册。
echo 现在在 文件 / 文件夹 / .zip 压缩包 / 文件夹空白处 右键，
echo 即可看到「ReallyZip」级联子菜单。
echo.
pause
