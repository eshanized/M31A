-- Migration 015: PromptOS Phase 7 provenance tracking for model invocations (OBS-02, D-08)
ALTER TABLE model_invocations ADD COLUMN prompt_provenance TEXT;
