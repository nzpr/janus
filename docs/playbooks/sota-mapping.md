# Process Mapping to Project Structure

This maps Jim process practices to concrete project artifacts.

## Mapping

1. Research -> Plan -> Implement workflow
   - `docs/playbooks/implementation-loop.md`

2. Decision preservation / handoff continuity
   - `docs/adr/`
   - `docs/decisions/`
   - `skill/decision-memory/decision-memory.skill.yaml`

3. Audited evolution
   - `evolution/events/`
   - `evolution/AUDIT-TRAIL.md`
   - `evolution/CHANGELOG.md`
   - `evolution/SCORECARD.md`

4. Optional baseline snapshots
   - `docs/baseline/` (if the project uses baseline snapshots)

5. Prompt and process constraints over time
   - `AGENTS.md`
   - `docs/playbooks/implementation-loop.md`
   - `docs/playbooks/talk-only-mode.md`

6. Reusable automation and consistency
   - `scripts/audit/new-adr.sh`
   - `scripts/audit/new-decision.sh`
   - `scripts/audit/log-evolution.sh`

## Expected Evolution Flow

1. Start task -> write plan.
2. Implement and validate.
3. Record task decision.
4. Record evolution event with evidence.
5. Promote repeated patterns to ADR or playbook updates.
