@echo off
REM ============================================================
REM ORouter FULL NATIVE: satu proses Rust (port 20130)
REM UI Vue dibundel di dalam rust-backend (vue-web/dist).
REM Buka dashboard di http://127.0.0.1:20130
REM ============================================================
cd /d C:\dev\orouter\rust-backend
echo.
echo   Dashboard : http://127.0.0.1:20130   (full native Rust + Vue)
echo   Tekan Ctrl+C untuk berhenti.
echo.
target\debug\orouter-backend.exe
