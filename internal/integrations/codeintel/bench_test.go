package codeintel

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

// Sample Go source for benchmarks
var benchGoSrc = []byte(`package main

import (
	"fmt"
	"net/http"
	"github.com/eshanized/M31A/internal/core/types"
)

type Engine struct {
	Name    string
	Version int
}

func (e *Engine) Run(ctx context.Context) error {
	fmt.Println("running")
	return nil
}

func NewEngine(name string) *Engine {
	return &Engine{Name: name}
}

const MaxRetries = 3
var DefaultTimeout = 30

func process(data []byte) (string, error) {
	return "", nil
}
`)

// Sample TypeScript source for benchmarks
var benchTSSrc = []byte(`import { useState } from 'react';
import type { Config } from './config';
import express from 'express';

export function greet(name: string): string {
  return "hello " + name;
}

export class UserService {
  getUser(id: string) {}
}

export interface Config {
  port: number;
  host: string;
}

export enum Status {
  Active,
  Inactive,
}
`)

func BenchmarkTreeSitterParseGo(b *testing.B) {
	p := &TreeSitterParser{}
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		p.Parse("main.go", benchGoSrc)
	}
}

func BenchmarkTreeSitterParseTS(b *testing.B) {
	p := &TreeSitterParser{}
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		p.Parse("app.ts", benchTSSrc)
	}
}

func BenchmarkRegexParseGo(b *testing.B) {
	p := &PythonParser{} // not for Go, but showing regex overhead
	_ = p
	// PythonParser can't parse Go, so we just measure TreeSitter vs nothing
	// The real comparison is TreeSitter vs the old regex parsers which are deleted
	b.Skip("old regex parsers removed — no baseline to compare")
}

func BenchmarkSymbolIndexAddAndQuery(b *testing.B) {
	idx := NewSymbolIndex()
	info := &FileInfo{
		Path: "bench.go",
		Exports: []SymbolInfo{
			{Name: "UserService", Kind: "struct", Exported: true},
			{Name: "GetUser", Kind: "func", Exported: true},
			{Name: "ProcessData", Kind: "func", Exported: true},
			{Name: "Config", Kind: "interface", Exported: true},
		},
		Funcs: []FuncSignature{
			{Name: "GetUser", Exported: true},
			{Name: "ProcessData", Exported: true},
		},
		Types: []TypeInfo{
			{Name: "UserService", Kind: "struct"},
			{Name: "Config", Kind: "interface"},
		},
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		idx.AddFile(info)
		idx.SymbolsMatching("User")
		idx.Define("GetUser")
		idx.FindTypes("Config")
	}
}

func BenchmarkSymbolIndexRemoveFile(b *testing.B) {
	for i := 0; i < b.N; i++ {
		b.StopTimer()
		idx := NewSymbolIndex()
		for j := 0; j < 100; j++ {
			idx.AddFile(&FileInfo{
				Path: fmt.Sprintf("file%d.go", j),
				Exports: []SymbolInfo{
					{Name: fmt.Sprintf("Symbol%d", j), Kind: "func", Exported: true},
				},
			})
		}
		b.StartTimer()
		idx.RemoveFile("file50.go")
	}
}

func BenchmarkTrieInsertAndSearch(b *testing.B) {
	trie := NewSymbolTrie()
	names := make([]string, 1000)
	for i := range names {
		names[i] = fmt.Sprintf("Symbol_%d", i)
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		for _, n := range names {
			trie.Insert(n)
		}
		trie.PrefixSearch("Symbol_")
		trie.Search("Symbol_500")
	}
}

func BenchmarkSymbolsMatching(b *testing.B) {
	idx := NewSymbolIndex()
	for i := 0; i < 500; i++ {
		idx.AddFile(&FileInfo{
			Path: fmt.Sprintf("pkg%d/file.go", i),
			Exports: []SymbolInfo{
				{Name: fmt.Sprintf("GetUser%d", i), Kind: "func", Exported: true},
				{Name: fmt.Sprintf("UserService%d", i), Kind: "struct", Exported: true},
				{Name: fmt.Sprintf("Config%d", i), Kind: "interface", Exported: true},
			},
		})
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		idx.SymbolsMatching("User")
	}
}

func BenchmarkBuildGraphSmall(b *testing.B) {
	dir := b.TempDir()
	// Create a small Go project
	os.MkdirAll(filepath.Join(dir, "pkg"), 0o755)
	os.WriteFile(filepath.Join(dir, "main.go"), benchGoSrc, 0o644)
	os.WriteFile(filepath.Join(dir, "pkg", "types.go"), []byte(`package pkg
type Config struct { Name string }
`), 0o644)
	os.WriteFile(filepath.Join(dir, "go.mod"), []byte(`module bench
go 1.21
`), 0o644)

	parsers := AllParsers()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		BuildGraph(dir, parsers)
	}
}

func BenchmarkBuildIndexFromFiles(b *testing.B) {
	files := make([]*FileInfo, 100)
	for i := range files {
		files[i] = &FileInfo{
			Path: fmt.Sprintf("pkg%d/file.go", i),
			Exports: []SymbolInfo{
				{Name: fmt.Sprintf("Symbol%d", i), Kind: "func", Exported: true},
			},
			Funcs: []FuncSignature{
				{Name: fmt.Sprintf("Symbol%d", i), Exported: true},
			},
		}
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		BuildIndex(files)
	}
}

func BenchmarkRelevanceScore(b *testing.B) {
	graph := NewCodeGraph()
	for i := 0; i < 50; i++ {
		var imports []string
		if i > 0 {
			imports = append(imports, fmt.Sprintf("pkg%d/file.go", i-1))
		}
		graph.AddNode(fmt.Sprintf("pkg%d/file.go", i), imports, "go")
	}

	idx := NewSymbolIndex()
	for i := 0; i < 50; i++ {
		idx.AddFile(&FileInfo{
			Path: fmt.Sprintf("pkg%d/file.go", i),
			Exports: []SymbolInfo{
				{Name: fmt.Sprintf("Symbol%d", i), Kind: "func", Exported: true},
			},
		})
	}

	scorer := NewRelevanceScorer(graph, idx)
	targets := []string{"pkg25/file.go"}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		scorer.Score(targets, "modify Symbol25", 10)
	}
}

func BenchmarkIncrementalBuild(b *testing.B) {
	dir := b.TempDir()
	os.MkdirAll(filepath.Join(dir, "pkg"), 0o755)
	os.WriteFile(filepath.Join(dir, "main.go"), benchGoSrc, 0o644)
	os.WriteFile(filepath.Join(dir, "go.mod"), []byte(`module bench
go 1.21
`), 0o644)

	parsers := AllParsers()
	graph, files, _ := BuildGraph(dir, parsers)
	symIndex := BuildIndex(files)
	idx := &Indexer{
		workDir: dir,
		parsers: parsers,
		graph:   graph,
		index:   symIndex,
		scorer:  NewRelevanceScorer(graph, symIndex),
		files:   files,
	}

	// Save initial cache
	fileCaches := BuildCacheFromFiles(dir, files, nil)
	SaveCache(dir, &IndexCache{Files: fileCaches})

	// Modify one file to trigger incremental
	os.WriteFile(filepath.Join(dir, "main.go"), []byte(`package main
func main() { println("changed") }
`), 0o644)

	ctx := context.Background()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		idx.Build(ctx)
	}
}

func BenchmarkParallelBuild(b *testing.B) {
	CPUs := []int{1, 2, 4}
	for _, cpus := range CPUs {
		b.Run(fmt.Sprintf("CPUs=%d", cpus), func(b *testing.B) {
			dir := b.TempDir()
			// Create many files
			for i := 0; i < 50; i++ {
				os.MkdirAll(filepath.Join(dir, fmt.Sprintf("pkg%d", i)), 0o755)
				os.WriteFile(filepath.Join(dir, fmt.Sprintf("pkg%d/file.go", i)), benchGoSrc, 0o644)
			}
			os.WriteFile(filepath.Join(dir, "go.mod"), []byte(`module bench
go 1.21
`), 0o644)

			parsers := AllParsers()
			b.SetParallelism(cpus)
			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				BuildGraph(dir, parsers)
			}
		})
	}
}

func BenchmarkFormatContext(b *testing.B) {
	graph := NewCodeGraph()
	for i := 0; i < 20; i++ {
		var imports []string
		if i > 0 {
			imports = append(imports, fmt.Sprintf("pkg%d/file.go", i-1))
		}
		graph.AddNode(fmt.Sprintf("pkg%d/file.go", i), imports, "go")
	}

	idx := NewSymbolIndex()
	for i := 0; i < 20; i++ {
		idx.AddFile(&FileInfo{
			Path: fmt.Sprintf("pkg%d/file.go", i),
			Exports: []SymbolInfo{
				{Name: fmt.Sprintf("Type%d", i), Kind: "struct", Exported: true},
			},
		})
	}

	indexer := &Indexer{
		graph:  graph,
		index:  idx,
		scorer: NewRelevanceScorer(graph, idx),
		files:  make([]*FileInfo, 20),
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		indexer.FormatContext([]string{"pkg10/file.go"}, "modify Type10", 5, 4096)
	}
}

func BenchmarkMemoryUsage(b *testing.B) {
	b.ReportAllocs()
	dir := b.TempDir()
	for i := 0; i < 100; i++ {
		os.MkdirAll(filepath.Join(dir, fmt.Sprintf("pkg%d", i)), 0o755)
		os.WriteFile(filepath.Join(dir, fmt.Sprintf("pkg%d/file.go", i)), benchGoSrc, 0o644)
	}
	os.WriteFile(filepath.Join(dir, "go.mod"), []byte(`module bench
go 1.21
`), 0o644)

	parsers := AllParsers()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		graph, files, _ := BuildGraph(dir, parsers)
		symIndex := BuildIndex(files)
		_ = graph
		_ = symIndex
	}

	var m runtime.MemStats
	runtime.ReadMemStats(&m)
	b.ReportMetric(float64(m.Alloc)/1024/1024, "MB_alloc")
}

func BenchmarkParseFilesSequential(b *testing.B) {
	dir := b.TempDir()
	paths := make([]string, 50)
	for i := 0; i < 50; i++ {
		paths[i] = fmt.Sprintf("pkg%d/file.go", i)
		os.MkdirAll(filepath.Join(dir, fmt.Sprintf("pkg%d", i)), 0o755)
		os.WriteFile(filepath.Join(dir, paths[i]), benchGoSrc, 0o644)
	}

	idx := &Indexer{workDir: dir, parsers: AllParsers()}
	ctx := context.Background()
	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		idx.parseFiles(ctx, paths)
	}
}

func BenchmarkParseFilesParallel(b *testing.B) {
	dir := b.TempDir()
	paths := make([]string, 50)
	for i := 0; i < 50; i++ {
		paths[i] = fmt.Sprintf("pkg%d/file.go", i)
		os.MkdirAll(filepath.Join(dir, fmt.Sprintf("pkg%d", i)), 0o755)
		os.WriteFile(filepath.Join(dir, paths[i]), benchGoSrc, 0o644)
	}

	idx := &Indexer{workDir: dir, parsers: AllParsers()}
	ctx := context.Background()
	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		idx.parseFilesParallel(ctx, paths)
	}
}
