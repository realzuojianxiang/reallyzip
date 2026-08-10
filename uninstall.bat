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
    pause
    exit /b 1
)

echo 正在移除 ReallyZip 右键菜单...
"%EXE%" --unregister-shell
if errorlevel 1 (
    echo [失败] 移除未成功，请重试。
    pause
    exit /b 1
)

echo.
echo [完成] 右键菜单已移除，资源管理器不再显示「ReallyZip」。
echo.
pause
