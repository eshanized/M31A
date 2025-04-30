func TestFmtBool(t *testing.T) {
	tests := []struct {
		input    bool
		expected string
	}{
		{true, "true"},
		{false, "false"},
	}

	for _, tt := range tests {
		got := fmtBool(tt.input)
		if got != tt.expected {
			t.Errorf("fmtBool(%v) = %q, expected %q", tt.input, got, tt.expected)
		}
	}
}

// ---------------------------------------------------------------------------
// Mock keychain for testing
// ---------------------------------------------------------------------------

type testKeychain struct {
	mu    sync.Mutex
	store map[string]string
}

func newTestKeychain() *testKeychain {
	return &testKeychain{store: make(map[string]string)}
}

func (t *testKeychain) Get(service string) (string, error) {
	t.mu.Lock()
	defer t.mu.Unlock()
	v, ok := t.store[service]
	if !ok {
		return "", keychain.ErrKeyNotFound
	}
	return v, nil
}

func (t *testKeychain) Set(service, value string) error {
	t.mu.Lock()
	defer t.mu.Unlock()
	t.store[service] = value
	return nil
}

func (t *testKeychain) Delete(service string) error {
	t.mu.Lock()
	defer t.mu.Unlock()
	if _, ok := t.store[service]; !ok {
		return keychain.ErrKeyNotFound
	}
	delete(t.store, service)
	return nil
}

// ---------------------------------------------------------------------------
// Test SettingsModel.saveAPIKeysToKeychain
// ---------------------------------------------------------------------------

func TestSettingsModel_SaveAPIKeysToKeychain(t *testing.T) {
	kc := newTestKeychain()
	cfg := config.DefaultConfig()
	cfg.Provider.OpenRouter.APIKey = "or-test-key"
	cfg.Provider.Zen.APIKey = "zen-test-key"

	m := NewSettingsModel(cfg, "/tmp/.m31a/config.toml", theme.Dark(), nil, kc)
	m.saveAPIKeysToKeychain()

	// Verify keys were saved
	or, err := kc.Get("openrouter")
	if err != nil {
		t.Fatalf("expected openrouter key, got error: %v", err)
	}
	if or != "or-test-key" {
		t.Errorf("expected 'or-test-key', got %q", or)
	}

	zen, err := kc.Get("zen")
	if err != nil {
		t.Fatalf("expected zen key, got error: %v", err)
	}
	if zen != "zen-test-key" {
		t.Errorf("expected 'zen-test-key', got %q", zen)
	}
}

func TestSettingsModel_SaveAPIKeysToKeychain_NilKeychain(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.Provider.OpenRouter.APIKey = "test-key"

	m := NewSettingsModel(cfg, "/tmp/.m31a/config.toml", theme.Dark(), nil, nil)
	// Should not panic
	m.saveAPIKeysToKeychain()
}

func TestSettingsModel_SaveAPIKeysToKeychain_EmptyKeys(t *testing.T) {
	kc := newTestKeychain()
	cfg := config.DefaultConfig()
	// Empty keys should not be saved
	m := NewSettingsModel(cfg, "/tmp/.m31a/config.toml", theme.Dark(), nil, kc)
	m.saveAPIKeysToKeychain()

	// Verify nothing was saved
	_, err := kc.Get("openrouter")
	if !errors.Is(err, keychain.ErrKeyNotFound) {
		t.Errorf("expected ErrKeyNotFound for empty openrouter key, got %v", err)
	}
}

// ---------------------------------------------------------------------------
// Test SetConfig re-masks API keys
// ---------------------------------------------------------------------------

func TestSettingsModel_SetConfig_RemasksAPIKeys(t *testing.T) {
	m := newTestSettingsModel()

	// First, unmask an API key field by starting edit
	m.activeTab = tabProvider
	m.focusedField = 2 // OpenRouter API Key field
	m = m.startEdit()
	m = m.confirmEdit()

	// Verify it's unmasked
	fields := m.fields[tabProvider]
	if fields[2].masked {
		t.Error("expected field to be unmasked after editing")
	}

	// Now call SetConfig (simulates post-save reload)
	m.SetConfig(m.config)

	// Verify API key fields are re-masked
	for _, fields := range m.fields {
		for _, f := range fields {
			if f.key == "provider.openrouter.api_key" || f.key == "provider.zen.api_key" {
				if !f.masked {
					t.Errorf("expected %s to be masked after SetConfig", f.key)
				}
			}
		}
	}
}
