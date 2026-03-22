# Push a message or structured payload to the Ember app via the server's push channel.
# The app receives it on the next poll and can rewrite content, structure, layout in real time.
#
# Usage:
#   .\push-to-ember.ps1 "Plain message"
#   .\push-to-ember.ps1 "app clear"   # reinitialize display (clear chat, rich content, error)
#   .\push-to-ember.ps1 "marquee"    # weather + gas marquee (uses last shared location)
#   .\push-to-ember.ps1 "nanoose"   # Nanoose Bay 7-day forecast bar (graphical)
#   .\push-to-ember.ps1 "qr"        # clear screen, show QR code in rich area (scan to download)
#   .\push-to-ember.ps1 "style"    # push server chat-style.css to app (reload ChatWebView styles at will)
#   .\push-to-ember.ps1 "rich"     # push server rich-placeholder.html to app (reload rich area)
#   .\push-to-ember.ps1 -Payload '{"chat":[{"text":"Hi","isUser":true},{"text":"Hello!","isUser":false}],"rich":"<div>Dashboard</div>"}'
#   .\push-to-ember.ps1 -PayloadFile payload.json
#
# Structured payload (JSON):
#   chat: [{text, isUser}, ...] - replace entire chat
#   chatCss: CSS string
#   rich: HTML - rich content area
#   richStyle: CSS for rich area
#   layout: {rich_height, theme, inference_timeout_sec} - inference_timeout_sec: live-tune response timeout (default 120)
#   input: prefill prompt
#   message: append as AI message
#
# Requires: ember-server running with --push-port (default 4434)

param(
    [Parameter(Position=0)]
    [string]$Message = "",
    [string]$Payload = "",
    [string]$PayloadFile = "",
    [string]$QrUrl = "",
    [string]$PushHost = "127.0.0.1",
    [int]$Port = 4434
)

$toSend = ""
if ($PayloadFile -ne "") {
    if (Test-Path $PayloadFile) {
        $toSend = Get-Content $PayloadFile -Raw
    } else {
        Write-Host "Payload file not found: $PayloadFile" -ForegroundColor Red
        exit 1
    }
} elseif ($Payload -ne "") {
    $toSend = $Payload
} elseif ($Message -eq "qr") {
    $url = if ($QrUrl) { $QrUrl } else { "https://github.com/Promethos-ai/blaster2000/releases/download/ember-v0.1.29/promqr.png" }
    $richHtml = "<div class=""rich-card"" style=""text-align:center;padding:24px""><img src=""$url"" style=""max-width:100%;max-height:400px;"" alt=""Scan to download"" /></div>"
    $escaped = $richHtml.Replace('\', '\\').Replace('"', '\"')
    $toSend = '{"chat":[],"rich":"' + $escaped + '"}'
} elseif ($Message -eq "style") {
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
    $stylePath = Join-Path $scriptDir "server\chat-style.css"
    if (Test-Path $stylePath) {
        $css = [System.IO.File]::ReadAllText($stylePath)
        $escaped = $css.Replace('\', '\\').Replace('"', '\"').Replace("`r`n", '\n').Replace("`r", '\n').Replace("`n", '\n')
        $toSend = '{"chatCss":"' + $escaped + '"}'
    } else {
        Write-Host "Style file not found: $stylePath" -ForegroundColor Red
        exit 1
    }
} elseif ($Message -eq "rich") {
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
    $richPath = Join-Path $scriptDir "server\rich-placeholder.html"
    if (Test-Path $richPath) {
        $html = [System.IO.File]::ReadAllText($richPath)
        $escaped = $html.Replace('\', '\\').Replace('"', '\"').Replace("`r`n", '\n').Replace("`r", '\n').Replace("`n", '\n')
        $toSend = '{"rich":"' + $escaped + '"}'
    } else {
        Write-Host "Rich placeholder not found: $richPath" -ForegroundColor Red
        exit 1
    }
} elseif ($Message -eq "nanoose") {
    try {
        $url = "https://api.open-meteo.com/v1/forecast?latitude=49.27&longitude=-124.16&daily=temperature_2m_max,temperature_2m_min,weather_code,precipitation_sum&timezone=America/Vancouver"
        $data = Invoke-RestMethod -Uri $url -TimeoutSec 15
        $times = $data.daily.time
        $maxT = $data.daily.temperature_2m_max
        $minT = $data.daily.temperature_2m_min
        $codes = $data.daily.weather_code
        $precip = $data.daily.precipitation_sum
        $dayNames = @("Sun","Mon","Tue","Wed","Thu","Fri","Sat")
        function Get-WxIcon($c) {
            if ($c -le 2) { return "&#9728;" }; if ($c -eq 3) { return "&#9729;" }
            if ($c -in 45,48) { return "&#127787;" }; if ($c -ge 51 -and $c -le 67) { return "&#9748;" }
            if ($c -ge 71 -and $c -le 77) { return "&#10052;" }; if ($c -ge 80 -and $c -le 82) { return "&#9928;" }
            if ($c -ge 95 -and $c -le 99) { return "&#9928;" }; return "&#9729;"
        }
        $bars = @()
        for ($i = 0; $i -lt [Math]::Min(7, $times.Count); $i++) {
            $d = [DateTime]::Parse($times[$i])
            $day = $dayNames[$d.DayOfWeek]
            $icon = Get-WxIcon $codes[$i]
            $hiF = [int]([double]$maxT[$i] * 9/5 + 32)
            $loF = [int]([double]$minT[$i] * 9/5 + 32)
            $p = [double]$precip[$i]
            $pVal = [math]::Round($p, 1)
            $pStr = if ($p -gt 0) { " <span style=""font-size:10px;color:#94a3b8"">" + $pVal + "mm</span>" } else { "" }
            $bars += "<div class=""wx-day"" style=""flex:1;text-align:center;padding:8px 4px;background:rgba(255,255,255,0.04);border-radius:6px;margin:0 2px""><div style=""font-size:11px;color:#94a3b8"">$day</div><div style=""font-size:18px;margin:4px 0"">$icon</div><div style=""font-size:13px;color:#e6edf3"">$hiF°</div><div style=""font-size:11px;color:#64748b"">$loF°</div>$pStr</div>"
        }
        $barHtml = $bars -join ""
        $richHtml = "<div class=""rich-card"" style=""padding:12px""><div style=""font-size:12px;color:#94a3b8;margin-bottom:10px"">Nanoose Bay · 7-day forecast</div><div style=""display:flex;flex-wrap:wrap;gap:4px;align-items:stretch"">$barHtml</div></div>"
        $escaped = $richHtml.Replace('\', '\\').Replace('"', '\"').Replace("`r`n", '\n').Replace("`r", '\n').Replace("`n", '\n')
        $toSend = '{"rich":"' + $escaped + '","layout":{"rich_height":"auto"}}'
    } catch {
        Write-Host "Nanoose weather fetch failed: $_" -ForegroundColor Red
        exit 1
    }
} elseif ($Message -ne "") {
    $toSend = $Message
}

if ($toSend -eq "") {
    Write-Host "Usage: .\push-to-ember.ps1 'Your message here'" -ForegroundColor Yellow
    Write-Host "       .\push-to-ember.ps1 -Payload '{\"chat\":[...],\"rich\":\"<div>...</div>\"}'" -ForegroundColor Gray
    Write-Host "       .\push-to-ember.ps1 -PayloadFile payload.json" -ForegroundColor Gray
    exit 1
}

try {
    $client = New-Object System.Net.Sockets.TcpClient
    $client.Connect($PushHost, $Port)
    $stream = $client.GetStream()
    $writer = New-Object System.IO.StreamWriter($stream)
    $writer.AutoFlush = $true
    $writer.WriteLine($toSend)
    $writer.Close()
    $stream.Close()
    $client.Close()
    $preview = if ($toSend.Length -gt 60) { $toSend.Substring(0, 60) + "..." } else { $toSend }
    Write-Host "Pushed ($($toSend.Length) chars): $preview" -ForegroundColor Green
} catch {
    # Fallback: write to push-queue.txt (server polls this when TCP push channel unavailable)
    $pushFile = "push-queue.txt"
    if (Test-Path $pushFile) {
        $existing = Get-Content $pushFile -Raw
        Set-Content $pushFile -Value "$existing$toSend`n" -NoNewline
    } else {
        Set-Content $pushFile -Value "$toSend`n"
    }
    $preview = if ($toSend.Length -gt 60) { $toSend.Substring(0, 60) + "..." } else { $toSend }
    Write-Host "TCP push failed; wrote to $pushFile (server polls every 1s): $preview" -ForegroundColor Yellow
}
