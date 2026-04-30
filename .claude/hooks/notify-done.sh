#!/bin/bash
# Stop/Notification — Windows 데스크톱 토스트 알림

MESSAGE="zm-mux: Claude Code 작업 완료"

# Windows PowerShell 토스트
powershell.exe -NoProfile -Command "
  Add-Type -AssemblyName System.Windows.Forms
  \$notify = New-Object System.Windows.Forms.NotifyIcon
  \$notify.Icon = [System.Drawing.SystemIcons]::Information
  \$notify.Visible = \$true
  \$notify.ShowBalloonTip(5000, 'Claude Code', '$MESSAGE', [System.Windows.Forms.ToolTipIcon]::Info)
  [System.Media.SystemSounds]::Asterisk.Play()
  Start-Sleep -Seconds 3
  \$notify.Dispose()
" > /dev/null 2>&1 &

exit 0
