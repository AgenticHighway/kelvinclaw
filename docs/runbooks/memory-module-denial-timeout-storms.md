# Runbook: Module Denial/Timeout Storms

## Symptoms

- spikes in `INVALID_ARGUMENT` (denied capabilities/claims mismatch).
- spikes in `DEADLINE_EXCEEDED` (module timeout/fuel exhaustion).
- increased request latency and elevated retry volume.

## Immediate Actions

1. Identify dominant `module_id`, `operation`, and deny reason from audit logs.
2. Rate-limit or temporarily disable the offending module.
3. Reduce caller retries to avoid amplification.
4. Increase telemetry sampling for the affected tenant/workspace/session scope.

## Containment

- tighten delegation limits (`timeout_ms`, `max_bytes`, `max_results`).
- lower module fuel/time budget if abuse pattern is clear.
- block module version and roll back to last known-good release.

## Recovery

1. Patch module and re-validate in staging.
2. Re-enable progressively with canary tenants.
3. Confirm latency and deny rates return to baseline.
4. Document root cause and preventive controls.
