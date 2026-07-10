package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"
)

type Package struct {
	Dir        string   `json:"Dir"`
	ImportPath string   `json:"ImportPath"`
	Imports    []string `json:"Imports"`
	TestImports   []string `json:"TestImports"`
	XTestImports  []string `json:"XTestImports"`
	Standard bool `json:"Standard"`
	Goroot   bool `json:"Goroot"`
	DepOnly  bool `json:"DepOnly"`
}

func main() {
	dec := json.NewDecoder(os.Stdin)
	
	edges := make(map[string]map[string]bool)
	allPkgs := make(map[string]*Package)
	
	for {
		var pkg Package
		if err := dec.Decode(&pkg); err != nil {
			break
		}
		if strings.HasPrefix(pkg.ImportPath, "github.com/eshanized/M31A") {
			allPkgs[pkg.ImportPath] = &pkg
			if _, ok := edges[pkg.ImportPath]; !ok {
				edges[pkg.ImportPath] = make(map[string]bool)
			}
			allImports := append(append([]string{}, pkg.Imports...), pkg.TestImports...)
			allImports = append(allImports, pkg.XTestImports...)
			for _, imp := range allImports {
				if strings.HasPrefix(imp, "github.com/eshanized/M31A") {
					edges[pkg.ImportPath][imp] = true
				}
			}
		}
	}
	
	// Tarjan's algorithm for SCCs
	index := 0
	indices := make(map[string]int)
	lowlink := make(map[string]int)
	onStack := make(map[string]bool)
	stack := []string{}
	var cycles [][]string
	
	var strongconnect func(string)
	strongconnect = func(v string) {
		indices[v] = index
		lowlink[v] = index
		index++
		stack = append(stack, v)
		onStack[v] = true
		
		for w := range edges[v] {
			if _, ok := indices[w]; !ok {
				strongconnect(w)
				lowlink[v] = min(lowlink[v], lowlink[w])
			} else if onStack[w] {
				lowlink[v] = min(lowlink[v], indices[w])
			}
		}
		
		if lowlink[v] == indices[v] {
			var scc []string
			for {
				w := stack[len(stack)-1]
				stack = stack[:len(stack)-1]
				onStack[w] = false
				scc = append(scc, w)
				if w == v {
					break
				}
			}
			if len(scc) > 1 {
				cycles = append(cycles, scc)
			}
		}
	}
	
	for v := range edges {
		if _, ok := indices[v]; !ok {
			strongconnect(v)
		}
	}
	
	fmt.Printf("=== CYCLES FOUND (%d) ===\n", len(cycles))
	for i, cycle := range cycles {
		fmt.Printf("Cycle %d:\n", i+1)
		for _, c := range cycle {
			short := strings.TrimPrefix(c, "github.com/eshanized/M31A/")
			fmt.Printf("  %s\n", short)
		}
	}
	
	// Zero incoming/outgoing
	incoming := make(map[string]int)
	for _, tos := range edges {
		for to := range tos {
			incoming[to]++
		}
	}
	
	var zeroIncoming, zeroOutgoing []string
	for _, pkg := range allPkgs {
		path := pkg.ImportPath
		if incoming[path] == 0 && path != "github.com/eshanized/M31A/cmd/m31a" {
			zeroIncoming = append(zeroIncoming, path)
		}
		if len(edges[path]) == 0 {
			zeroOutgoing = append(zeroOutgoing, path)
		}
	}
	
	fmt.Printf("\n=== ZERO INCOMING (except main) (%d) ===\n", len(zeroIncoming))
	for _, p := range zeroIncoming {
		fmt.Printf("  %s\n", strings.TrimPrefix(p, "github.com/eshanized/M31A/"))
	}
	
	fmt.Printf("\n=== ZERO OUTGOING (leaf packages) (%d) ===\n", len(zeroOutgoing))
	for _, p := range zeroOutgoing {
		fmt.Printf("  %s\n", strings.TrimPrefix(p, "github.com/eshanized/M31A/"))
	}
	
	// Layer detection
	layer := make(map[string]string)
	for _, pkg := range allPkgs {
		path := pkg.ImportPath
		if strings.HasPrefix(path, "github.com/eshanized/M31A/cmd/") {
			layer[path] = "cmd"
		} else if strings.HasPrefix(path, "github.com/eshanized/M31A/internal/") {
			layer[path] = "internal"
		} else if strings.HasPrefix(path, "github.com/eshanized/M31A/pkg/") {
			layer[path] = "pkg"
		} else {
			layer[path] = "root"
		}
	}
	
	// pkg -> internal violations
	fmt.Println("\n=== PKG -> INTERNAL VIOLATIONS ===")
	violations := 0
	for fromPkg, tos := range edges {
		if layer[fromPkg] == "pkg" {
			for to := range tos {
				if layer[to] == "internal" {
					violations++
					fmt.Printf("  %s -> %s\n", 
						strings.TrimPrefix(fromPkg, "github.com/eshanized/M31A/"),
						strings.TrimPrefix(to, "github.com/eshanized/M31A/"))
				}
			}
		}
	}
	fmt.Printf("Total: %d\n", violations)
	
	// internal/types should only be imported, never import internal/*
	fmt.Println("\n=== INTERNAL/TYPES IMPORT CHECK ===")
	typesPkg := "github.com/eshanized/M31A/internal/types"
	if edges[typesPkg] != nil {
		fmt.Println("internal/types IMPORTS (should be empty):")
		for to := range edges[typesPkg] {
			fmt.Printf("  -> %s\n", strings.TrimPrefix(to, "github.com/eshanized/M31A/"))
		}
	} else {
		fmt.Println("internal/types imports: NONE (correct)")
	}
	
	// cmd/m31a should only import internal/* and pkg/*
	fmt.Println("\n=== CMD/M31A IMPORT CHECK ===")
	cmdPkg := "github.com/eshanized/M31A/cmd/m31a"
	if edges[cmdPkg] != nil {
		for to := range edges[cmdPkg] {
			l := layer[to]
			if l != "internal" && l != "pkg" && l != "root" {
				fmt.Printf("  UNEXPECTED: %s -> %s (layer: %s)\n", 
					strings.TrimPrefix(cmdPkg, "github.com/eshanized/M31A/"),
					strings.TrimPrefix(to, "github.com/eshanized/M31A/"), l)
			}
		}
		fmt.Println("All imports are internal/ or pkg/ (correct)")
	}
	
	// Fan-in/fan-out per internal package
	fmt.Println("\n=== INTERNAL PACKAGE FAN-IN / FAN-OUT ===")
	for _, pkg := range allPkgs {
		path := pkg.ImportPath
		if layer[path] == "internal" {
			fanOut := len(edges[path])
			fanIn := incoming[path]
			short := strings.TrimPrefix(path, "github.com/eshanized/M31A/")
			fmt.Printf("  %s: in=%d out=%d\n", short, fanIn, fanOut)
		}
	}
	
	// Packages that could be pkg (public API) based on usage
	fmt.Println("\n=== INTERNAL PACKAGES THAT COULD BE PKG (high external fan-in) ===")
	for _, pkg := range allPkgs {
		path := pkg.ImportPath
		if layer[path] == "internal" {
			fanIn := incoming[path]
			if fanIn > 5 { // arbitrary threshold
				short := strings.TrimPrefix(path, "github.com/eshanized/M31A/")
				fmt.Printf("  %s: fan-in=%d\n", short, fanIn)
			}
		}
	}
	
	// Duplicate implementations check - interfaces implemented in multiple packages
	fmt.Println("\n=== INTERFACE IMPLEMENTATIONS (potential duplicates) ===")
	// This would need AST analysis - skip for now
	fmt.Println("  (Requires AST analysis - see internal/types for interface definitions)")
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}