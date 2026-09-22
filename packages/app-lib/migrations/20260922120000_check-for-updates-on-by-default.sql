-- The launch-time/hourly version check (goal 4) always ran unconditionally until now, with the
-- check_for_updates column left unused and OFF by default from when it briefly gated an in-place
-- self-updater that was later dropped (see 20260914120000). Goal 7 turns it into a real off switch,
-- decided on by default -- so it should preserve the behavior every existing install already had
-- (checks running) rather than silently going quiet for everyone on upgrade.
UPDATE settings SET check_for_updates = TRUE;
