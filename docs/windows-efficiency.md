# Windows Efficiency mode

This reference describes the process policy used by the Windows builds. It is a scheduling request, not a measured promise about energy savings.

The application calls `SetProcessInformation` with `ProcessPowerThrottling`, sets the execution-speed control/state bits and selects `IDLE_PRIORITY_CLASS`. It changes only its own process. Unrelated power-throttling flags remain intact. If the priority update fails, the previous throttling state is restored. Unsupported API calls do not prevent startup.

Windows 11 interprets the execution-speed request as EcoQoS. Task Manager owns the leaf indicator; application code does not fabricate it. Other operating systems keep their normal scheduling policy.

## Verify a running process

Use `scripts/check-windows-efficiency.ps1 -ProcessId 1234`, replacing the example ID with the application's PID. A successful enabled state reports `eco_qos: true`, `idle_priority: true` and priority class `64`. The verifier opens a query-only process handle and does not change the process.

## Opt out

Set `CODEX_PRESENCE_EFFICIENCY_MODE=0` before launching the application. `false`, `off` and `disabled` are also accepted. The normal default requests Efficiency mode. This does not modify the machine's power plan or other applications.

## Sources

- [Microsoft: Quality of Service](https://learn.microsoft.com/en-us/windows/win32/procthread/quality-of-service)
- [Microsoft: SetProcessInformation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setprocessinformation)

The implementation uses the installed Windows SDK's 12-byte `PROCESS_POWER_THROTTLING_STATE` layout. Local tests verify the ABI constants and option parsing; process readback verifies the applied policy.
