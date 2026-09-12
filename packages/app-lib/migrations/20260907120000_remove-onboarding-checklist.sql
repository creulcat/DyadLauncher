-- The onboarding checklist could never actually complete: it required
-- has_logged_into_modrinth, but Dyad Launcher no longer supports signing
-- into a Modrinth account (see the remove-modrinth-account migration), so
-- that column could never become TRUE and the checklist stayed stuck
-- forever. Rather than patch the condition, drop the checklist entirely
-- and always show the sidebar's account section as-is.
DROP TABLE onboarding_checklist;
