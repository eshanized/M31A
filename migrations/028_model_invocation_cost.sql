-- Migration 028: model invocation cost and provenance tracking (D-08, MDL-05).
--
-- Records authoritative or estimated financial cost per model invocation.
-- cost_usd defaults NULL when cost is unknown (never synthetic 0.00).
-- cost_provenance records Authoritative, Estimated, or Unknown.

ALTER TABLE model_invocations ADD COLUMN cost_usd REAL;
ALTER TABLE model_invocations ADD COLUMN cost_provenance TEXT;
