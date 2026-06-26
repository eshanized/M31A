package efie

import (
	"container/heap"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/fileutil"
)

// --- query.go tests ---

func TestComputeMedianPageRank_Empty(t *testing.T) {
	g := NewWeightedImportGraph()
	result := computeMedianPageRank(g)
	if result != 0 {
		t.Errorf("computeMedianPageRank(empty) = %f, want 0", result)
	}
}

func TestComputeMedianPageRank_Single(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("a.go", nil, "go")
	g.nodes["a.go"].PageRank = 0.5
	result := computeMedianPageRank(g)
	if result != 0.5 {
		t.Errorf("computeMedianPageRank(single) = %f, want 0.5", result)
	}
}

func TestComputeMedianPageRank_Even(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("a.go", nil, "go")
	g.AddNode("b.go", nil, "go")
	g.nodes["a.go"].PageRank = 1.0
	g.nodes["b.go"].PageRank = 3.0
	result := computeMedianPageRank(g)
	if result != 2.0 {
		t.Errorf("computeMedianPageRank(even) = %f, want 2.0", result)
	}
}

func TestComputeMedianPageRank_Odd(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("a.go", nil, "go")
	g.AddNode("b.go", nil, "go")
	g.AddNode("c.go", nil, "go")
	g.nodes["a.go"].PageRank = 1.0
	g.nodes["b.go"].PageRank = 2.0
	g.nodes["c.go"].PageRank = 3.0
	result := computeMedianPageRank(g)
	if result != 2.0 {
		t.Errorf("computeMedianPageRank(odd) = %f, want 2.0", result)
	}
}

func TestSortFloat64s(t *testing.T) {
	tests := []struct {
		name string
		in   []float64
		want []float64
	}{
		{"empty", nil, nil},
		{"single", []float64{1.0}, []float64{1.0}},
		{"sorted", []float64{1.0, 2.0, 3.0}, []float64{1.0, 2.0, 3.0}},
		{"reverse", []float64{3.0, 1.0, 2.0}, []float64{1.0, 2.0, 3.0}},
		{"duplicates", []float64{2.0, 1.0, 2.0}, []float64{1.0, 2.0, 2.0}},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			sortFloat64s(tt.in)
			if len(tt.in) != len(tt.want) {
				t.Fatalf("len = %d, want %d", len(tt.in), len(tt.want))
			}
			for i := range tt.in {
				if tt.in[i] != tt.want[i] {
					t.Errorf("result[%d] = %f, want %f", i, tt.in[i], tt.want[i])
				}
			}
		})
	}
}

func TestItoa(t *testing.T) {
	tests := []struct {
		n    int
		want string
	}{
		{0, "0"},
		{1, "1"},
		{42, "42"},
		{12345, "12345"},
		{999999999, "999999999"},
	}

	for _, tt := range tests {
		got := itoa(tt.n)
		if got != tt.want {
			t.Errorf("itoa(%d) = %q, want %q", tt.n, got, tt.want)
		}
	}
}

func TestMaxHeap(t *testing.T) {
	h := &maxHeap{}
	heap.Init(h)

	heap.Push(h, &scoredEntry{path: "a.go", score: 1.0})
	heap.Push(h, &scoredEntry{path: "b.go", score: 3.0})
	heap.Push(h, &scoredEntry{path: "c.go", score: 2.0})

	if h.Len() != 3 {
		t.Fatalf("heap len = %d, want 3", h.Len())
	}

	first := heap.Pop(h).(*scoredEntry)
	if first.path != "b.go" || first.score != 3.0 {
		t.Errorf("first pop = %q/%f, want b.go/3.0", first.path, first.score)
	}

	second := heap.Pop(h).(*scoredEntry)
	if second.path != "c.go" || second.score != 2.0 {
		t.Errorf("second pop = %q/%f, want c.go/2.0", second.path, second.score)
	}

	third := heap.Pop(h).(*scoredEntry)
	if third.path != "a.go" || third.score != 1.0 {
		t.Errorf("third pop = %q/%f, want a.go/1.0", third.path, third.score)
	}
}

func TestQueryTypes(t *testing.T) {
	if QueryRelevant != "relevant" {
		t.Errorf("QueryRelevant = %q", QueryRelevant)
	}
	if QueryUpstream != "upstream" {
		t.Errorf("QueryUpstream = %q", QueryUpstream)
	}
	if QueryDownstream != "downstream" {
		t.Errorf("QueryDownstream = %q", QueryDownstream)
	}
	if QueryDefine != "define" {
		t.Errorf("QueryDefine = %q", QueryDefine)
	}
	if QueryReferences != "references" {
		t.Errorf("QueryReferences = %q", QueryReferences)
	}
}

// --- index.go tests ---

func TestDirOf(t *testing.T) {
	tests := []struct {
		path string
		want string
	}{
		{"src/main.go", "src"},
		{"a/b/c.go", "a/b"},
		{"/absolute/path.go", "/absolute"},
		{"file.go", "."},
		{"", "."},
		{"/", "."},
		{"path/to/dir/", "path/to/dir"},
		{"back\\slash\\file.go", "back\\slash"},
	}

	for _, tt := range tests {
		got := fileutil.DirOf(tt.path)
		if got != tt.want {
			t.Errorf("DirOf(%q) = %q, want %q", tt.path, got, tt.want)
		}
	}
}

func TestToLower(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"HELLO", "hello"},
		{"Hello World", "hello world"},
		{"already lower", "already lower"},
		{"", ""},
		{"ABC123", "abc123"},
		{"!@#$%", "!@#$%"},
	}

	for _, tt := range tests {
		got := toLower(tt.input)
		if got != tt.want {
			t.Errorf("toLower(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestContains(t *testing.T) {
	tests := []struct {
		s, substr string
		want      bool
	}{
		{"hello", "ell", true},
		{"hello", "world", false},
		{"hello", "", true},
		{"hello", "hello", true},
		{"hello", "helloo", false},
		{"", "", true},
		{"", "a", false},
	}

	for _, tt := range tests {
		got := strings.Contains(tt.s, tt.substr)
		if got != tt.want {
			t.Errorf("Contains(%q, %q) = %v, want %v", tt.s, tt.substr, got, tt.want)
		}
	}
}

func TestMultiResIndex_BasicOperations(t *testing.T) {
	idx := NewMultiResIndex()

	idx.AddFile("a.go", "go", []SymbolInfo{
		{Name: "FuncA", Kind: "func", Exported: true},
		{Name: "varX", Kind: "var", Exported: false},
	})
	idx.AddFile("b.go", "go", []SymbolInfo{
		{Name: "FuncB", Kind: "func", Exported: true},
	})

	if idx.FileCount() != 2 {
		t.Errorf("FileCount() = %d, want 2", idx.FileCount())
	}
	if idx.SymbolCount() != 3 {
		t.Errorf("SymbolCount() = %d, want 3", idx.SymbolCount())
	}

	defs := idx.Define("FuncA")
	if len(defs) != 1 {
		t.Fatalf("Define(FuncA) len = %d, want 1", len(defs))
	}
	if defs[0].File != "a.go" {
		t.Errorf("Define(FuncA) file = %q, want a.go", defs[0].File)
	}

	syms := idx.FileSymbols("a.go")
	if len(syms) != 2 {
		t.Errorf("FileSymbols(a.go) len = %d, want 2", len(syms))
	}

	allSyms := idx.AllSymbols()
	if len(allSyms) != 3 {
		t.Errorf("AllSymbols() len = %d, want 3", len(allSyms))
	}
}

func TestMultiResIndex_SettersAndGetters(t *testing.T) {
	idx := NewMultiResIndex()
	idx.AddFile("a.go", "go", nil)

	idx.SetCommunity("a.go", 42)
	if idx.CommunityOf("a.go") != 42 {
		t.Errorf("CommunityOf = %d, want 42", idx.CommunityOf("a.go"))
	}

	idx.SetCommunityAdj(map[int]map[int]bool{42: {43: true}})
	adj := idx.CommunityAdj()
	if !adj[42][43] {
		t.Error("CommunityAdj missing 42->43")
	}

	idx.SetImportanceRank([]string{"a.go", "b.go"})
	rank := idx.SortedImportanceRank()
	if len(rank) != 2 || rank[0] != "a.go" {
		t.Errorf("SortedImportanceRank = %v", rank)
	}

	idx.SetCentralityPercentiles(0.5, 0.95)
	communities := idx.Communities()
	_ = communities
}

func TestMultiResIndex_SymbolsMatching(t *testing.T) {
	idx := NewMultiResIndex()
	idx.AddFile("a.go", "go", []SymbolInfo{
		{Name: "HandleRequest", Kind: "func", Exported: true},
		{Name: "handleInternal", Kind: "func", Exported: false},
	})
	idx.BuildTrieIndex()

	matches := idx.SymbolsMatching("Handle")
	if len(matches) == 0 {
		t.Error("SymbolsMatching('Handle') returned empty")
	}
}

func TestMultiResIndex_CommunityOf_Missing(t *testing.T) {
	idx := NewMultiResIndex()
	if idx.CommunityOf("missing.go") != 0 {
		t.Error("CommunityOf for missing path should be 0")
	}
}

// --- community.go tests ---

func TestHashString(t *testing.T) {
	// Deterministic
	h1 := hashString("hello")
	h2 := hashString("hello")
	if h1 != h2 {
		t.Errorf("hashString(hello) = %d, %d (should be equal)", h1, h2)
	}

	// Different strings
	h3 := hashString("world")
	if h1 == h3 {
		t.Error("hashString(hello) == hashString(world)")
	}

	// Empty string
	h4 := hashString("")
	if h4 != 0 {
		t.Errorf("hashString(\"\") = %d, want 0", h4)
	}

	// Negative hash normalization
	h5 := hashString(string([]byte{255, 255, 255, 255, 255}))
	if h5 < 0 {
		t.Errorf("hashString long string = %d, want >= 0", h5)
	}
}

func TestSortedStringSlice(t *testing.T) {
	tests := []struct {
		name string
		in   []string
		want []string
	}{
		{"empty", nil, nil},
		{"single", []string{"a"}, []string{"a"}},
		{"already sorted", []string{"a", "b", "c"}, []string{"a", "b", "c"}},
		{"unsorted", []string{"c", "a", "b"}, []string{"a", "b", "c"}},
		{"duplicates", []string{"b", "a", "b"}, []string{"a", "b", "b"}},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			sortedStringSlice(tt.in)
			if len(tt.in) != len(tt.want) {
				t.Fatalf("len = %d, want %d", len(tt.in), len(tt.want))
			}
			for i := range tt.in {
				if tt.in[i] != tt.want[i] {
					t.Errorf("result[%d] = %q, want %q", i, tt.in[i], tt.want[i])
				}
			}
		})
	}
}

func TestLouvainDeterministic_SingleNode(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("a.go", nil, "go")

	result := LouvainDetect_Deterministic(g, 42, 10)
	if len(result) != 1 {
		t.Errorf("single node result len = %d, want 1", len(result))
	}
	if _, ok := result["a.go"]; !ok {
		t.Error("result missing a.go")
	}
}

func TestLouvainDeterministic_NoEdges(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("a.go", nil, "go")
	g.AddNode("b.go", nil, "go")

	result := LouvainDetect_Deterministic(g, 42, 10)
	if len(result) != 2 {
		t.Errorf("no edges result len = %d, want 2", len(result))
	}
}

// --- parsers.go tests ---

func TestGoParser(t *testing.T) {
	p := GoParser{}
	if p.Language() != "go" {
		t.Errorf("Language() = %q, want go", p.Language())
	}
	if !p.CanParse("main.go") {
		t.Error("CanParse(main.go) = false")
	}
	if p.CanParse("main.py") {
		t.Error("CanParse(main.py) = true")
	}
}

func TestGoParser_Parse(t *testing.T) {
	p := GoParser{}
	src := []byte(`package main

import "fmt"

func main() {
	fmt.Println("hello")
}

func Helper() string {
	return "help"
}

type Config struct {
	Name string
}
`)
	info, err := p.Parse("test.go", src)
	if err != nil {
		t.Fatalf("Parse error: %v", err)
	}
	if info.Path != "test.go" {
		t.Errorf("Path = %q", info.Path)
	}
	if info.Language != "go" {
		t.Errorf("Language = %q", info.Language)
	}
	if len(info.Imports) != 1 {
		t.Errorf("Imports len = %d, want 1", len(info.Imports))
	}
	if len(info.Funcs) < 2 {
		t.Errorf("Funcs len = %d, want >= 2", len(info.Funcs))
	}
	if len(info.Types) < 1 {
		t.Errorf("Types len = %d, want >= 1", len(info.Types))
	}
}

func TestTypeScriptParser(t *testing.T) {
	p := TypeScriptParser{}
	if p.Language() != "typescript" {
		t.Errorf("Language() = %q", p.Language())
	}
	if !p.CanParse("main.ts") {
		t.Error("CanParse(main.ts) = false")
	}
	if !p.CanParse("main.tsx") {
		t.Error("CanParse(main.tsx) = false")
	}
	if !p.CanParse("main.js") {
		t.Error("CanParse(main.js) = false")
	}
	if !p.CanParse("main.jsx") {
		t.Error("CanParse(main.jsx) = false")
	}
	if !p.CanParse("main.mjs") {
		t.Error("CanParse(main.mjs) = false")
	}
	if !p.CanParse("main.cjs") {
		t.Error("CanParse(main.cjs) = false")
	}
	if p.CanParse("main.go") {
		t.Error("CanParse(main.go) = true")
	}
}

func TestTypeScriptParser_Parse(t *testing.T) {
	p := TypeScriptParser{}
	src := []byte(`import { foo } from 'bar';
import 'side-effect';

export function hello(x: number): string {
	return String(x);
}

export class MyClass {
	name: string;
}

export interface Config {
	width: number;
}

export type ID = string | number;

export enum Color {
	Red,
	Green,
	Blue,
}
`)
	info, err := p.Parse("test.ts", src)
	if err != nil {
		t.Fatalf("Parse error: %v", err)
	}
	if len(info.Imports) < 2 {
		t.Errorf("Imports len = %d, want >= 2", len(info.Imports))
	}
	if len(info.Exports) < 5 {
		t.Errorf("Exports len = %d, want >= 5", len(info.Exports))
	}
	if len(info.Funcs) < 1 {
		t.Errorf("Funcs len = %d, want >= 1", len(info.Funcs))
	}
	if len(info.Types) < 3 {
		t.Errorf("Types len = %d, want >= 3", len(info.Types))
	}
}

func TestPythonParser(t *testing.T) {
	p := PythonParser{}
	if p.Language() != "python" {
		t.Errorf("Language() = %q", p.Language())
	}
	if !p.CanParse("main.py") {
		t.Error("CanParse(main.py) = false")
	}
	if p.CanParse("main.go") {
		t.Error("CanParse(main.go) = true")
	}
}

func TestPythonParser_Parse(t *testing.T) {
	p := PythonParser{}
	src := []byte(`import os
from sys import argv

class MyClass:
	pass

def hello():
	pass

def _private():
	pass

value = 42
`)
	info, err := p.Parse("test.py", src)
	if err != nil {
		t.Fatalf("Parse error: %v", err)
	}
	if len(info.Imports) < 2 {
		t.Errorf("Imports len = %d, want >= 2", len(info.Imports))
	}
	if len(info.Types) < 1 {
		t.Errorf("Types len = %d, want >= 1", len(info.Types))
	}
	if len(info.Funcs) < 1 {
		t.Errorf("Funcs len = %d, want >= 1", len(info.Funcs))
	}
}

func TestRustParser(t *testing.T) {
	p := RustParser{}
	if p.Language() != "rust" {
		t.Errorf("Language() = %q", p.Language())
	}
	if !p.CanParse("main.rs") {
		t.Error("CanParse(main.rs) = false")
	}
	if p.CanParse("main.go") {
		t.Error("CanParse(main.go) = true")
	}
}

func TestRustParser_Parse(t *testing.T) {
	p := RustParser{}
	src := []byte(`use std::io;

pub fn main() {
	println!("hello");
}

pub struct Config {
	name: String,
}

pub enum Color {
	Red,
	Green,
}
`)
	info, err := p.Parse("test.rs", src)
	if err != nil {
		t.Fatalf("Parse error: %v", err)
	}
	if len(info.Imports) < 1 {
		t.Errorf("Imports len = %d, want >= 1", len(info.Imports))
	}
	if len(info.Exports) < 3 {
		t.Errorf("Exports len = %d, want >= 3", len(info.Exports))
	}
	if len(info.Funcs) < 1 {
		t.Errorf("Funcs len = %d, want >= 1", len(info.Funcs))
	}
	if len(info.Types) < 1 {
		t.Errorf("Types len = %d, want >= 1", len(info.Types))
	}
}

func TestIsExported(t *testing.T) {
	tests := []struct {
		name     string
		language string
		want     bool
	}{
		{"ExportedFunc", "go", true},
		{"exportedFunc", "go", false},
		{"hello", "python", true},
		{"_private", "python", false},
		{"__dunder__", "python", false},
		{"anything", "rust", true},
		{"_prefix", "rust", false},
		{"", "go", false},
	}

	for _, tt := range tests {
		got := IsExported(tt.name, tt.language)
		if got != tt.want {
			t.Errorf("IsExported(%q, %q) = %v, want %v", tt.name, tt.language, got, tt.want)
		}
	}
}

func TestCleanPyParams(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"self", ""},
		{"cls", ""},
		{"self, x, y", "x, y"},
		{"cls, x", "x"},
		{"x, y", "x, y"},
		{"", ""},
		{"  x  ", "x"},
	}

	for _, tt := range tests {
		got := cleanPyParams(tt.input)
		if got != tt.want {
			t.Errorf("cleanPyParams(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

// --- scorer.go tests ---

func TestExactSymbolCheck(t *testing.T) {
	idx := NewMultiResIndex()
	idx.byName["FuncA"] = []SymbolLocation{{File: "a.go", Kind: "func"}}

	if !exactSymbolCheck("a.go", "FuncA", idx) {
		t.Error("exactSymbolCheck should return true for matching file")
	}
	if exactSymbolCheck("b.go", "FuncA", idx) {
		t.Error("exactSymbolCheck should return false for non-matching file")
	}
	if exactSymbolCheck("a.go", "FuncB", idx) {
		t.Error("exactSymbolCheck should return false for missing symbol")
	}
}

func TestSortByScoreDescending(t *testing.T) {
	files := []ScoredFile{
		{Path: "a.go", Score: 1.0},
		{Path: "b.go", Score: 3.0},
		{Path: "c.go", Score: 2.0},
	}
	sortByScoreDescending(files)
	if files[0].Path != "b.go" || files[0].Score != 3.0 {
		t.Errorf("first = %q/%f", files[0].Path, files[0].Score)
	}
	if files[1].Path != "c.go" || files[1].Score != 2.0 {
		t.Errorf("second = %q/%f", files[1].Path, files[1].Score)
	}
	if files[2].Path != "a.go" || files[2].Score != 1.0 {
		t.Errorf("third = %q/%f", files[2].Path, files[2].Score)
	}
}

func TestSortByScoreDescending_Empty(t *testing.T) {
	sortByScoreDescending(nil)
	sortByScoreDescending([]ScoredFile{})
}

func TestIsStopWord(t *testing.T) {
	if !isStopWord("the") {
		t.Error("'the' should be a stop word")
	}
	if isStopWord("hello") {
		t.Error("'hello' should not be a stop word")
	}
}

// --- rng.go tests ---

func TestSimpleRng_Deterministic(t *testing.T) {
	r1 := newSeededRng(42)
	r2 := newSeededRng(42)
	for i := 0; i < 10; i++ {
		if r1.Intn(100) != r2.Intn(100) {
			t.Errorf("Intn not deterministic at step %d", i)
		}
	}
}

func TestSimpleRng_IntnBounds(t *testing.T) {
	r := newSeededRng(42)
	for i := 0; i < 1000; i++ {
		v := r.Intn(10)
		if v < 0 || v >= 10 {
			t.Errorf("Intn(10) = %d, out of range", v)
		}
	}
}

func TestSimpleRng_Perm(t *testing.T) {
	r := newSeededRng(42)
	perm := r.Perm(10)
	if len(perm) != 10 {
		t.Fatalf("Perm(10) len = %d", len(perm))
	}
	seen := make(map[int]bool)
	for _, v := range perm {
		if seen[v] {
			t.Errorf("Perm(10) duplicate %d", v)
		}
		seen[v] = true
	}
}

func TestSimpleRng_Shuffle(t *testing.T) {
	r := newSeededRng(42)
	s := []int{1, 2, 3, 4, 5}
	r.Shuffle(len(s), func(i, j int) { s[i], s[j] = s[j], s[i] })
	// After shuffle, all elements should still be present
	seen := make(map[int]bool)
	for _, v := range s {
		seen[v] = true
	}
	for i := 1; i <= 5; i++ {
		if !seen[i] {
			t.Errorf("Shuffle lost element %d", i)
		}
	}
}

// --- graph.go additional tests ---

func TestWeightedImportGraph_HasNode(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("a.go", nil, "go")

	if !g.HasNode("a.go") {
		t.Error("HasNode(a.go) = false")
	}
	if g.HasNode("missing.go") {
		t.Error("HasNode(missing.go) = true")
	}
}

func TestWeightedImportGraph_NodeCount(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("a.go", nil, "go")
	g.AddNode("b.go", nil, "go")

	if g.NodeCount() != 2 {
		t.Errorf("NodeCount() = %d, want 2", g.NodeCount())
	}
}

func TestWeightedImportGraph_Neighbors(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("a.go", []string{"b.go"}, "go")
	g.AddNode("b.go", nil, "go")

	// a.go imports b.go, so a.go's imports = [b.go], a.go's importedBy = []
	// b.go is imported by a.go, so b.go's importedBy = [a.go]
	imports, _ := g.Neighbors("a.go")
	if len(imports) != 1 || imports[0] != "b.go" {
		t.Errorf("Neighbors(a.go) imports = %v", imports)
	}

	_, importedBy := g.Neighbors("b.go")
	if len(importedBy) != 1 || importedBy[0] != "a.go" {
		t.Errorf("Neighbors(b.go) importedBy = %v", importedBy)
	}

	_, _ = g.Neighbors("missing.go")
}

func TestWeightedImportGraph_AllPaths(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("a.go", nil, "go")
	g.AddNode("b.go", nil, "go")

	paths := g.AllPaths()
	if len(paths) != 2 {
		t.Errorf("AllPaths() len = %d, want 2", len(paths))
	}
}

func TestWeightedImportGraph_SetExternal(t *testing.T) {
	g := NewWeightedImportGraph()
	g.SetExternal("ext.go")

	if !g.HasNode("ext.go") {
		t.Error("external node not created")
	}
}

func TestWeightedImportGraph_RebuildEdges(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("a.go", []string{"b.go"}, "go")
	g.AddNode("b.go", nil, "go")
	g.AddNode("c.go", nil, "go")

	g.RebuildEdges("a.go", []string{"c.go"})
	imports, _ := g.Neighbors("a.go")
	if len(imports) != 1 || imports[0] != "c.go" {
		t.Errorf("RebuildEdges imports = %v", imports)
	}

	// Verify reverse edges: c.go should be imported by a.go
	_, importedBy := g.Neighbors("c.go")
	if len(importedBy) != 1 || importedBy[0] != "a.go" {
		t.Errorf("RebuildEdges c.go importedBy = %v", importedBy)
	}

	// Verify old reverse edge removed: b.go should not be imported by a.go
	_, bImportedBy := g.Neighbors("b.go")
	if len(bImportedBy) != 0 {
		t.Errorf("RebuildEdges b.go importedBy = %v, want empty", bImportedBy)
	}
}

func TestWeightedImportGraph_RebuildEdges_MissingPath(t *testing.T) {
	g := NewWeightedImportGraph()
	g.RebuildEdges("missing.go", []string{"other.go"})
}

func TestWeightedImportGraph_AllNodes(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("a.go", nil, "go")
	g.AddNode("b.go", nil, "go")

	nodes := g.AllNodes()
	if len(nodes) != 2 {
		t.Errorf("AllNodes() len = %d, want 2", len(nodes))
	}
}
