package narrative

import (
	"testing"
	"time"
)

func BenchmarkClassifierClassify(b *testing.B) {
	c := NewClassifier()
	events := []RawEvent{
		{Type: EventToolStart, Data: map[string]interface{}{"tool_name": "FileRead", "file": "main.go"}},
		{Type: EventTaskStart, Data: map[string]interface{}{"description": "test"}},
		{Type: EventError, Data: map[string]interface{}{"error": "fail"}},
		{Type: EventModelSwitch, Data: map[string]interface{}{}},
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		c.Classify(events[i%len(events)])
	}
}

func BenchmarkTemplateResolverResolve(b *testing.B) {
	r := NewTemplateResolver()
	event := RawEvent{
		Type:      EventTaskStart,
		Timestamp: time.Now(),
		Data: map[string]interface{}{
			"description": "implement auth module",
		},
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		r.Resolve(NarrativeStartingTask, event)
	}
}

func BenchmarkGrouperAddAndFlush(b *testing.B) {
	config := GroupConfig{
		Window:    5 * time.Second,
		MaxItems:  10,
		FlushIdle: 5 * time.Second,
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		g := NewGrouper(config)
		for j := 0; j < 5; j++ {
			g.Add(RawEvent{Type: EventToolStart, Data: map[string]interface{}{"tool_name": "FileRead"}}, "tools")
		}
		g.Flush("tools")
	}
}

func BenchmarkTimingGuardShouldEmit(b *testing.B) {
	tg := NewTimingGuard(DefaultTimingConfig())
	tg.Activate(NarrativeObject{Type: NarrativeReadingFile, Text: "test"})
	n := NarrativeObject{Type: NarrativeWritingFile, Text: "other"}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		tg.ShouldEmit(n)
	}
}

func BenchmarkEngineProcessEvent(b *testing.B) {
	e := NewEngine(DefaultEngineConfig())
	event := RawEvent{
		Type:      EventTaskStart,
		Timestamp: time.Now(),
		Data:      map[string]interface{}{"description": "benchmark test"},
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = e.ProcessEvent(event)
	}
}

func BenchmarkEngineProcessEventParallel(b *testing.B) {
	e := NewEngine(DefaultEngineConfig())
	event := RawEvent{
		Type:      EventTaskStart,
		Timestamp: time.Now(),
		Data:      map[string]interface{}{"description": "benchmark test"},
	}

	b.ResetTimer()
	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			_ = e.ProcessEvent(event)
		}
	})
}

func BenchmarkFullPipeline(b *testing.B) {
	e := NewEngine(DefaultEngineConfig())
	events := []RawEvent{
		{Type: EventToolStart, Data: map[string]interface{}{"tool_name": "FileRead", "file": "main.go"}},
		{Type: EventToolStart, Data: map[string]interface{}{"tool_name": "FileRead", "file": "config.go"}},
		{Type: EventToolStart, Data: map[string]interface{}{"tool_name": "Grep", "pattern": "TODO"}},
		{Type: EventTaskStart, Data: map[string]interface{}{"description": "fix bug"}},
		{Type: EventToolStart, Data: map[string]interface{}{"tool_name": "Bash", "command": "go test"}},
		{Type: EventError, Data: map[string]interface{}{"error": "fail"}},
		{Type: EventToolStart, Data: map[string]interface{}{"tool_name": "Edit", "file": "fix.go"}},
		{Type: EventTaskComplete, Data: map[string]interface{}{"description": "fix bug"}},
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = e.ProcessEvents(events)
		e.FlushGroups()
	}
}

func BenchmarkGroupSummary(b *testing.B) {
	events := []RawEvent{
		{Data: map[string]interface{}{"tool_name": "FileRead"}},
		{Data: map[string]interface{}{"tool_name": "FileRead"}},
		{Data: map[string]interface{}{"tool_name": "FileWrite"}},
		{Data: map[string]interface{}{"tool_name": "Grep"}},
		{Data: map[string]interface{}{"tool_name": "Bash"}},
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		GroupSummary(events, "tools")
	}
}

func BenchmarkCollectFiles(b *testing.B) {
	events := []RawEvent{
		{Data: map[string]interface{}{"file": "a.go"}},
		{Data: map[string]interface{}{"file_path": "b.go"}},
		{Data: map[string]interface{}{"path": "c.go"}},
		{Data: map[string]interface{}{"file": "d.go"}},
		{Data: map[string]interface{}{"file": "e.go"}},
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		CollectFiles(events)
	}
}
