package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"
)

type Package struct {
	Dir           string   `json:"Dir"`
	ImportPath    string   `json:"ImportPath"`
	Name          string   `json:"Name"`
	Imports       []string `json:"Imports"`
	TestImports   []string `json:"TestImports"`
	XTestImports  []string `json:"XTestImports"`
	Standard      bool     `json:"Standard"`
	Goroot        bool     `json:"Goroot"`
	DepOnly       bool     `json:"DepOnly"`
}

func main() {
	// Read from stdin
	dec := json.NewDecoder(os.Stdin)
	
	packages := make(map[string]*Package)
	var allPkgs []*Package
	
	for {
		var pkg Package
		if err := dec.Decode(&pkg); err != nil {
			break
		}
		
		// Only care about our project packages
		if strings.HasPrefix(pkg.ImportPath, "github.com/eshanized/M31A") {
			packages[pkg.ImportPath] = &pkg
			allPkgs = append(allPkgs, &pkg)
		}
	}
	
	// Determine layer for each package
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
	
	// Build adjacency list
	edges := make(map[string]map[string]bool)
	for _, pkg := range allPkgs {
		from := pkg.ImportPath
		if _, ok := edges[from]; !ok {
			edges[from] = make(map[string]bool)
		}
		
		allImports := append(append([]string{}, pkg.Imports...), pkg.TestImports...)
		allImports = append(allImports, pkg.XTestImports...)
		
		for _, imp := range allImports {
			if strings.HasPrefix(imp, "github.com/eshanized/M31A") {
				edges[from][imp] = true
			}
		}
	}
	
	// Find cycles using Tarjan's algorithm
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
	
	// Find packages with zero incoming/outgoing edges
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
	
	// Find pkg -> internal violations
	var violations []string
	for from, tos := range edges {
		if layer[from] == "pkg" {
			for to := range tos {
				if layer[to] == "internal" {
					violations = append(violations, fmt.Sprintf("%s -> %s", from, to))
				}
			}
		}
	}
	
	// Generate DOT file
	fmt.Println("digraph M31A_Dependencies {")
	fmt.Println("  rankdir=LR;")
	fmt.Println("  node [fontname=\"Helvetica\", fontsize=10];")
	fmt.Println("  edge [fontname=\"Helvetica\", fontsize=8];")
	fmt.Println()
	
	// Define nodes with colors by layer
	colorMap := map[string]string{
		"cmd":      "#e74c3c",     // red
		"internal": "#3498db",     // blue
		"pkg":      "#2ecc71",     // green
		"root":     "#f39c12",     // orange
		"stdlib":   "#95a5a6",     // gray
	}
	
	// Group by layer for subgraphs
	layerPkgs := make(map[string][]string)
	for _, pkg := range allPkgs {
		l := layer[pkg.ImportPath]
		layerPkgs[l] = append(layerPkgs[l], pkg.ImportPath)
	}
	
	for l, pkgs := range layerPkgs {
		fmt.Printf("  subgraph cluster_%s {\n", l)
		fmt.Printf("    label=\"%s\";\n", strings.Title(l))
		fmt.Printf("    color=%s;\n", colorMap[l])
		for _, p := range pkgs {
			shortName := strings.TrimPrefix(p, "github.com/eshanized/M31A/")
			style := "filled"
			if l == "cmd" {
				style = "filled,bold"
			}
			if isInCycles(p, cycles) {
				style += ",peripheries=2"
			}
			fmt.Printf("    \"%s\" [label=\"%s\", fillcolor=%s, style=\"%s\"];\n", p, shortName, colorMap[l], style)
		}
		fmt.Println("  }")
		fmt.Println()
	}
	
	// Add edges
	for fromPkg, tos := range edges {
		for to := range tos {
			edgeColor := "#95a5a6"
			penWidth := "1.0"
			
			fromLayer := layer[fromPkg]
			toLayer := layer[to]
			
			if fromLayer == "pkg" && toLayer == "internal" {
				edgeColor = "#e74c3c"
				penWidth = "2.0"
			} else if fromLayer == "internal" && toLayer == "internal" {
				edgeColor = "#3498db"
			} else if fromLayer == "cmd" {
				edgeColor = "#e74c3c"
			} else if fromLayer == "pkg" {
				edgeColor = "#2ecc71"
			}
			
			// Check if this edge is part of a cycle
			inCycle := false
			for _, cycle := range cycles {
				if contains(cycle, fromPkg) && contains(cycle, to) {
					inCycle = true
					break
				}
			}
			
			if inCycle {
				edgeColor = "#e67e22"
				penWidth = "2.0"
			}
			
			fmt.Printf("  \"%s\" -> \"%s\" [color=%s, penwidth=%s];\n", fromPkg, to, edgeColor, penWidth)
		}
	}
	
	// Add legend
	fmt.Println()
	fmt.Println("  // Legend")
	fmt.Println("  subgraph cluster_legend {")
	fmt.Println("    label=\"Legend\";")
	fmt.Println("    style=dashed;")
	fmt.Println("    node [shape=plaintext];")
	fmt.Println("    cmd_node [label=\"cmd (main)\", fillcolor=\"#e74c3c\", style=filled];")
	fmt.Println("    internal_node [label=\"internal\", fillcolor=\"#3498db\", style=filled];")
	fmt.Println("    pkg_node [label=\"pkg (public)\", fillcolor=\"#2ecc71\", style=filled];")
	fmt.Println("    root_node [label=\"root\", fillcolor=\"#f39c12\", style=filled];")
	fmt.Println("    normal_edge [label=\"normal\", style=invis];")
	fmt.Println("    violation_edge [label=\"pkg→internal VIOLATION\", color=\"#e74c3c\", penwidth=2.0, style=invis];")
	fmt.Println("    cycle_edge [label=\"cycle\", color=\"#e67e22\", penwidth=2.0, style=invis];")
	fmt.Println("  }")
	fmt.Println("}")
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func contains(slice []string, item string) bool {
	for _, s := range slice {
		if s == item {
			return true
		}
	}
	return false
}

func isInCycles(pkg string, cycles [][]string) bool {
	for _, cycle := range cycles {
		if contains(cycle, pkg) {
			return true
		}
	}
	return false
}