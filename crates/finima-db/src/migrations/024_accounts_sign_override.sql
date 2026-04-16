-- Migration 024: Per-account user-set sign-convention override.
--
-- See ADR-018. End users set this via the Account detail page in the UI
-- ("Flip this account" button). When non-NULL, this column takes
-- precedence over the YAML institution rule and over autodetection,
-- making it the highest-precedence input to SignNormalizer.
--
-- The PUT /api/accounts/:id/sign-override handler is responsible for
-- re-normalizing existing transactions on the account when this column
-- changes, so that historical data reflects the new convention
-- without requiring a re-import.

ALTER TABLE accounts
    ADD COLUMN sign_convention_override TEXT;

ALTER TABLE accounts
    ADD CONSTRAINT accounts_sign_override_chk
    CHECK (
        sign_convention_override IS NULL
        OR sign_convention_override IN (
            'positive_means_inflow',
            'positive_means_outflow'
        )
    );

COMMENT ON COLUMN accounts.sign_convention_override IS
    'User-set sign-convention override for this account. When non-NULL, '
    'takes precedence over institution YAML rules and autodetection in '
    'the SignNormalizer chain. NULL means "use the normal resolution '
    'chain" (institution rule -> autodetection -> account-type default).';
