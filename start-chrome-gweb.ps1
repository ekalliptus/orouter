# Launches the dedicated "Gemini Web" Chrome profile with remote debugging —
# the automation target for the gemini-web provider (argon on free tier).
# Sign in to the Google account once inside this window; the session is
# device-bound (DBSC) to this profile, so keep this window open while using
# gemini-web models. Safe to run alongside your normal Chrome.

Start-Process -FilePath "C:\Program Files\Google\Chrome\Application\chrome.exe" -ArgumentList `
  '--remote-debugging-port=9222',`
  '--user-data-dir="C:\Users\ekalliptus\AppData\Local\ChromeGWebProfile"',`
  '--no-first-run',`
  '--no-default-browser-check'
