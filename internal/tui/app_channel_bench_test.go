package tui

import (
	"context"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
)

func newBenchAppState() *AppState {
	ch := make(chan tea.Msg, ChannelCap)
	return &AppState{
		emitterCh:   ch,
		shutdownCtx: context.Background(),
	}
}

func BenchmarkChannelEmitter_200Burst(b *testing.B) {
	emitter := newChannelEmitter()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		for j := 0; j < 200; j++ {
			emitter.Emit(j)
		}
		ch := emitter.ch
		for j := 0; j < 200; j++ {
			<-ch
		}
	}
}

func BenchmarkChannelEmitter_500Burst(b *testing.B) {
	emitter := newChannelEmitter()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		for j := 0; j < 500; j++ {
			emitter.Emit(j)
		}
		ch := emitter.ch
		for j := 0; j < 500; j++ {
			<-ch
		}
	}
}

func BenchmarkDrainEmitterCmd(b *testing.B) {
	app := newBenchAppState()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		app.emitterCh <- 1
		cmd := app.drainEmitterCmd()
		if cmd != nil {
			cmd()
		}
	}
}

func BenchmarkDrainMultipleCmd_Single(b *testing.B) {
	app := newBenchAppState()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		app.emitterCh <- 1
		cmd := app.drainMultipleCmd()
		if cmd != nil {
			cmd()
		}
	}
}

func BenchmarkDrainMultipleCmd_Batch4(b *testing.B) {
	app := newBenchAppState()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		for j := 0; j < 4; j++ {
			app.emitterCh <- 1
		}
		cmd := app.drainMultipleCmd()
		if cmd != nil {
			cmd()
		}
	}
}

func BenchmarkDrainAdaptiveCmd_LowLoad(b *testing.B) {
	app := newBenchAppState()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		app.emitterCh <- 1
		cmd := app.drainAdaptiveCmd()
		if cmd != nil {
			cmd()
		}
	}
}

func BenchmarkDrainAdaptiveCmd_HighLoad(b *testing.B) {
	app := newBenchAppState()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		for j := 0; j < 200; j++ {
			app.emitterCh <- 1
		}
		cmd := app.drainAdaptiveCmd()
		if cmd != nil {
			cmd()
		}
		// Drain remaining so channel doesn't fill up
		for j := 0; j < 200; j++ {
			<-app.emitterCh
		}
	}
}
