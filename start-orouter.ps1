# ORouter local production starter
# custom-server.js intentionally wraps the in-repo Next build (request-header
# sanitizing + background refresh), so "next start + standalone" is expected.
# __NEXT_PRIVATE_STANDALONE_CONFIG=1 silences the harmless standalone warning.
Set-Location C:\dev\orouter
$env:NEXT_PRIVATE_STANDALONE_CONFIG = "1"
node custom-server.js --port 20128
