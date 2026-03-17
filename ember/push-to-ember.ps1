# Push a message or structured payload to the Ember app via the server's push channel.
# The app receives it on the next poll and can rewrite content, structure, layout in real time.
#
# Usage:
#   .\push-to-ember.ps1 "Plain message"
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
    [string]$Host = "127.0.0.1",
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
    $client.Connect($Host, $Port)
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
    Write-Host "Failed to push: $_" -ForegroundColor Red
    Write-Host "Is ember-server running with push channel on ${Host}:${Port}?" -ForegroundColor Yellow
    exit 1
}
