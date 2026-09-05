// Capture the chat/completions request from the Freebuff TUI.
// The TUI ignores piped stdin because it opens /dev/tty equivalents.
// Workaround: use Windows SendKeys via PowerShell to type into the TUI window.

const http = require('http');
const https = require('https');
const fs = require('fs');
const net = require('net');
const os = require('os');
const path = require('path');
const { spawn, execSync } = require('child_process');

const keyPath = path.join(os.tmpdir(), 'fb-key.pem');
const certPath = path.join(os.tmpdir(), 'fb-cert.pem');
const PROXY_PORT = 18994;
const captured = [];

const fakeServer = https.createServer(
  { key: fs.readFileSync(keyPath), cert: fs.readFileSync(certPath) },
  (req, res) => {
    let body = '';
    req.on('data', (c) => (body += c));
    req.on('end', () => {
      const isRun = req.url.includes('agent-runs');
      const isChat = req.url.includes('chat/completions');
      if (isRun || isChat) {
        // Fixed allowlist of dump filenames — no dynamic path components.
        const dumpPath = isChat
          ? path.join(os.tmpdir(), 'fb-flow-chat.json')
          : path.join(os.tmpdir(), 'fb-flow-run.json');
        fs.writeFileSync(dumpPath, JSON.stringify({ url: req.url, headers: req.headers, body: body }, null, 1));
        console.log('\n>>> captured', req.method, req.url, '->', dumpPath);
      } else {
        console.log('[proxy]', req.method, req.url.slice(0, 80));
      }
      const fwd = https.request(
        {
          hostname: 'www.codebuff.com',
          port: 443,
          path: req.url,
          method: req.method,
          headers: { ...req.headers, host: 'www.codebuff.com' },
        },
        (fres) => {
          if (req.url.includes('chat/completions')) {
            console.log('\n>>> UPSTREAM STATUS for chat:', fres.statusCode);
            let respSample = '';
            fres.on('data', (c) => {
              if (respSample.length < 500) respSample += c.toString();
            });
            fres.on('end', () => console.log('>>> UPSTREAM RESP (500):', respSample.slice(0, 500)));
          }
          res.writeHead(fres.statusCode, fres.headers);
          fres.pipe(res);
        }
      );
      fwd.on('error', () => res.end());
      fwd.end(body);
    });
  }
);

const proxy = http.createServer((req, res) => res.end('ok'));
proxy.on('connect', (req, clientSocket, head) => {
  const [host, port] = req.url.split(':');
  if (!host.includes('codebuff.com')) {
    const real = net.connect(parseInt(port) || 443, host, () => {
      clientSocket.write('HTTP/1.1 200 Connection Established\r\n\r\n');
      real.write(head);
      clientSocket.pipe(real).pipe(clientSocket);
    });
    real.on('error', () => clientSocket.end());
    return;
  }
  clientSocket.write('HTTP/1.1 200 Connection Established\r\n\r\n');
  clientSocket.on('error', () => {});
  fakeServer.emit('connection', clientSocket);
});

fakeServer.listen(0, () => {
  proxy.listen(PROXY_PORT, () => {
    const workDir = path.join(os.tmpdir(), 'fbtest');
    fs.mkdirSync(workDir, { recursive: true });

    // Launch via cmd wrapper (freebuff is a .sh/.cmd shim, Start-Process fails on it)
    const ps = spawn('powershell.exe', [
      '-NoProfile',
      '-Command',
      [
        '$env:HTTPS_PROXY="http://127.0.0.1:' + PROXY_PORT + '"',
        '$env:HTTP_PROXY="http://127.0.0.1:' + PROXY_PORT + '"',
        '$env:NODE_TLS_REJECT_UNAUTHORIZED="0"',
        '$env:FREEBUFF_MITM="1"',
        '$cmd = (Get-Command freebuff.cmd -ErrorAction SilentlyContinue).Source; if (-not $cmd) { $cmd = "C:\\Users\\ekalliptus\\bin\\freebuff.cmd" }',
        '$p = Start-Process -FilePath "cmd.exe" -ArgumentList "/c", "`"$cmd`"" -WorkingDirectory "' + workDir + '" -PassThru -WindowStyle Normal',
        'Start-Sleep -Seconds 14',
        '$wshell = New-Object -ComObject wscript.shell',
        '$activated = $false',
        'for ($i = 0; $i -lt 10 -and -not $activated; $i++) { $activated = $wshell.AppActivate($p.Id); if (-not $activated) { Start-Sleep -Seconds 1 } }',
        'Start-Sleep -Milliseconds 800',
        '$wshell.SendKeys("say hello")',
        'Start-Sleep -Milliseconds 900',
        '$wshell.SendKeys("{ENTER}")',
        'Start-Sleep -Seconds 24',
        'Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue',
      ].join('; '),
    ], { stdio: 'inherit' });

    setTimeout(() => {
      console.log('\n=== DONE. captured:', captured.length, '===');
      proxy.close();
      fakeServer.close();
      process.exit(0);
    }, 75000);
  });
});
