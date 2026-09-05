-- Dyad Launcher does not support signing into a Modrinth account (the
-- sign-in flow, shared instances, friends, and Modrinth Servers
-- hosting/billing features have all been removed), so the stored
-- session table no longer has any use. Drop it entirely.
DROP INDEX IF EXISTS modrinth_users_active;
DROP TABLE modrinth_users;
