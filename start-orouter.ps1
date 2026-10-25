# ORouter local production starter
# custom-server.js intentionally wraps the in-repo Next build (request-header
# sanitizing + background refresh), so "next start + standalone" is expected.
# __NEXT_PRIVATE_STANDALONE_CONFIG=1 silences the harmless standalone warning.
Set-Location C:\dev\orouter
$env:NEXT_PRIVATE_STANDALONE_CONFIG = "1"
# gemini-web provider attaches to the debug-profile Chrome on :9222 (see
# start-chrome-gweb.ps1). Without that Chrome, gemini-web requests fail but
# every other provider is unaffected.
$env:GEMINI_WEB_CDP_ENDPOINT = "http://127.0.0.1:9222"
node custom-server.js --port 20128
