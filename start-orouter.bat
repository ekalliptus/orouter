@echo off
REM ============================================================
REM ORouter start: Node engine (20129) + Rust gateway (20130)
REM Open the dashboard at http://127.0.0.1:20130
REM ============================================================
start "ORouter Node engine" cmd /k "cd /d C:\dev\orouter && node custom-server.js --port 20129"
timeout /t 6 /nobreak >nul
start "ORouter Rust gateway" cmd /k "cd /d C:\dev\orouter\rust-backend && set NODE_UPSTREAM=http://127.0.0.1:20129&& target\debug\orouter-backend.exe"
echo.
echo   Dashboard : http://127.0.0.1:20130   (UI 9Router asli, gateway Rust)
echo   Node only : http://127.0.0.1:20129
echo.
echo Dua jendela terminal akan terbuka. Biarkan keduanya tetap jalan.
