# Push a message or structured payload to the Ember app via the server's push channel.
# The app receives it on the next poll and can rewrite content, structure, layout in real time.
#
# Usage:
#   .\push-to-ember.ps1 "Plain message"
#   .\push-to-ember.ps1 "app clear"   # reinitialize display (clear chat, rich content, error)
#   .\push-to-ember.ps1 "marquee"    # weather + gas marquee (uses last shared location)
#   .\push-to-ember.ps1 "qr"        # clear screen, show QR code in rich area (scan to download)
#   .\push-to-ember.ps1 -Payload '{"chat":[{"text":"Hi","isUser":true},{"text":"Hello!","isUser":false}],"rich":"<div>Dashboard</div>"}'
#   .\push-to-ember.ps1 -PayloadFile payload.json
#
# Structured payload (JSON):
#   chat: [{text, isUser}, ...] - replace entire chat
#   chatCss: CSS string
#   rich: HTML - rich content area
#   richStyle: CSS for rich area
#   layout: {rich_height: "full"|"auto"|"140", theme: "dark"|"light"}
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
    $url = if ($QrUrl) { $QrUrl } else { "https://github.com/Promethos-ai/blaster2000/releases/download/ember-v0.1.28/promqr.png" }
    $richHtml = "<div class=""rich-card"" style=""text-align:center;padding:24px""><img src=""$url"" style=""max-width:100%;max-height:400px;"" alt=""Scan to download"" /></div>"
    $escaped = $richHtml.Replace('\', '\\').Replace('"', '\"')
    $toSend = '{"chat":[],"rich":"' + $escaped + '"}'
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
