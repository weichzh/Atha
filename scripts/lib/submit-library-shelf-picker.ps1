# Drive only the file picker owned by the isolated Atha gate process.
[CmdletBinding()]
param([Parameter(Mandatory)][int]$ProcessId)

$ErrorActionPreference = 'Stop'
$selectedBookPath = $env:ATHA_SHELF_GATE_PICKER_BOOK
if ([string]::IsNullOrWhiteSpace($selectedBookPath)) { throw 'The gate picker requires a local book path.' }

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

namespace Atha {
public static class NativePicker {
    [DllImport("user32.dll", EntryPoint = "SendMessageW", CharSet = CharSet.Unicode)]
    public static extern IntPtr SetText(IntPtr window, uint message, IntPtr parameter, string value);

    [DllImport("user32.dll", EntryPoint = "SendMessageW")]
    public static extern IntPtr Click(IntPtr window, uint message, IntPtr parameter, IntPtr value);
}
}
'@
$deadline = [DateTime]::UtcNow.AddSeconds(20)
$processCondition = [System.Windows.Automation.PropertyCondition]::new(
    [System.Windows.Automation.AutomationElement]::ProcessIdProperty,
    $ProcessId
)
do {
    $windows = [System.Windows.Automation.AutomationElement]::RootElement.FindAll(
        [System.Windows.Automation.TreeScope]::Children,
        $processCondition
    )
    foreach ($window in $windows) {
        $fileName = $window.FindFirst(
            [System.Windows.Automation.TreeScope]::Descendants,
            [System.Windows.Automation.AndCondition]::new(
                [System.Windows.Automation.PropertyCondition]::new(
                    [System.Windows.Automation.AutomationElement]::AutomationIdProperty,
                    '1148'
                ),
                [System.Windows.Automation.PropertyCondition]::new(
                    [System.Windows.Automation.AutomationElement]::ClassNameProperty,
                    'Edit'
                )
            )
        )
        $openButton = $window.FindFirst(
            [System.Windows.Automation.TreeScope]::Descendants,
            [System.Windows.Automation.AndCondition]::new(
                [System.Windows.Automation.PropertyCondition]::new(
                    [System.Windows.Automation.AutomationElement]::AutomationIdProperty,
                    '1'
                ),
                [System.Windows.Automation.PropertyCondition]::new(
                    [System.Windows.Automation.AutomationElement]::ClassNameProperty,
                    'Button'
                )
            )
        )
        if ($null -eq $fileName -or $null -eq $openButton) { continue }

        try {
            $window.SetFocus()
            $textSet = [Atha.NativePicker]::SetText(
                [IntPtr]$fileName.Current.NativeWindowHandle,
                0x000C,
                [IntPtr]::Zero,
                $SelectedBookPath
            )
            if ($textSet -eq [IntPtr]::Zero) { continue }
            [void][Atha.NativePicker]::Click(
                [IntPtr]$openButton.Current.NativeWindowHandle,
                0x00F5,
                [IntPtr]::Zero,
                [IntPtr]::Zero
            )
            return
        }
        catch {
            continue
        }
    }
    Start-Sleep -Milliseconds 100
} while ([DateTime]::UtcNow -lt $deadline)

throw 'Windows book picker could not be completed.'
